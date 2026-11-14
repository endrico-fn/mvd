use crate::domain::error::MvdError;
use crate::security::profile_guard::ValidatedSandboxProfile;
use rusqlite::{Connection, OpenFlags};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use zeroize::{Zeroize, ZeroizeOnDrop};

/// In-memory cookie row with automatic RAM zeroization on drop (`ZeroizeOnDrop`).
#[derive(Debug, Clone, Zeroize, ZeroizeOnDrop)]
pub struct EphemeralCookieRow {
    pub host: String,
    pub include_subdomains: bool,
    pub path: String,
    pub is_secure: bool,
    pub expiry: i64,
    pub name: String,
    pub value: String,
}

impl EphemeralCookieRow {
    /// Formats a single Netscape cookie jar line in RAM.
    #[must_use]
    pub fn to_netscape_line(&self) -> String {
        let include_sub = if self.include_subdomains {
            "TRUE"
        } else {
            "FALSE"
        };
        let secure = if self.is_secure { "TRUE" } else { "FALSE" };
        format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            self.host, include_sub, self.path, secure, self.expiry, self.name, self.value
        )
    }
}

/// RAII Guard for an ephemeral Netscape cookie file stored in `/run/user/$UID/mvd/vault/` (`tmpfs`).
/// Overwrites the file with `0x00` bytes and unlinks it automatically when dropped!
#[derive(Debug)]
pub struct EphemeralCookieFileGuard {
    path: PathBuf,
}

impl EphemeralCookieFileGuard {
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for EphemeralCookieFileGuard {
    fn drop(&mut self) {
        if let Ok(meta) = fs::metadata(&self.path) {
            let len = usize::try_from(meta.len()).unwrap_or(4096);
            if let Ok(mut file) = OpenOptions::new().write(true).open(&self.path) {
                let zeros = vec![0u8; len];
                let _ = file.write_all(&zeros);
                let _ = file.sync_all();
            }
        }
        let _ = fs::remove_file(&self.path);
    }
}

/// Manages zero-leak, domain-scoped cookie extraction from `pbf03` into `/run/user/$UID/mvd/vault/`.
pub struct CookieVault;

impl CookieVault {
    /// Ensures the RAM vault directory exists with strict `0700` permissions.
    pub fn init_ram_vault_dir(vault_dir: &Path) -> Result<(), MvdError> {
        fs::create_dir_all(vault_dir)?;
        let mut perms = fs::metadata(vault_dir)?.permissions();
        perms.set_mode(0o700);
        fs::set_permissions(vault_dir, perms)?;
        Ok(())
    }

    /// Queries ONLY cookies matching `target_domain_suffix` (e.g. `youtube.com`) from a SQLite connection.
    /// Unrelated cookies inside `pbf03` (e.g. from LK21 or research tabs) are NEVER extracted!
    pub fn query_domain_scoped_cookies(
        conn: &Connection,
        target_domain_suffix: &str,
    ) -> Result<Vec<EphemeralCookieRow>, MvdError> {
        let clean_domain = target_domain_suffix.trim_start_matches('.').to_lowercase();
        let dot_domain = format!(".{clean_domain}");
        let wildcard_pattern = format!("%.{clean_domain}");

        let mut stmt = conn.prepare(
            "SELECT host, path, isSecure, expiry, name, value
             FROM moz_cookies
             WHERE LOWER(host) = ?1 OR LOWER(host) = ?2 OR LOWER(host) LIKE ?3",
        )?;

        let rows = stmt.query_map(
            rusqlite::params![clean_domain, dot_domain, wildcard_pattern],
            |row| {
                let host: String = row.get(0)?;
                let include_subdomains = host.starts_with('.');
                let path: String = row.get(1)?;
                let is_secure_int: i64 = row.get(2)?;
                let expiry: i64 = row.get(3)?;
                let name: String = row.get(4)?;
                let value: String = row.get(5)?;
                Ok(EphemeralCookieRow {
                    host,
                    include_subdomains,
                    path,
                    is_secure: is_secure_int != 0,
                    expiry,
                    name,
                    value,
                })
            },
        )?;

        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    /// Extracts domain-scoped cookies from the validated `pbf03` profile into an ephemeral `0600` file
    /// inside `vault_dir` (`tmpfs`), returning an RAII guard that wipes & unlinks it on drop.
    pub fn materialize_scoped_jar_for_job(
        profile: &ValidatedSandboxProfile,
        vault_dir: &Path,
        job_id: &str,
        target_domain_suffix: &str,
    ) -> Result<EphemeralCookieFileGuard, MvdError> {
        Self::init_ram_vault_dir(vault_dir)?;

        let src_db = profile.canonical_profile_dir.join("cookies.sqlite");
        let snap_db = vault_dir.join(format!("snap-{job_id}.sqlite"));
        fs::copy(&src_db, &snap_db)?;

        // Also copy WAL if Firefox is actively open in pbf03
        let src_wal = profile.canonical_profile_dir.join("cookies.sqlite-wal");
        let snap_wal = vault_dir.join(format!("snap-{job_id}.sqlite-wal"));
        if src_wal.exists() {
            let _ = fs::copy(&src_wal, &snap_wal);
        }

        let conn = Connection::open_with_flags(
            &snap_db,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        let scoped_cookies = Self::query_domain_scoped_cookies(&conn, target_domain_suffix);
        drop(conn);
        let _ = fs::remove_file(&snap_db);
        let _ = fs::remove_file(&snap_wal);

        let cookies = scoped_cookies?;
        let jar_path = vault_dir.join(format!("jar-{job_id}.txt"));

        let mut file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .mode(0o600)
            .open(&jar_path)?;

        file.write_all(b"# Netscape HTTP Cookie File\n# Ephemeral RAM-only pbf03 scoped jar\n\n")?;
        for mut c in cookies {
            let mut line = c.to_netscape_line();
            file.write_all(line.as_bytes())?;
            line.zeroize();
            c.zeroize();
        }
        file.sync_all()?;

        Ok(EphemeralCookieFileGuard { path: jar_path })
    }
}
