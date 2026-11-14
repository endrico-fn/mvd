use crate::domain::error::MvdError;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryRecord {
    pub id: i64,
    pub platform: String,
    pub media_id: String,
    pub normalized_url: String,
    pub title: String,
    pub container: String,
    pub output_path: String,
    pub file_size_bytes: u64,
    pub completed_at_epoch: u64,
}

/// SQLite WAL History & Deduplication Ledger (`$XDG_DATA_HOME/mvd/history.db`).
///
/// SAFETY INVARIANT: This module has ZERO filesystem deletion calls (`fs::remove_file`).
/// Clearing history ONLY removes SQLite database rows; completed media files in `~/Downloads/mvd/`
/// are NEVER deleted.
pub struct HistoryLedger {
    conn: Connection,
}

impl HistoryLedger {
    pub fn open(db_path: &Path) -> Result<Self, MvdError> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(db_path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        let ledger = Self { conn };
        ledger.init_schema()?;
        Ok(ledger)
    }

    pub fn open_in_memory() -> Result<Self, MvdError> {
        let conn = Connection::open_in_memory()?;
        let ledger = Self { conn };
        ledger.init_schema()?;
        Ok(ledger)
    }

    fn init_schema(&self) -> Result<(), MvdError> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS downloads (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                platform TEXT NOT NULL,
                media_id TEXT NOT NULL,
                normalized_url TEXT NOT NULL,
                title TEXT NOT NULL,
                container TEXT NOT NULL,
                output_path TEXT NOT NULL,
                file_size_bytes INTEGER NOT NULL,
                completed_at_epoch INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_downloads_media_id ON downloads(platform, media_id);
            CREATE INDEX IF NOT EXISTS idx_downloads_url ON downloads(normalized_url);",
        )?;
        Ok(())
    }

    pub fn record_completed(
        &self,
        platform: &str,
        media_id: &str,
        normalized_url: &str,
        title: &str,
        container: &str,
        output_path: &str,
        file_size_bytes: u64,
        completed_at_epoch: u64,
    ) -> Result<(), MvdError> {
        self.conn.execute(
            "INSERT INTO downloads (
                platform, media_id, normalized_url, title, container, output_path, file_size_bytes, completed_at_epoch
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                platform,
                media_id,
                normalized_url,
                title,
                container,
                output_path,
                i64::try_from(file_size_bytes).unwrap_or(0),
                i64::try_from(completed_at_epoch).unwrap_or(0),
            ],
        )?;
        Ok(())
    }

    pub fn is_duplicate(
        &self,
        platform: &str,
        media_id: &str,
        normalized_url: &str,
    ) -> Result<bool, MvdError> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(1) FROM downloads WHERE (platform = ?1 AND media_id = ?2) OR normalized_url = ?3",
            params![platform, media_id, normalized_url],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    pub fn list_recent(&self, limit: usize) -> Result<Vec<HistoryRecord>, MvdError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, platform, media_id, normalized_url, title, container, output_path, file_size_bytes, completed_at_epoch
             FROM downloads
             ORDER BY completed_at_epoch DESC, id DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![i64::try_from(limit).unwrap_or(100)], |row| {
            let file_size: i64 = row.get(7)?;
            let epoch: i64 = row.get(8)?;
            Ok(HistoryRecord {
                id: row.get(0)?,
                platform: row.get(1)?,
                media_id: row.get(2)?,
                normalized_url: row.get(3)?,
                title: row.get(4)?,
                container: row.get(5)?,
                output_path: row.get(6)?,
                file_size_bytes: u64::try_from(file_size).unwrap_or(0),
                completed_at_epoch: u64::try_from(epoch).unwrap_or(0),
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    /// Removes a single history ledger entry without touching `~/Downloads/mvd/`.
    pub fn delete_record_only(&self, record_id: i64) -> Result<usize, MvdError> {
        let deleted = self
            .conn
            .execute("DELETE FROM downloads WHERE id = ?1", params![record_id])?;
        Ok(deleted)
    }

    /// Clears all history ledger records without touching any downloaded media files on disk.
    pub fn clear_all_records_only(&self) -> Result<usize, MvdError> {
        let deleted = self.conn.execute("DELETE FROM downloads", [])?;
        Ok(deleted)
    }
}
