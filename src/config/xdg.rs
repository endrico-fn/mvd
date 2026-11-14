use std::path::PathBuf;

/// Strict XDG Base Directory paths for `mvd` (`[!] DO NOT POLLUTE THE SYSTEM`).
#[derive(Debug, Clone)]
pub struct XdgPaths {
    pub config_file: PathBuf,
    pub data_history_db: PathBuf,
    pub state_queue_json: PathBuf,
    pub cache_root: PathBuf,
    pub cache_thumbs: PathBuf,
    pub cache_lyrics: PathBuf,
    pub runtime_cookie_vault: PathBuf,
    pub download_root: PathBuf,
}

impl XdgPaths {
    #[must_use]
    pub fn resolve() -> Self {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/tmp"));

        let config_base = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".config"));

        let data_base = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".local").join("share"));

        let state_base = std::env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".local").join("state"));

        let cache_base = std::env::var_os("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".cache"));

        // Ephemeral RAM-backed tmpfs directory (/run/user/$UID/mvd/vault)
        let runtime_base = std::env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/dev/shm"));

        let cache_root = cache_base.join("mvd");

        Self {
            config_file: config_base.join("mvd").join("config.toml"),
            data_history_db: data_base.join("mvd").join("history.db"),
            state_queue_json: state_base.join("mvd").join("queue.json"),
            cache_thumbs: cache_root.join("thumbs"),
            cache_lyrics: cache_root.join("lyrics"),
            cache_root,
            runtime_cookie_vault: runtime_base.join("mvd").join("vault"),
            download_root: home.join("Downloads").join("mvd"),
        }
    }
}
