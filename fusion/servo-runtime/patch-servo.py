import os
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SERVO = ROOT / "vendor" / "servo"

def replace_once(path: Path, old: str, new: str) -> None:
    text = path.read_text(encoding="utf-8")
    if new in text:
        return
    if old not in text:
        raise SystemExit(f"Servo patch anchor missing: {path} :: {old[:80]!r}")
    path.write_text(text.replace(old, new, 1), encoding="utf-8")

embedder = SERVO / "components/net/embedder.rs"
replace_once(
    embedder,
    """    /// Response to an asynchronous request from SiteDataManager with cookies result.
    EmbedderCookieOperationResponseWithCookies(CookieOperationId, Vec<Cookie<'static>>),
""",
    """    /// Ask the WebView embedder where a native navigation download should be saved.
    RequestDownloadPath(
        WebViewId,
        ServoUrl,
        Option<String>,
        TokioOneshotSender<Option<PathBuf>>,
    ),
    /// Report completion (or failure) of a native navigation download.
    DownloadFinished(
        WebViewId,
        PathBuf,
        u64,
        Option<String>,
    ),
    /// Response to an asynchronous request from SiteDataManager with cookies result.
    EmbedderCookieOperationResponseWithCookies(CookieOperationId, Vec<Cookie<'static>>),
"""
)

delegate = SERVO / "components/servo/webview_delegate.rs"
replace_once(
    delegate,
    """    fn load_web_resource(&self, _webview: WebView, _load: WebResourceLoad) {}

    /// Request to display a notification.
""",
    """    fn load_web_resource(&self, _webview: WebView, _load: WebResourceLoad) {}

    /// Choose a filesystem destination for a navigation response that Servo identified as
    /// a download. Returning None cancels the download without replacing the current page.
    fn request_download_path(
        &self,
        _webview: WebView,
        _url: Url,
        _suggested_filename: Option<String>,
    ) -> Option<PathBuf> {
        None
    }

    /// Notify the embedder that a native download finished or failed.
    fn notify_download_finished(
        &self,
        _webview: WebView,
        _path: PathBuf,
        _bytes: u64,
        _error: Option<String>,
    ) {
    }

    /// Request to display a notification.
"""
)

servo_rs = SERVO / "components/servo/servo.rs"
replace_once(
    servo_rs,
    """            NetToEmbedderMsg::EmbedderCookieOperationResponseWithCookies(operation_id, cookies) => {
""",
    """            NetToEmbedderMsg::RequestDownloadPath(
                webview_id,
                url,
                suggested_filename,
                response_sender,
            ) => {
                let path = self.get_webview_handle(webview_id).and_then(|webview| {
                    webview.delegate().request_download_path(
                        webview,
                        url.into_url(),
                        suggested_filename,
                    )
                });
                let _ = response_sender.send(path);
            },
            NetToEmbedderMsg::DownloadFinished(webview_id, path, bytes, error) => {
                if let Some(webview) = self.get_webview_handle(webview_id) {
                    webview
                        .delegate()
                        .notify_download_finished(webview, path, bytes, error);
                }
            },
            NetToEmbedderMsg::EmbedderCookieOperationResponseWithCookies(operation_id, cookies) => {
"""
)

