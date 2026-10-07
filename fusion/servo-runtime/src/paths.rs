use std::path::PathBuf;

pub fn portable_mode() -> bool {
    if std::env::var("GEKKO_PORTABLE").ok().as_deref() == Some("1") {
        return true;
    }
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("portable.flag")))
        .is_some_and(|marker| marker.exists())
}

pub fn data_root() -> Option<PathBuf> {
    if portable_mode() {
        return std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|dir| dir.join("GekkoData")));
    }
    dirs::data_local_dir().map(|root| root.join("Quantic").join("Gekko"))
}

pub fn browser_data_file() -> Option<PathBuf> {
    data_root().map(|root| root.join("browser-data.json"))
}

pub fn encrypted_sync_file() -> Option<PathBuf> {
    data_root().map(|root| root.join("GEKKO-Sync.gekko-sync"))
}

pub fn legacy_browser_data_file() -> Option<PathBuf> {
    dirs::data_local_dir().map(|root| {
        root.join("Quantic")
            .join("Glide")
            .join("browser-data.json")
    })
}

pub fn download_dir() -> Option<PathBuf> {
    if portable_mode() {
        return data_root().map(|root| root.join("Downloads"));
    }
    dirs::download_dir().or_else(|| data_root().map(|root| root.join("Downloads")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_data_root_uses_gekko_identity() {
        if !portable_mode() {
            let root = data_root().expect("data root");
            assert!(root.to_string_lossy().contains("Gekko"));
        }
    }

    #[test]
    fn legacy_path_is_glide_only_for_migration() {
        let path = legacy_browser_data_file().expect("legacy path");
        assert!(path.to_string_lossy().contains("Glide"));
    }
}
