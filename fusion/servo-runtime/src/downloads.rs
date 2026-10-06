use std::fs::{self, File};
use std::path::{Path, PathBuf};

use reqwest::blocking::Client;
use reqwest::header::{CONTENT_DISPOSITION, COOKIE, REFERER};
use url::Url;

#[derive(Debug)]
pub enum DownloadOutcome {
    Completed { path: PathBuf, bytes: u64 },
    Failed { url: String, error: String },
}

const BINARY_SUFFIXES: &[&str] = &[
    ".zip", ".7z", ".rar", ".exe", ".msi", ".msix", ".dmg", ".pkg", ".apk",
    ".deb", ".rpm", ".iso", ".tar", ".tar.gz", ".tgz", ".bz2", ".xz", ".zst",
];

pub fn is_probable_download_url(url: &Url) -> bool {
    if !matches!(url.scheme(), "http" | "https") {
        return false;
    }
    let path = url.path().to_ascii_lowercase();
    BINARY_SUFFIXES.iter().any(|suffix| path.ends_with(suffix))
}

fn fallback_filename(url: &Url) -> String {
    url.path_segments()
        .and_then(|segments| segments.filter(|part| !part.is_empty()).next_back())
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| "download.bin".into())
}

fn filename_from_content_disposition(value: &str) -> Option<String> {
    for part in value.split(';').map(str::trim) {
        if let Some(encoded) = part.strip_prefix("filename*=UTF-8''") {
            return urlencoding::decode(encoded).ok().map(|value| value.into_owned());
        }
        if let Some(raw) = part.strip_prefix("filename=") {
            let raw = raw.trim_matches('"').trim_matches(''');
            if !raw.is_empty() {
                return Some(raw.to_string());
            }
        }
    }
    None
}

fn sanitize_filename(name: &str) -> String {
    let mut clean = String::with_capacity(name.len().min(140));
    for ch in name.chars().take(140) {
        if ch.is_control() || matches!(ch, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') {
            clean.push('_');
        } else {
            clean.push(ch);
        }
    }
    let clean = clean.trim().trim_matches('.').to_string();
    if clean.is_empty() || clean == "." || clean == ".." {
        "download.bin".into()
    } else {
        clean
    }
}

fn default_download_dir() -> Option<PathBuf> {
    dirs::download_dir().or_else(|| {
        dirs::data_local_dir().map(|root| root.join("Quantic").join("Glide").join("Downloads"))
    })
}

fn unique_path(dir: &Path, filename: &str) -> PathBuf {
    let candidate = dir.join(filename);
    if !candidate.exists() {
        return candidate;
    }

    let path = Path::new(filename);
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("download");
    let ext = path.extension().and_then(|s| s.to_str());

    for index in 1..10_000 {
        let name = match ext {
            Some(ext) if !ext.is_empty() => format!("{stem} ({index}).{ext}"),
            _ => format!("{stem} ({index})"),
        };
        let candidate = dir.join(name);
        if !candidate.exists() {
            return candidate;
        }
    }

    dir.join(format!("{stem}-copy"))
}

pub fn download_to_default(
    url: Url,
    cookie_header: Option<String>,
    referrer: Option<String>,
) -> DownloadOutcome {
    let url_string = url.to_string();

    let result = (|| -> Result<(PathBuf, u64), Box<dyn std::error::Error + Send + Sync>> {
        let client = Client::builder()
            .user_agent("Quantic Glide Fusion/0.1")
            .redirect(reqwest::redirect::Policy::limited(10))
            .build()?;

        let mut request = client.get(url.clone());
        if let Some(cookie_header) = cookie_header.filter(|value| !value.is_empty()) {
            request = request.header(COOKIE, cookie_header);
        }
        if let Some(referrer) = referrer.filter(|value| !value.is_empty()) {
            request = request.header(REFERER, referrer);
        }

        let mut response = request.send()?.error_for_status()?;
        let filename = response
            .headers()
            .get(CONTENT_DISPOSITION)
            .and_then(|value| value.to_str().ok())
            .and_then(filename_from_content_disposition)
            .unwrap_or_else(|| fallback_filename(response.url()));

        let filename = sanitize_filename(&filename);
        let dir = default_download_dir().ok_or("download directory unavailable")?;
        fs::create_dir_all(&dir)?;

        let final_path = unique_path(&dir, &filename);
        let temp_path = final_path.with_extension(
            final_path
                .extension()
                .and_then(|value| value.to_str())
                .map(|ext| format!("{ext}.part"))
                .unwrap_or_else(|| "part".into()),
        );

        let mut file = File::create(&temp_path)?;
        let bytes = match response.copy_to(&mut file) {
            Ok(bytes) => bytes,
            Err(error) => {
                let _ = fs::remove_file(&temp_path);
                return Err(Box::new(error));
            }
        };
        file.sync_all()?;
        fs::rename(&temp_path, &final_path)?;

        Ok((final_path, bytes))
    })();

    match result {
        Ok((path, bytes)) => DownloadOutcome::Completed { path, bytes },
        Err(error) => DownloadOutcome::Failed {
            url: url_string,
            error: error.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_common_binary_downloads() {
        assert!(is_probable_download_url(
            &Url::parse("https://example.com/app/setup.exe").unwrap()
        ));
        assert!(is_probable_download_url(
            &Url::parse("https://example.com/archive.tar.gz").unwrap()
        ));
        assert!(!is_probable_download_url(
            &Url::parse("https://example.com/index.html").unwrap()
        ));
    }

    #[test]
    fn sanitizes_path_characters() {
        assert_eq!(sanitize_filename("../bad:name?.zip"), "_bad_name_.zip");
    }
}