http_loader = SERVO / "components/net/http_loader.rs"
replace_once(
    http_loader,
    """use std::collections::HashSet;
use std::iter::FromIterator;
use std::sync::Arc as StdArc;
""",
    """use std::collections::HashSet;
use std::fs::{self, File};
use std::io::Write;
use std::iter::FromIterator;
use std::path::{Path, PathBuf};
use std::sync::Arc as StdArc;
"""
)
replace_once(
    http_loader,
    """fn set_default_accept_encoding(headers: &mut HeaderMap) {
""",
    r'''const QUANTIC_DOWNLOAD_HEADER: &str = "x-quantic-download";

fn decode_download_filename(value: &str) -> Option<String> {
    if value.is_empty() || value == "1" {
        return None;
    }
    content_security_policy::percent_encoding::percent_decode_str(value)
        .decode_utf8()
        .ok()
        .map(|value| value.into_owned())
        .filter(|value| !value.trim().is_empty())
}

fn forced_download_filename(request: &Request) -> Option<String> {
    request
        .headers
        .get(QUANTIC_DOWNLOAD_HEADER)
        .and_then(|value| value.to_str().ok())
        .and_then(decode_download_filename)
}

fn suggested_download_filename(headers: &HeaderMap) -> Option<String> {
    let value = headers
        .get(header::CONTENT_DISPOSITION)?
        .to_str()
        .ok()?;

    for part in value.split(';').map(str::trim) {
        if let Some(encoded) = part.strip_prefix("filename*=UTF-8''") {
            if let Ok(decoded) = content_security_policy::percent_encoding::percent_decode_str(encoded).decode_utf8() {
                if !decoded.trim().is_empty() {
                    return Some(decoded.into_owned());
                }
            }
        }
        if let Some(raw) = part.strip_prefix("filename=") {
            let raw = raw.trim_matches('"').trim_matches('\'').trim();
            if !raw.is_empty() {
                return Some(raw.to_string());
            }
        }
    }
    None
}

fn is_native_download_response(request: &Request, headers: &HeaderMap) -> bool {
    if request.mode != RequestMode::Navigate || request.target_webview_id.is_none() {
        return false;
    }

    if request.headers.contains_key(QUANTIC_DOWNLOAD_HEADER) {
        return true;
    }

    let attachment = headers
        .get(header::CONTENT_DISPOSITION)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| {
            value
                .split(';')
                .next()
                .is_some_and(|kind| kind.trim().eq_ignore_ascii_case("attachment"))
        });

    if attachment {
        return true;
    }

    let mime = headers
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .map(str::trim)
        .unwrap_or_default();

    matches!(
        mime,
        "application/octet-stream"
            | "application/zip"
            | "application/x-7z-compressed"
            | "application/x-rar-compressed"
            | "application/vnd.android.package-archive"
            | "application/x-msdownload"
            | "application/x-msi"
            | "application/x-iso9660-image"
    )
}

fn partial_download_path(path: &Path) -> PathBuf {
    let mut part = path.as_os_str().to_os_string();
    part.push(".part");
    PathBuf::from(part)
}

fn set_default_accept_encoding(headers: &mut HeaderMap) {
'''
)
replace_once(
    http_loader,
    """    let mut headers = request_headers.clone();

    let devtools_bytes = StdArc::new(Mutex::new(vec![]));
""",
    """    let mut headers = request_headers.clone();
    // Quantic's internal marker must survive Servo redirect/request cloning, but it
    // must never be exposed to the remote server.
    headers.remove(QUANTIC_DOWNLOAD_HEADER);

    let devtools_bytes = StdArc::new(Mutex::new(vec![]));
"""
)
replace_once(
    http_loader,
    """    response.headers = response_stream.headers().clone();
    response.referrer = request.referrer.to_url().cloned();
    response.referrer_policy = request.referrer_policy;

    let response_body = response.body.clone();
""",
    """    response.headers = response_stream.headers().clone();
    response.referrer = request.referrer.to_url().cloned();
    response.referrer_policy = request.referrer_policy;

    if is_native_download_response(request, &response.headers) {
        let webview_id = request
            .target_webview_id
            .expect("native download response must belong to a WebView");
        let suggested_filename = forced_download_filename(request)
            .or_else(|| suggested_download_filename(&response.headers));
        let (path_sender, path_receiver) = tokio::sync::oneshot::channel();

        context.state.embedder_proxy.send(NetToEmbedderMsg::RequestDownloadPath(
            webview_id,
            url.clone(),
            suggested_filename,
            path_sender,
        ));

        let chosen_path = path_receiver.await.ok().flatten();

        response.status = HttpStatus::new(StatusCode::NO_CONTENT, b"No Content".to_vec());
        response.headers = HeaderMap::new();
        *response.body.lock() = ResponseBody::Done(vec![]);

        let (done_sender, done_receiver) = unbounded_channel();
        *done_chan = Some((done_sender.clone(), done_receiver));
        let _ = done_sender.send(Data::Done);

        if let Some(final_path) = chosen_path {
            if let Some(parent) = final_path.parent() {
                let _ = fs::create_dir_all(parent);
            }

            let temp_path = partial_download_path(&final_path);
            match File::create(&temp_path) {
                Ok(file) => {
                    let finish_proxy = context.state.embedder_proxy.clone();
                    let error_proxy = context.state.embedder_proxy.clone();
                    let final_for_finish = final_path.clone();
                    let final_for_error = final_path.clone();
                    let temp_for_finish = temp_path.clone();
                    let temp_for_error = temp_path.clone();

                    spawn_task(
                        response_stream
                            .into_body()
                            .try_fold((file, 0_u64), move |(mut file, total), chunk| {
                                let result = file
                                    .write_all(&chunk)
                                    .map(|_| (file, total.saturating_add(chunk.len() as u64)));
                                future::ready(result)
                            })
                            .and_then(move |(mut file, total)| {
                                let result = (|| -> std::io::Result<()> {
                                    file.flush()?;
                                    file.sync_all()?;
                                    fs::rename(&temp_for_finish, &final_for_finish)?;
                                    Ok(())
                                })();

                                if let Err(error) = result {
                                    let _ = fs::remove_file(&temp_for_finish);
                                    finish_proxy.send(NetToEmbedderMsg::DownloadFinished(
                                        webview_id,
                                        final_for_finish,
                                        total,
                                        Some(error.to_string()),
                                    ));
                                } else {
                                    finish_proxy.send(NetToEmbedderMsg::DownloadFinished(
                                        webview_id,
                                        final_for_finish,
                                        total,
                                        None,
                                    ));
                                }
                                future::ready(Ok(()))
                            })
                            .map_err(move |error| {
                                let _ = fs::remove_file(&temp_for_error);
                                error_proxy.send(NetToEmbedderMsg::DownloadFinished(
                                    webview_id,
                                    final_for_error,
                                    0,
                                    Some(error.to_string()),
                                ));
                            }),
                    );
                },
                Err(error) => {
                    context.state.embedder_proxy.send(NetToEmbedderMsg::DownloadFinished(
                        webview_id,
                        final_path,
                        0,
                        Some(error.to_string()),
                    ));
                },
            }
        }

        return response;
    }

    let response_body = response.body.clone();
"""
)


