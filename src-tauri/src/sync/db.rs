use anyhow::Result;
use rusqlite::{params, Connection};
use serde::Serialize;
use std::path::Path;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS sync_records (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    profile_id TEXT NOT NULL,
    local_path TEXT,
    asset_id TEXT,
    file_name TEXT NOT NULL,
    content_hash TEXT,
    sync_status TEXT NOT NULL DEFAULT 'synced',
    direction TEXT NOT NULL,
    file_size_bytes INTEGER,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    last_synced_at TEXT
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_upload ON sync_records(profile_id, local_path) WHERE local_path IS NOT NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_download ON sync_records(profile_id, asset_id) WHERE asset_id IS NOT NULL;
CREATE TABLE IF NOT EXISTS activity_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    profile_id TEXT NOT NULL,
    occurred_at TEXT NOT NULL,
    event_type TEXT NOT NULL,
    file_name TEXT NOT NULL,
    local_path TEXT,
    asset_id TEXT,
    file_size INTEGER,
    message TEXT NOT NULL
);
";

pub fn open(path: &Path) -> Result<Connection> {
    let conn = Connection::open(path)?;
    conn.execute_batch(SCHEMA)?;
    Ok(conn)
}

pub fn is_uploaded(conn: &Connection, profile_id: &str, hash: &str) -> Result<bool> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sync_records WHERE profile_id=?1 AND content_hash=?2 AND direction='upload'",
        params![profile_id, hash],
        |r| r.get(0),
    )?;
    Ok(count > 0)
}

pub fn has_asset(conn: &Connection, profile_id: &str, asset_id: &str) -> Result<bool> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sync_records WHERE profile_id=?1 AND asset_id=?2",
        params![profile_id, asset_id],
        |r| r.get(0),
    )?;
    Ok(count > 0)
}

pub fn upsert_upload(
    conn: &Connection,
    profile_id: &str,
    local_path: &str,
    asset_id: &str,
    hash: &str,
    file_size: u64,
    file_name: &str,
) -> Result<()> {
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO sync_records (profile_id, local_path, asset_id, file_name, content_hash, direction, file_size_bytes, created_at, updated_at, last_synced_at)
         VALUES (?1,?2,?3,?4,?5,'upload',?6,?7,?7,?7)
         ON CONFLICT(profile_id, local_path) DO UPDATE SET
           asset_id=excluded.asset_id, content_hash=excluded.content_hash,
           file_size_bytes=excluded.file_size_bytes, updated_at=excluded.updated_at, last_synced_at=excluded.last_synced_at",
        params![profile_id, local_path, asset_id, file_name, hash, file_size as i64, now],
    )?;
    Ok(())
}

pub fn upsert_download(
    conn: &Connection,
    profile_id: &str,
    asset_id: &str,
    local_path: &str,
    file_name: &str,
    file_size: i64,
) -> Result<()> {
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO sync_records (profile_id, local_path, asset_id, file_name, direction, file_size_bytes, created_at, updated_at, last_synced_at)
         VALUES (?1,?2,?3,?4,'download',?5,?6,?6,?6)
         ON CONFLICT(profile_id, asset_id) DO UPDATE SET
           local_path=excluded.local_path, file_size_bytes=excluded.file_size_bytes,
           updated_at=excluded.updated_at, last_synced_at=excluded.last_synced_at",
        params![profile_id, local_path, asset_id, file_name, file_size, now],
    )?;
    Ok(())
}

pub fn log_activity(
    conn: &Connection,
    profile_id: &str,
    event_type: &str,
    file_name: &str,
    local_path: Option<&str>,
    asset_id: Option<&str>,
    file_size: Option<i64>,
    message: &str,
) -> Result<()> {
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO activity_log (profile_id, occurred_at, event_type, file_name, local_path, asset_id, file_size, message)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
        params![profile_id, now, event_type, file_name, local_path, asset_id, file_size, message],
    )?;
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityEntry {
    pub occurred_at: String,
    pub event_type: String,
    pub file_name: String,
    pub message: String,
}

pub fn get_recent_activity(
    conn: &Connection,
    profile_id: &str,
    limit: i64,
) -> Result<Vec<ActivityEntry>> {
    let mut stmt = conn.prepare(
        "SELECT occurred_at, event_type, file_name, message FROM activity_log
         WHERE profile_id=?1 ORDER BY occurred_at DESC LIMIT ?2",
    )?;
    let entries = stmt
        .query_map(params![profile_id, limit], |r| {
            Ok(ActivityEntry {
                occurred_at: r.get(0)?,
                event_type: r.get(1)?,
                file_name: r.get(2)?,
                message: r.get(3)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(entries)
}
