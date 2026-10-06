use std::{
    cell::RefCell,
    io,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, ToSocketAddrs, UdpSocket},
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};

use neqo_common::{Datagram, Header, Tos, event::Provider as _};
use neqo_http3::{
    Http3Client, Http3ClientEvent, Http3Parameters, Http3State, Output, Priority, StreamId,
};
use neqo_transport::EmptyConnectionIdGenerator;
use nss::AuthenticationStatus;
use rustls::{
    RootCertStore,
    client::{WebPkiServerVerifier, danger::ServerCertVerifier},
    pki_types::{CertificateDer, ServerName, UnixTime},
};
use url::Url;

pub struct NeqoHttp3Core {
    client: Http3Client,
}

impl NeqoHttp3Core {
    pub fn new(
        server_name: &str,
        local_addr: SocketAddr,
        remote_addr: SocketAddr,
    ) -> Result<Self, String> {
        nss::init().map_err(|error| format!("NSS init failed: {error}"))?;
        let client = Http3Client::new(
            server_name,
            Rc::new(RefCell::new(EmptyConnectionIdGenerator::default())),
            local_addr,
            remote_addr,
            Http3Parameters::default(),
            Instant::now(),
        )
        .map_err(|error| format!("Neqo HTTP/3 init failed: {error}"))?;
        Ok(Self { client })
    }

    pub fn state(&self) -> Http3State {
        self.client.state()
    }

    /// Drive Neqo far enough to produce a real QUIC Initial datagram.
    /// The caller owns UDP I/O and certificate decisions.
    pub fn initial_datagram(&mut self) -> Option<Vec<u8>> {
        self.client
            .process_output(Instant::now())
            .dgram()
            .map(|datagram| datagram.to_vec())
    }

    pub fn transport_stats_debug(&self) -> String {
        format!("{:?}", self.client.transport_stats())
    }
}

#[derive(Clone, Debug)]
pub struct H3Response {
    pub status: u16,
    pub headers: Vec<(String, Vec<u8>)>,
    pub body: Vec<u8>,
}

fn resolve_remote(host: &str, port: u16) -> Result<SocketAddr, String> {
    (host, port)
        .to_socket_addrs()
        .map_err(|error| format!("DNS failed for {host}:{port}: {error}"))?
        .next()
        .ok_or_else(|| format!("No address for {host}:{port}"))
}

fn bind_for(remote: SocketAddr) -> Result<UdpSocket, String> {
    let local = match remote.ip() {
        IpAddr::V4(_) => SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0),
        IpAddr::V6(_) => SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0),
    };
    UdpSocket::bind(local).map_err(|error| format!("UDP bind failed: {error}"))
}

fn verify_peer(client: &Http3Client, host: &str) -> Result<(), String> {
    let certificate = client
        .peer_certificate()
        .ok_or_else(|| "Neqo peer certificate unavailable".to_string())?;
    let chain = certificate.iter().map(|der| der.to_vec()).collect::<Vec<_>>();
    let Some(end_entity) = chain.first() else {
        return Err("Neqo peer certificate chain is empty".into());
    };

    let end_entity = CertificateDer::from(end_entity.clone());
    let intermediates = chain
        .iter()
        .skip(1)
        .cloned()
        .map(CertificateDer::from)
        .collect::<Vec<_>>();
    let server_name = ServerName::try_from(host.to_string())
        .map_err(|_| format!("Invalid TLS server name: {host}"))?;
    let roots = Arc::new(RootCertStore::from_iter(
        webpki_roots::TLS_SERVER_ROOTS.iter().cloned(),
    ));
    let verifier = WebPkiServerVerifier::builder(roots)
        .build()
        .map_err(|error| format!("TLS verifier setup failed: {error:?}"))?;

    verifier
        .verify_server_cert(
            &end_entity,
            &intermediates,
            &server_name,
            &[],
            UnixTime::now(),
        )
        .map_err(|error| format!("TLS certificate rejected for {host}: {error}"))?;
    Ok(())
}

