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
    r'''fn suggested_download_filename(headers: &HeaderMap) -> Option<String> {
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
        let suggested_filename = suggested_download_filename(&response.headers);
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

                                let error = result.err().map(|error| error.to_string());
                                finish_proxy.send(NetToEmbedderMsg::DownloadFinished(
                                    webview_id,
                                    final_for_finish,
                                    total,
                                    error,
                                ));
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

print("Quantic Servo native-download patch applied")
