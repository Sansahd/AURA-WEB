use std::{
    collections::HashMap,
    io,
    net::TcpStream,
    sync::mpsc::{self, Receiver, Sender, TryRecvError},
    thread,
    time::Duration,
};

use tungstenite::{
    client::IntoClientRequest,
    connect,
    http::{
        header::{COOKIE, ORIGIN, SEC_WEBSOCKET_PROTOCOL},
        HeaderValue,
    },
    protocol::{frame::coding::CloseCode, CloseFrame, Message},
    stream::MaybeTlsStream,
    WebSocket,
};
use url::Url;

use crate::{
    js_runtime::{WebSocketEvent, WebSocketRequest},
    privacy::{PrivacyDecision, PrivacyPolicy, RequestContext, ResourceKind},
};

#[derive(Debug)]
enum WorkerCommand {
    Text(String),
    Binary(Vec<u8>),
    Close { code: Option<u16>, reason: String },
}

#[derive(Debug)]
pub struct WebSocketHub {
    workers: HashMap<u64, Sender<WorkerCommand>>,
    event_tx: Sender<WebSocketEvent>,
    event_rx: Receiver<WebSocketEvent>,
}

impl Default for WebSocketHub {
    fn default() -> Self {
        let (event_tx, event_rx) = mpsc::channel();
        Self {
            workers: HashMap::new(),
            event_tx,
            event_rx,
        }
    }
}

impl WebSocketHub {
    pub fn handle_request(
        &mut self,
        policy: &PrivacyPolicy,
        top_level: &Url,
        cookie_header: Option<&str>,
        request: WebSocketRequest,
    ) -> Result<(), String> {
        match request.action.as_str() {
            "open" => self.open(policy, top_level, cookie_header, request),
            "send" => self.send(request),
            "close" => self.close(request),
            action => Err(format!("unsupported WebSocket action: {action}")),
        }
    }

    pub fn drain_events(&mut self) -> Vec<WebSocketEvent> {
        let mut events = Vec::new();
        while let Ok(event) = self.event_rx.try_recv() {
            if event.event_type == "close" {
                self.workers.remove(&event.socket_id);
            }
            events.push(event);
        }
        events
    }

    fn open(
        &mut self,
        policy: &PrivacyPolicy,
        top_level: &Url,
        cookie_header: Option<&str>,
        request: WebSocketRequest,
    ) -> Result<(), String> {
        if self.workers.contains_key(&request.socket_id) {
            return Err(format!("WebSocket {} already exists", request.socket_id));
        }

        let raw_url = request
            .url
            .as_deref()
            .ok_or_else(|| "WebSocket open requires a URL".to_string())?;
        let target = Url::parse(raw_url).map_err(|error| error.to_string())?;
        if !matches!(target.scheme(), "ws" | "wss") {
            return Err("WebSocket URL must use ws:// or wss://".to_string());
        }

        let target_host = target
            .host_str()
            .ok_or_else(|| "WebSocket URL has no host".to_string())?;
        let top_host = top_level
            .host_str()
            .ok_or_else(|| "top-level page has no host".to_string())?;

        match policy.decide(&RequestContext {
            top_level_host: top_host.to_string(),
            request_host: target_host.to_string(),
            kind: ResourceKind::WebSocket,
        }) {
            PrivacyDecision::Allow => {}
            PrivacyDecision::Block { reason } => {
                return Err(format!("blocked by Quantic Privacy Core: {reason}"));
            }
        }

        let (command_tx, command_rx) = mpsc::channel();
        self.workers.insert(request.socket_id, command_tx);

        let event_tx = self.event_tx.clone();
        let socket_id = request.socket_id;
        let protocols = request.protocols.clone();
        let origin = top_level.origin().ascii_serialization();
        let cookie = cookie_header.map(str::to_string);
        let target_string = target.to_string();

        thread::Builder::new()
            .name(format!("quantic-websocket-{socket_id}"))
            .spawn(move || {
                websocket_worker(
                    socket_id,
                    &target_string,
                    &origin,
                    cookie.as_deref(),
                    &protocols,
                    command_rx,
                    event_tx,
                );
            })
            .map_err(|error| {
                self.workers.remove(&socket_id);
                format!("failed to spawn WebSocket worker: {error}")
            })?;

        Ok(())
    }