fn request_target(url: &Url) -> (String, String) {
    let port = url.port_or_known_default().unwrap_or(443);
    let host = url.host_str().unwrap_or_default();
    let authority = if port == 443 {
        host.to_string()
    } else {
        format!("{host}:{port}")
    };
    let mut path = url.path().to_string();
    if path.is_empty() {
        path.push('/');
    }
    if let Some(query) = url.query() {
        path.push('?');
        path.push_str(query);
    }
    (authority, path)
}

fn process_events(
    client: &mut Http3Client,
    host: &str,
    method: &str,
    authority: &str,
    path: &str,
    request_headers: &[Header],
    stream: &mut Option<StreamId>,
    response_status: &mut Option<u16>,
    response_headers: &mut Vec<(String, Vec<u8>)>,
    body: &mut Vec<u8>,
    done: &mut bool,
) -> Result<(), String> {
    while let Some(event) = client.next_event() {
        match event {
            Http3ClientEvent::AuthenticationNeeded => {
                match verify_peer(client, host) {
                    Ok(()) => client.authenticated(AuthenticationStatus::Ok, Instant::now()),
                    Err(error) => {
                        client.authenticated(AuthenticationStatus::Unknown, Instant::now());
                        return Err(error);
                    }
                }
            }
            Http3ClientEvent::StateChange(Http3State::Connected)
            | Http3ClientEvent::RequestsCreatable => {
                if stream.is_none() {
                    let now = Instant::now();
                    let id = client
                        .fetch(
                            now,
                            method,
                            ("https", authority, path),
                            request_headers,
                            Priority::default(),
                        )
                        .map_err(|error| format!("Neqo fetch start failed: {error}"))?;
                    client
                        .stream_close_send(id, now)
                        .map_err(|error| format!("Neqo request close failed: {error}"))?;
                    *stream = Some(id);
                }
            }
            Http3ClientEvent::HeaderReady {
                stream_id,
                headers,
                fin,
                ..
            } if Some(stream_id) == *stream => {
                for header in headers {
                    if header.name() == ":status" {
                        if let Ok(value) = header.value_utf8() {
                            *response_status = value.parse::<u16>().ok();
                        }
                    } else if !header.name().starts_with(':') {
                        response_headers.push((header.name().to_string(), header.value().to_vec()));
                    }
                }
                if fin {
                    *done = true;
                }
            }
            Http3ClientEvent::DataReadable { stream_id } if Some(stream_id) == *stream => {
                loop {
                    let mut chunk = vec![0_u8; 64 * 1024];
                    let (amount, fin) = client
                        .read_data(Instant::now(), stream_id, &mut chunk)
                        .map_err(|error| format!("Neqo response read failed: {error}"))?;
                    if amount > 0 {
                        body.extend_from_slice(&chunk[..amount]);
                        if body.len() > 64 * 1024 * 1024 {
                            return Err("HTTP/3 response exceeded 64 MiB safety limit".into());
                        }
                    }
                    if fin {
                        *done = true;
                        break;
                    }
                    if amount == 0 {
                        break;
                    }
                }
            }
            Http3ClientEvent::StateChange(Http3State::Closed(_)) => {
                if !*done {
                    return Err("Neqo HTTP/3 connection closed before response completed".into());
                }
            }
            _ => {}
        }
    }
    Ok(())
}