# --- Gekko Fusion: Ladybird shadow parser runs on every real main HTML response. ---
net_cargo = SERVO / "components/net/Cargo.toml"
replace_once(
    net_cargo,
    """malloc_size_of_derive = { workspace = true }
mime = { workspace = true }
""",
    """malloc_size_of_derive = { workspace = true }
quantic-ladybird-html = { path = "../../../../quantic-engine/vendor/ladybird_html" }
mime = { workspace = true }
"""
)

replace_once(
    embedder,
    """    DownloadFinished(
        WebViewId,
        PathBuf,
        u64,
        Option<String>,
    ),
    /// Response to an asynchronous request from SiteDataManager with cookies result.
""",
    """    DownloadFinished(
        WebViewId,
        PathBuf,
        u64,
        Option<String>,
    ),
    /// Ladybird shadow-parser telemetry for a real main HTML document.
    LadybirdDocumentAudit(
        WebViewId,
        u64,
        u64,
    ),
    /// Response to an asynchronous request from SiteDataManager with cookies result.
"""
)

replace_once(
    delegate,
    """    fn notify_download_finished(
        &self,
        _webview: WebView,
        _path: PathBuf,
        _bytes: u64,
        _error: Option<String>,
    ) {
    }

    /// Request to display a notification.
""",
    """    fn notify_download_finished(
        &self,
        _webview: WebView,
        _path: PathBuf,
        _bytes: u64,
        _error: Option<String>,
    ) {
    }

    /// Report that Ladybird tokenized the real HTML response in parallel with Servo.
    fn notify_ladybird_document_audit(
        &self,
        _webview: WebView,
        _token_count: u64,
        _invalid_count: u64,
    ) {
    }

    /// Request to display a notification.
"""
)

replace_once(
    servo_rs,
    """            NetToEmbedderMsg::DownloadFinished(webview_id, path, bytes, error) => {
                if let Some(webview) = self.get_webview_handle(webview_id) {
                    webview
                        .delegate()
                        .notify_download_finished(webview, path, bytes, error);
                }
            },
            NetToEmbedderMsg::EmbedderCookieOperationResponseWithCookies(operation_id, cookies) => {
""",
    """            NetToEmbedderMsg::DownloadFinished(webview_id, path, bytes, error) => {
                if let Some(webview) = self.get_webview_handle(webview_id) {
                    webview
                        .delegate()
                        .notify_download_finished(webview, path, bytes, error);
                }
            },
            NetToEmbedderMsg::LadybirdDocumentAudit(webview_id, token_count, invalid_count) => {
                if let Some(webview) = self.get_webview_handle(webview_id) {
                    webview
                        .delegate()
                        .notify_ladybird_document_audit(webview, token_count, invalid_count);
                }
            },
            NetToEmbedderMsg::EmbedderCookieOperationResponseWithCookies(operation_id, cookies) => {
"""
)