    fn send(&self, request: WebSocketRequest) -> Result<(), String> {
        let sender = self
            .workers
            .get(&request.socket_id)
            .ok_or_else(|| format!("unknown WebSocket {}", request.socket_id))?;

        let command = if let Some(binary) = request.binary {
            WorkerCommand::Binary(binary)
        } else {
            WorkerCommand::Text(request.text.unwrap_or_default())
        };
        sender
            .send(command)
            .map_err(|_| "WebSocket worker is no longer available".to_string())
    }

    fn close(&self, request: WebSocketRequest) -> Result<(), String> {
        let sender = self
            .workers
            .get(&request.socket_id)
            .ok_or_else(|| format!("unknown WebSocket {}", request.socket_id))?;
        sender
            .send(WorkerCommand::Close {
                code: request.code,
                reason: request.reason.unwrap_or_default(),
            })
            .map_err(|_| "WebSocket worker is no longer available".to_string())
    }
}

#[allow(clippy::too_many_arguments)]
fn websocket_worker(
    socket_id: u64,
    url: &str,
    origin: &str,
    cookie_header: Option<&str>,
    protocols: &[String],
    command_rx: Receiver<WorkerCommand>,
    event_tx: Sender<WebSocketEvent>,
) {
    let result = connect_socket(url, origin, cookie_header, protocols);
    let (mut socket, protocol) = match result {
        Ok(value) => value,
        Err(error) => {
            let _ = event_tx.send(WebSocketEvent::error(socket_id, error));
            let _ = event_tx.send(WebSocketEvent::close(
                socket_id,
                1006,
                "connection failed".to_string(),
                false,
            ));
            return;
        }
    };

    let _ = event_tx.send(WebSocketEvent::open(socket_id, protocol));
    let _ = set_read_timeout(&socket, Some(Duration::from_millis(25)));

    loop {
        loop {
            match command_rx.try_recv() {
                Ok(WorkerCommand::Text(text)) => {
                    if let Err(error) = socket.send(Message::Text(text.into())) {
                        let _ = event_tx.send(WebSocketEvent::error(socket_id, error.to_string()));
                        return;
                    }
                }
                Ok(WorkerCommand::Binary(binary)) => {
                    if let Err(error) = socket.send(Message::Binary(binary.into())) {
                        let _ = event_tx.send(WebSocketEvent::error(socket_id, error.to_string()));
                        return;
                    }
                }
                Ok(WorkerCommand::Close { code, reason }) => {
                    let frame = code.map(|code| CloseFrame {
                        code: CloseCode::from(code),
                        reason: reason.into(),
                    });
                    let _ = socket.close(frame);
                    return;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    let _ = socket.close(None);
                    return;
                }
            }
        }

        match socket.read() {
            Ok(Message::Text(text)) => {
                let _ = event_tx.send(WebSocketEvent::text(socket_id, text.to_string()));
            }
            Ok(Message::Binary(binary)) => {
                let _ = event_tx.send(WebSocketEvent::binary(socket_id, binary.to_vec()));
            }
            Ok(Message::Close(frame)) => {
                let (code, reason) = frame
                    .map(|frame| (u16::from(frame.code), frame.reason.to_string()))
                    .unwrap_or((1000, String::new()));
                let _ = event_tx.send(WebSocketEvent::close(socket_id, code, reason, true));
                return;
            }
            Ok(Message::Ping(payload)) => {
                let _ = socket.send(Message::Pong(payload));
            }
            Ok(Message::Pong(_) | Message::Frame(_)) => {}
            Err(tungstenite::Error::Io(error))
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) => {}
            Err(tungstenite::Error::ConnectionClosed | tungstenite::Error::AlreadyClosed) => {
                let _ = event_tx.send(WebSocketEvent::close(socket_id, 1000, String::new(), true));
                return;
            }
            Err(error) => {
                let _ = event_tx.send(WebSocketEvent::error(socket_id, error.to_string()));
                let _ = event_tx.send(WebSocketEvent::close(
                    socket_id,
                    1006,
                    "transport error".to_string(),
                    false,
                ));
                return;
            }
        }
    }
}

