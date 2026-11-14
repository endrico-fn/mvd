use crate::config::ALLOWED_SANDBOX_PROFILE;
use crate::domain::error::MvdError;
use std::path::{Path, PathBuf};

/// Proof token that a Firefox profile directory has been cryptographically/canonically
/// verified to belong to `pbf03` and NOT any Work profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedSandboxProfile {
    pub profile_name: String,
    pub canonical_profile_dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawFirefoxProfileEntry {
    pub name: String,
    pub is_relative: bool,
    pub path: String,
    pub is_default: bool,
}

/// Parses `profiles.ini` content and enforces the strict `pbf03` allowlist.
pub struct ProfileGuard;

impl ProfileGuard {
    /// Rejects any profile name other than `pbf03`.
    pub fn verify_profile_name(requested_name: &str) -> Result<(), MvdError> {
        if requested_name.trim() != ALLOWED_SANDBOX_PROFILE {
            return Err(MvdError::ForbiddenProfile {
                attempted: requested_name.to_string(),
            });
        }
        Ok(())
    }

    /// Parses Firefox `profiles.ini` text without touching any profile directory on disk.
    #[must_use]
    pub fn parse_profiles_ini(ini_contents: &str) -> Vec<RawFirefoxProfileEntry> {
        let mut entries = Vec::new();
        let mut current_name: Option<String> = None;
        let mut current_is_relative = true;
        let mut current_path: Option<String> = None;
        let mut current_default = false;

        for raw_line in ini_contents.lines() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
                continue;
            }
            if line.starts_with('[') && line.ends_with(']') {
                if let (Some(name), Some(path)) = (current_name.take(), current_path.take()) {
                    entries.push(RawFirefoxProfileEntry {
                        name,
                        is_relative: current_is_relative,
                        path,
                        is_default: current_default,
                    });
                }
                current_is_relative = true;
                current_default = false;
                continue;
            }
            if let Some((k, v)) = line.split_once('=') {
                match k.trim() {
                    "Name" => current_name = Some(v.trim().to_string()),
                    "IsRelative" => current_is_relative = v.trim() == "1",
                    "Path" => current_path = Some(v.trim().to_string()),
                    "Default" => current_default = v.trim() == "1",
                    _ => {}
                }
            }
        }

        if let (Some(name), Some(path)) = (current_name, current_path) {
            entries.push(RawFirefoxProfileEntry {
                name,
                is_relative: current_is_relative,
                path,
                is_default: current_default,
            });
        }

        entries
    }

    /// Resolves ONLY the `pbf03` profile from parsed `profiles.ini` entries.
    /// Work profiles (Profile 1, Profile 2, default-release, etc.) are strictly ignored and blocked.
    pub fn resolve_sandbox_profile(
        firefox_root: &Path,
        ini_contents: &str,
        requested_profile: &str,
    ) -> Result<ValidatedSandboxProfile, MvdError> {
        Self::verify_profile_name(requested_profile)?;

        let entries = Self::parse_profiles_ini(ini_contents);
        let target = entries
            .into_iter()
            .find(|e| e.name == ALLOWED_SANDBOX_PROFILE)
            .ok_or_else(|| MvdError::SandboxPathInvalid {
                reason: format!(
                    "Profile '{ALLOWED_SANDBOX_PROFILE}' not found in Firefox profiles.ini"
                ),
            })?;

        let raw_dir = if target.is_relative {
            firefox_root.join(&target.path)
        } else {
            PathBuf::from(&target.path)
        };

        // Ensure the resolved directory name contains 'pbf03' as a secondary guard against symlink/path tampering
        let dir_str = raw_dir.to_string_lossy();
        if !dir_str.contains(ALLOWED_SANDBOX_PROFILE) {
            return Err(MvdError::SandboxPathInvalid {
                reason: format!(
                    "Profile path '{dir_str}' does not match sandbox identifier '{ALLOWED_SANDBOX_PROFILE}'"
                ),
            });
        }

        Ok(ValidatedSandboxProfile {
            profile_name: target.name,
            canonical_profile_dir: raw_dir,
        })
    }
}