replace_once(
    http_loader,
    """fn set_default_accept_encoding(headers: &mut HeaderMap) {
""",
    """fn ladybird_document_audit(body: &[u8]) -> (u64, u64) {
    use quantic_ladybird_html::{HtmlTokenizer, TokenType};

    let source = String::from_utf8_lossy(body);
    let mut tokenizer = HtmlTokenizer::new(source.encode_utf16().collect());
    let mut tokens = 0_u64;
    let mut invalid = 0_u64;

    while let Some(token) = tokenizer.next_token(false, false) {
        tokens = tokens.saturating_add(1);
        if token.token_type == TokenType::Invalid {
            invalid = invalid.saturating_add(1);
        }
        if token.token_type == TokenType::EndOfFile {
            break;
        }
    }

    (tokens, invalid)
}

fn set_default_accept_encoding(headers: &mut HeaderMap) {
"""
)

replace_once(
    http_loader,
    """    let response_body = response.body.clone();

    // We're about to spawn a future to be waited on here
""",
    """    let ladybird_audit_enabled = request.mode == RequestMode::Navigate &&
        response
            .headers
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.to_ascii_lowercase().starts_with("text/html"));
    let ladybird_webview_id = request.target_webview_id;
    let ladybird_proxy = context.state.embedder_proxy.clone();

    let response_body = response.body.clone();

    // We're about to spawn a future to be waited on here
"""
)

replace_once(
    http_loader,
    """                // This allocation may be retained by the http-cache.
                completed_body.shrink_to_fit();
                // If devtools is disabled avoid cloning, since the result would
""",
    """                // This allocation may be retained by the http-cache.
                completed_body.shrink_to_fit();

                if ladybird_audit_enabled {
                    if let Some(webview_id) = ladybird_webview_id {
                        let (token_count, invalid_count) = ladybird_document_audit(&completed_body);
                        ladybird_proxy.send(NetToEmbedderMsg::LadybirdDocumentAudit(
                            webview_id,
                            token_count,
                            invalid_count,
                        ));
                    }
                }

                // If devtools is disabled avoid cloning, since the result would
"""
)


# --- Gekko Download Manager: progress + real cancellation. ---
replace_once(
    embedder,
    """    DownloadFinished(
        WebViewId,
        PathBuf,
        u64,
        Option<String>,
    ),
    /// Ladybird shadow-parser telemetry for a real main HTML document.
""",
    """    DownloadFinished(
        WebViewId,
        PathBuf,
        u64,
        Option<String>,
    ),
    /// Progress for a native download. Total is Content-Length when known.
    DownloadProgress(
        WebViewId,
        PathBuf,
        u64,
        Option<u64>,
    ),
    /// Ladybird shadow-parser telemetry for a real main HTML document.
"""
)

replace_once(
    delegate,
    """    fn notify_download_finished(
        &self,
        _webview: WebView,
        _path: PathBuf,
        _bytes: u64,
        _error: Option<String>,
    ) {
    }

    /// Report that Ladybird tokenized the real HTML response in parallel with Servo.
""",
    """    fn notify_download_finished(
        &self,
        _webview: WebView,
        _path: PathBuf,
        _bytes: u64,
        _error: Option<String>,
    ) {
    }

    /// Report incremental native download progress.
    fn notify_download_progress(
        &self,
        _webview: WebView,
        _path: PathBuf,
        _bytes: u64,
        _total: Option<u64>,
    ) {
    }

    /// Report that Ladybird tokenized the real HTML response in parallel with Servo.
"""
)

replace_once(
    servo_rs,
    """            NetToEmbedderMsg::LadybirdDocumentAudit(webview_id, token_count, invalid_count) => {
""",
    """            NetToEmbedderMsg::DownloadProgress(webview_id, path, bytes, total) => {
                if let Some(webview) = self.get_webview_handle(webview_id) {
                    webview
                        .delegate()
                        .notify_download_progress(webview, path, bytes, total);
                }
            },
            NetToEmbedderMsg::LadybirdDocumentAudit(webview_id, token_count, invalid_count) => {
"""
)