fn connect_socket(
    url: &str,
    origin: &str,
    cookie_header: Option<&str>,
    protocols: &[String],
) -> Result<(WebSocket<MaybeTlsStream<TcpStream>>, String), String> {
    let mut request = url
        .into_client_request()
        .map_err(|error| error.to_string())?;

    request.headers_mut().insert(
        ORIGIN,
        HeaderValue::from_str(origin).map_err(|error| error.to_string())?,
    );
    if let Some(cookie_header) = cookie_header.filter(|value| !value.is_empty()) {
        request.headers_mut().insert(
            COOKIE,
            HeaderValue::from_str(cookie_header).map_err(|error| error.to_string())?,
        );
    }
    if !protocols.is_empty() {
        request.headers_mut().insert(
            SEC_WEBSOCKET_PROTOCOL,
            HeaderValue::from_str(&protocols.join(", ")).map_err(|error| error.to_string())?,
        );
    }

    let (socket, response) = connect(request).map_err(|error| error.to_string())?;
    let protocol = response
        .headers()
        .get(SEC_WEBSOCKET_PROTOCOL)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_string();

    Ok((socket, protocol))
}

fn set_read_timeout(
    socket: &WebSocket<MaybeTlsStream<TcpStream>>,
    timeout: Option<Duration>,
) -> io::Result<()> {
    match socket.get_ref() {
        MaybeTlsStream::Plain(stream) => stream.set_read_timeout(timeout),
        MaybeTlsStream::Rustls(stream) => stream.sock.set_read_timeout(timeout),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::privacy::PrivacyPolicy;

    #[test]
    fn loopback_worker_connects_receives_and_sends_without_blocking_page_thread() {
        use std::{
            net::TcpListener,
            time::{Duration, Instant},
        };

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut socket = tungstenite::accept(stream).unwrap();
            socket.send(Message::Text("server-ready".into())).unwrap();

            let message = socket.read().unwrap();
            let Message::Text(text) = message else {
                panic!("expected text frame from Quantic worker");
            };
            socket
                .send(Message::Text(format!("echo:{text}").into()))
                .unwrap();
            let _ = socket.close(None);
        });

        let top = Url::parse("http://127.0.0.1/").unwrap();
        let mut hub = WebSocketHub::default();
        hub.handle_request(
            &PrivacyPolicy::default(),
            &top,
            None,
            WebSocketRequest {
                socket_id: 7,
                action: "open".to_string(),
                url: Some(format!("ws://{address}/socket")),
                protocols: Vec::new(),
                text: None,
                binary: None,
                code: None,
                reason: None,
            },
        )
        .unwrap();

        let deadline = Instant::now() + Duration::from_secs(3);
        let mut events = Vec::new();
        while Instant::now() < deadline {
            events.extend(hub.drain_events());
            if events.iter().any(|event| {
                event.event_type == "message" && event.text.as_deref() == Some("server-ready")
            }) {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }

        assert!(events.iter().any(|event| event.event_type == "open"));
        assert!(events
            .iter()
            .any(|event| event.event_type == "message"
                && event.text.as_deref() == Some("server-ready")));

        hub.handle_request(
            &PrivacyPolicy::default(),
            &top,
            None,
            WebSocketRequest {
                socket_id: 7,
                action: "send".to_string(),
                url: None,
                protocols: Vec::new(),
                text: Some("hello".to_string()),
                binary: None,
                code: None,
                reason: None,
            },
        )
        .unwrap();

        let deadline = Instant::now() + Duration::from_secs(3);
        let mut echo = None;
        while Instant::now() < deadline {
            for event in hub.drain_events() {
                if event.event_type == "message" {
                    echo = event.text;
                }
            }
            if echo.is_some() {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }

        assert_eq!(echo.as_deref(), Some("echo:hello"));
        server.join().unwrap();
    }

    #[test]
    fn privacy_gate_blocks_third_party_websocket_before_worker_spawn() {
        let mut hub = WebSocketHub::default();
        let top = Url::parse("https://app.example/").unwrap();
        let request = WebSocketRequest {
            socket_id: 1,
            action: "open".to_string(),
            url: Some("wss://tracker.invalid/socket".to_string()),
            protocols: Vec::new(),
            text: None,
            binary: None,
            code: None,
            reason: None,
        };

        let error = hub
            .handle_request(&PrivacyPolicy::default(), &top, None, request)
            .unwrap_err();
        assert!(error.contains("third-party-denied-by-default"));
        assert!(hub.workers.is_empty());
    }
}
