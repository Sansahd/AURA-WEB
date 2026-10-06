use std::fs;
use std::path::{Path, PathBuf};

use url::Url;

fn fallback_filename(url: &Url) -> String {
    url.path_segments()
        .and_then(|segments| segments.filter(|part| !part.is_empty()).next_back())
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| "download.bin".into())
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

pub fn destination_path(url: &Url, suggested_filename: Option<&str>) -> Option<PathBuf> {
    let filename = suggested_filename
        .filter(|value| !value.trim().is_empty())
        .map(sanitize_filename)
        .unwrap_or_else(|| sanitize_filename(&fallback_filename(url)));

    let dir = default_download_dir()?;
    fs::create_dir_all(&dir).ok()?;
    Some(unique_path(&dir, &filename))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_path_characters() {
        assert_eq!(sanitize_filename("../bad:name?.zip"), "_bad_name_.zip");
        assert_eq!(sanitize_filename("report.pdf"), "report.pdf");
    }

    #[test]
    fn derives_a_name_from_extensionless_urls() {
        let url = Url::parse("https://example.com/export?id=42").unwrap();
        assert_eq!(fallback_filename(&url), "export");
    }
}