replace_once(
    http_loader,
    """fn partial_download_path(path: &Path) -> PathBuf {
    let mut part = path.as_os_str().to_os_string();
    part.push(".part");
    PathBuf::from(part)
}
""",
    """fn partial_download_path(path: &Path) -> PathBuf {
    let mut part = path.as_os_str().to_os_string();
    part.push(".part");
    PathBuf::from(part)
}

fn cancel_download_path(path: &Path) -> PathBuf {
    let mut marker = path.as_os_str().to_os_string();
    marker.push(".part.cancel");
    PathBuf::from(marker)
}
"""
)

replace_once(
    http_loader,
    """        let chosen_path = path_receiver.await.ok().flatten();

        response.status = HttpStatus::new(StatusCode::NO_CONTENT, b"No Content".to_vec());
""",
    """        let chosen_path = path_receiver.await.ok().flatten();
        let expected_download_bytes = response
            .headers
            .get(header::CONTENT_LENGTH)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<u64>().ok());

        response.status = HttpStatus::new(StatusCode::NO_CONTENT, b"No Content".to_vec());
"""
)

replace_once(
    http_loader,
    """                    let finish_proxy = context.state.embedder_proxy.clone();
                    let error_proxy = context.state.embedder_proxy.clone();
                    let final_for_finish = final_path.clone();
                    let final_for_error = final_path.clone();
                    let temp_for_finish = temp_path.clone();
                    let temp_for_error = temp_path.clone();

                    spawn_task(
                        response_stream
                            .into_body()
                            .try_fold((file, 0_u64), move |(mut file, total), chunk| {
                                let result = file
                                    .write_all(&chunk)
                                    .map(|_| (file, total.saturating_add(chunk.len() as u64)));
                                future::ready(result)
                            })
""",
    """                    let finish_proxy = context.state.embedder_proxy.clone();
                    let error_proxy = context.state.embedder_proxy.clone();
                    let progress_proxy = context.state.embedder_proxy.clone();
                    let final_for_finish = final_path.clone();
                    let final_for_error = final_path.clone();
                    let final_for_progress = final_path.clone();
                    let temp_for_finish = temp_path.clone();
                    let temp_for_error = temp_path.clone();
                    let cancel_for_progress = cancel_download_path(&final_path);
                    let cancel_for_finish = cancel_for_progress.clone();
                    let cancel_for_error = cancel_for_progress.clone();
                    let _ = fs::remove_file(&cancel_for_progress);

                    spawn_task(
                        response_stream
                            .into_body()
                            .try_fold((file, 0_u64), move |(mut file, total), chunk| {
                                let result = (|| -> std::io::Result<_> {
                                    if cancel_for_progress.exists() {
                                        return Err(std::io::Error::new(
                                            std::io::ErrorKind::Interrupted,
                                            "download cancelled",
                                        ));
                                    }
                                    file.write_all(&chunk)?;
                                    let next = total.saturating_add(chunk.len() as u64);
                                    progress_proxy.send(NetToEmbedderMsg::DownloadProgress(
                                        webview_id,
                                        final_for_progress.clone(),
                                        next,
                                        expected_download_bytes,
                                    ));
                                    Ok((file, next))
                                })();
                                future::ready(result)
                            })
"""
)

replace_once(
    http_loader,
    """                                let result = (|| -> std::io::Result<()> {
                                    file.flush()?;
                                    file.sync_all()?;
                                    fs::rename(&temp_for_finish, &final_for_finish)?;
                                    Ok(())
                                })();

                                if let Err(error) = result {
""",
    """                                let result = (|| -> std::io::Result<()> {
                                    file.flush()?;
                                    file.sync_all()?;
                                    fs::rename(&temp_for_finish, &final_for_finish)?;
                                    let _ = fs::remove_file(&cancel_for_finish);
                                    Ok(())
                                })();

                                if let Err(error) = result {
"""
)

