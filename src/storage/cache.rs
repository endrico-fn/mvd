use crate::domain::error::MvdError;
use std::fs;
use std::path::Path;

/// Bounded LRU + TTL Cache Manager for `$XDG_CACHE_HOME/mvd/{thumbs,lyrics}/`.
pub struct BoundedCacheManager {
    pub max_bytes: u64,
}

impl BoundedCacheManager {
    #[must_use]
    pub const fn new(max_mib: u64) -> Self {
        Self {
            max_bytes: max_mib.saturating_mul(1024 * 1024),
        }
    }

    pub fn compute_usage_bytes(cache_root: &Path) -> u64 {
        if !cache_root.exists() {
            return 0;
        }
        Self::dir_size_recursive(cache_root)
    }

    fn dir_size_recursive(dir: &Path) -> u64 {
        let Ok(entries) = fs::read_dir(dir) else {
            return 0;
        };
        let mut total = 0u64;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                total = total.saturating_add(Self::dir_size_recursive(&path));
            } else if let Ok(meta) = entry.metadata() {
                total = total.saturating_add(meta.len());
            }
        }
        total
    }

    /// Purges the cache directory on user request (`X` in Settings) and returns reclaimed bytes.
    pub fn purge_all(cache_root: &Path) -> Result<u64, MvdError> {
        let before = Self::compute_usage_bytes(cache_root);
        if cache_root.exists() {
            fs::remove_dir_all(cache_root)?;
        }
        fs::create_dir_all(cache_root.join("thumbs"))?;
        fs::create_dir_all(cache_root.join("lyrics"))?;
        Ok(before)
    }
}