pub fn fetch_https(
    url: &Url,
    method: &str,
    headers: &[(String, Vec<u8>)],
    timeout: Duration,
) -> Result<H3Response, String> {
    if url.scheme() != "https" {
        return Err("Neqo H3 only accepts https URLs".into());
    }
    if !matches!(method, "GET" | "HEAD") {
        return Err(format!("Neqo H3 method not supported yet: {method}"));
    }

    nss::init().map_err(|error| format!("NSS init failed: {error}"))?;
    let host = url
        .host_str()
        .ok_or_else(|| "HTTPS URL has no host".to_string())?;
    let port = url.port_or_known_default().unwrap_or(443);
    let remote = resolve_remote(host, port)?;
    let socket = bind_for(remote)?;
    let local = socket
        .local_addr()
        .map_err(|error| format!("UDP local address failed: {error}"))?;
    socket
        .set_read_timeout(Some(Duration::from_millis(40)))
        .map_err(|error| format!("UDP timeout setup failed: {error}"))?;

    let mut client = Http3Client::new(
        host,
        Rc::new(RefCell::new(EmptyConnectionIdGenerator::default())),
        local,
        remote,
        Http3Parameters::default(),
        Instant::now(),
    )
    .map_err(|error| format!("Neqo HTTP/3 init failed: {error}"))?;

    let (authority, path) = request_target(url);
    let mut request_headers = headers
        .iter()
        .filter(|(name, _)| {
            !matches!(
                name.to_ascii_lowercase().as_str(),
                "connection" | "host" | "keep-alive" | "proxy-connection"
                    | "transfer-encoding" | "upgrade" | "accept-encoding"
            )
        })
        .map(|(name, value)| Header::new(name.to_ascii_lowercase(), value.clone()))
        .collect::<Vec<_>>();
    if !request_headers.iter().any(|header| header.name() == "user-agent") {
        request_headers.push(Header::new("user-agent", "Quantic-Gekko/0.8.5"));
    }
    if !request_headers.iter().any(|header| header.name() == "accept") {
        request_headers.push(Header::new("accept", "*/*"));
    }

    let deadline = Instant::now() + timeout;
    let mut stream = None;
    let mut response_status = None;
    let mut response_headers = Vec::new();
    let mut body = Vec::new();
    let mut done = false;
    let mut recv_buf = vec![0_u8; 65_535];

    while Instant::now() < deadline {
        process_events(
            &mut client,
            host,
            method,
            &authority,
            &path,
            &request_headers,
            &mut stream,
            &mut response_status,
            &mut response_headers,
            &mut body,
            &mut done,
        )?;
        if done {
            return Ok(H3Response {
                status: response_status.unwrap_or(200),
                headers: response_headers,
                body,
            });
        }

        match client.process_output(Instant::now()) {
            Output::Datagram(datagram) => {
                socket
                    .send_to(&datagram, datagram.destination())
                    .map_err(|error| format!("UDP send failed: {error}"))?;
                continue;
            }
            Output::Callback(delay) => {
                let remaining = deadline.saturating_duration_since(Instant::now());
                let wait = delay.min(remaining).min(Duration::from_millis(40));
                let _ = socket.set_read_timeout(Some(wait.max(Duration::from_millis(1))));
            }
            Output::None => {}
        }

        match socket.recv_from(&mut recv_buf) {
            Ok((size, source)) if size > 0 => {
                let datagram = Datagram::new(
                    source,
                    local,
                    Tos::default(),
                    recv_buf[..size].to_vec(),
                );
                client.process_input(datagram, Instant::now());
            }
            Ok(_) => {}
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) => {}
            Err(error) => return Err(format!("UDP receive failed: {error}")),
        }
    }

    Err(format!("Neqo HTTP/3 timed out after {} ms", timeout.as_millis()))
}

pub fn probe_https(url: &Url, timeout: Duration) -> Result<(), String> {
    fetch_https(url, "HEAD", &[], timeout)
        .or_else(|_| fetch_https(url, "GET", &[], timeout))
        .map(|_| ())
}

pub fn default_probe() -> Result<NeqoHttp3Core, String> {
    let local = "0.0.0.0:0"
        .parse::<SocketAddr>()
        .map_err(|error| error.to_string())?;
    let remote = "192.0.2.1:443"
        .parse::<SocketAddr>()
        .map_err(|error| error.to_string())?;
    NeqoHttp3Core::new("example.com", local, remote)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_real_neqo_http3_client() {
        let probe = default_probe().expect("create Neqo client");
        assert!(matches!(probe.state(), Http3State::Initializing));
    }

    #[test]
    fn emits_real_quic_initial_packet() {
        let mut probe = default_probe().expect("create Neqo client");
        let datagram = probe.initial_datagram().expect("QUIC initial datagram");
        assert!(datagram.len() >= 1200, "QUIC Initial must be padded");
        assert_ne!(datagram[0] & 0x80, 0, "QUIC Initial uses a long header");
    }

    #[test]
    fn request_target_keeps_query_and_non_default_port() {
        let url = Url::parse("https://example.test:8443/a/b?q=1").unwrap();
        let (authority, path) = request_target(&url);
        assert_eq!(authority, "example.test:8443");
        assert_eq!(path, "/a/b?q=1");
    }
}