replace_once(
    http_loader,
    """                            .map_err(move |error| {
                                let _ = fs::remove_file(&temp_for_error);
                                error_proxy.send(NetToEmbedderMsg::DownloadFinished(
""",
    """                            .map_err(move |error| {
                                let _ = fs::remove_file(&temp_for_error);
                                let _ = fs::remove_file(&cancel_for_error);
                                error_proxy.send(NetToEmbedderMsg::DownloadFinished(
"""
)


# --- Gekko Privacy Shield V3: block third-party cookies in Servo itself. ---
replace_once(
    http_loader,
    """fn set_request_cookies(
    url: &ServoUrl,
    headers: &mut HeaderMap,
    cookie_jar: &RwLock<CookieStorage>,
) {
""",
    """fn quantic_blocks_third_party_cookies(request: &Request) -> bool {
    // A user-initiated top-level navigation must be allowed to carry that
    // destination site's first-party cookies. Frames and all subresources
    // remain subject to the same-site boundary below.
    if request.mode == RequestMode::Navigate &&
        request.destination == Destination::Document
    {
        return false;
    }

    let Origin::Origin(request_origin) = &request.origin else {
        // Privacy Shield is fail-closed when Servo has not resolved a
        // subresource origin yet.
        return true;
    };

    !is_same_site(request_origin, &request.current_url().origin())
}

fn set_request_cookies(
    url: &ServoUrl,
    headers: &mut HeaderMap,
    cookie_jar: &RwLock<CookieStorage>,
) {
"""
)

replace_once(
    http_loader,
    """        set_request_cookies(
            &current_url,
            &mut http_request.headers,
            &context.state.cookie_jar,
        );
""",
    """        if quantic_blocks_third_party_cookies(http_request) {
            http_request.headers.remove(header::COOKIE);
            debug!(
                "Gekko Privacy Shield blocked third-party request cookies for {}",
                current_url
            );
        } else {
            set_request_cookies(
                &current_url,
                &mut http_request.headers,
                &context.state.cookie_jar,
            );
        }
"""
)

replace_once(
    http_loader,
    """    if credentials_flag {
        set_cookies_from_headers(&url, &response.headers, &context.state.cookie_jar);
    }
""",
    """    if credentials_flag && !quantic_blocks_third_party_cookies(request) {
        set_cookies_from_headers(&url, &response.headers, &context.state.cookie_jar);
    } else if credentials_flag {
        debug!(
            "Gekko Privacy Shield ignored third-party Set-Cookie for {}",
            url
        );
    }
"""
)


# --- Gekko async embedder responders: allow H3 interception off the UI thread. ---
responders = SERVO / "components/servo/responders.rs"
replace_once(
    responders,
    """pub(crate) trait AbstractSender {
    type Message;
""",
    """pub(crate) trait AbstractSender: Send {
    type Message;
"""
)
replace_once(
    responders,
    """impl<T: Serialize> AbstractSender for GenericSender<T> {""",
    """impl<T: Serialize + Send> AbstractSender for GenericSender<T> {"""
)
replace_once(
    responders,
    """impl<T> AbstractSender for TokioSender<T> {""",
    """impl<T: Send> AbstractSender for TokioSender<T> {"""
)
replace_once(
    responders,
    """impl<T> AbstractSender for OneshotSender<T> {""",
    """impl<T: Send> AbstractSender for OneshotSender<T> {"""
)
replace_once(
    responders,
    """impl<T: Serialize + 'static> AutomaticResponder<T> {""",
    """impl<T: Serialize + Send + 'static> AutomaticResponder<T> {"""
)

# Validate the native graphics contract on Windows CI before compiling Servo.
# This shared patch script is executed by all three release validation workflows,
# so the same source revision is exercised on desktop and Android.
if os.environ.get("GITHUB_ACTIONS") == "true":
    print(f"GEKKO Native CI source revision: {os.environ.get('GITHUB_SHA', 'unknown')}", flush=True)
    if os.name == "nt":
        manifest = (ROOT / "fusion" / "servo-runtime" / "Cargo.toml").read_text(encoding="utf-8")
        if 'features = ["sm-no-wgl", "sm-angle-builtin"]' not in manifest:
            raise SystemExit("GEKKO Windows graphics contract failed: Surfman ANGLE with sm-no-wgl is required")
        print("GEKKO Windows graphics contract passed: ANGLE enabled; WGL disabled", flush=True)

print("Quantic Servo Gekko Fusion patch applied")
