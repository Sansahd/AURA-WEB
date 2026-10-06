use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use url::Url;

use crate::paths;

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
    paths::download_dir()
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


pub fn partial_path(path: &Path) -> PathBuf {
    let mut value = path.as_os_str().to_os_string();
    value.push(".part");
    PathBuf::from(value)
}

pub fn cancel_marker(path: &Path) -> PathBuf {
    let mut value = path.as_os_str().to_os_string();
    value.push(".part.cancel");
    PathBuf::from(value)
}

pub fn request_cancel(path: &Path) -> std::io::Result<()> {
    let marker = cancel_marker(path);
    if let Some(parent) = marker.parent() {
        fs::create_dir_all(parent)?;
    }
    File::create(marker)?.write_all(b"cancel")?;
    Ok(())
}

pub fn clear_cancel_marker(path: &Path) {
    let _ = fs::remove_file(cancel_marker(path));
}

pub fn reveal_in_folder(path: &Path) -> std::io::Result<()> {
    #[cfg(target_os = "windows")]
    {
        Command::new("explorer")
            .arg(format!("/select,{}", path.display()))
            .spawn()?;
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open").arg("-R").arg(path).spawn()?;
        return Ok(());
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let target = path.parent().unwrap_or(path);
        Command::new("xdg-open").arg(target).spawn()?;
        return Ok(());
    }

    #[allow(unreachable_code)]
    Ok(())
}

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

    #[test]
    fn cancellation_marker_is_sidecar_of_partial_file() {
        let path = PathBuf::from("report.zip");
        assert_eq!(
            cancel_marker(&path).file_name().and_then(|name| name.to_str()),
            Some("report.zip.part.cancel")
        );
        assert_eq!(
            partial_path(&path).file_name().and_then(|name| name.to_str()),
            Some("report.zip.part")
        );
    }
}
