use rusqlite::{Connection, Result, params};
use crate::state::{Session, ImageRecord, LogEntry};

pub fn init_db(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS sessions (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS images (
            id TEXT PRIMARY KEY,
            session_id TEXT NOT NULL,
            prompt TEXT NOT NULL,
            model TEXT NOT NULL,
            parent_image_id TEXT,
            filename TEXT NOT NULL,
            file_type TEXT NOT NULL,
            created_at TEXT NOT NULL,
            FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
        );
        CREATE TABLE IF NOT EXISTS settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS api_logs (
            id TEXT PRIMARY KEY,
            timestamp TEXT NOT NULL,
            endpoint TEXT NOT NULL,
            prompt TEXT NOT NULL,
            status TEXT NOT NULL,
            detail TEXT NOT NULL,
            duration_ms INTEGER NOT NULL
        );
        PRAGMA foreign_keys = ON;"
    )?;
    Ok(())
}

pub fn get_setting(conn: &Connection, key: &str) -> Result<Option<String>> {
    let mut stmt = conn.prepare("SELECT value FROM settings WHERE key = ?1")?;
    let mut rows = stmt.query_map(params![key], |row| row.get::<_, String>(0))?;
    match rows.next() {
        Some(row) => Ok(Some(row?)),
        None => Ok(None),
    }
}

pub fn set_setting(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

pub fn insert_session(conn: &Connection, s: &Session) -> Result<()> {
    conn.execute(
        "INSERT INTO sessions (id, name, created_at, updated_at) VALUES (?1, ?2, ?3, ?4)",
        params![s.id, s.name, s.created_at, s.updated_at],
    )?;
    Ok(())
}

pub fn get_sessions(conn: &Connection) -> Result<Vec<Session>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, created_at, updated_at FROM sessions ORDER BY updated_at DESC"
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(Session {
            id: row.get(0)?,
            name: row.get(1)?,
            created_at: row.get(2)?,
            updated_at: row.get(3)?,
        })
    })?;
    rows.collect()
}

pub fn get_session(conn: &Connection, id: &str) -> Result<Option<Session>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, created_at, updated_at FROM sessions WHERE id = ?1"
    )?;
    let mut rows = stmt.query_map(params![id], |row| {
        Ok(Session {
            id: row.get(0)?,
            name: row.get(1)?,
            created_at: row.get(2)?,
            updated_at: row.get(3)?,
        })
    })?;
    match rows.next() {
        Some(row) => Ok(Some(row?)),
        None => Ok(None),
    }
}

pub fn rename_session(conn: &Connection, id: &str, name: &str) -> Result<()> {
    conn.execute(
        "UPDATE sessions SET name = ?1, updated_at = ?2 WHERE id = ?3",
        params![name, chrono::Utc::now().to_rfc3339(), id],
    )?;
    Ok(())
}

pub fn delete_session(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("DELETE FROM images WHERE session_id = ?1", params![id])?;
    conn.execute("DELETE FROM sessions WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn insert_image(conn: &Connection, img: &ImageRecord) -> Result<()> {
    conn.execute(
        "INSERT INTO images (id, session_id, prompt, model, parent_image_id, filename, file_type, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![img.id, img.session_id, img.prompt, img.model, img.parent_image_id, img.filename, img.file_type, img.created_at],
    )?;
    conn.execute(
        "UPDATE sessions SET updated_at = ?1 WHERE id = ?2",
        params![chrono::Utc::now().to_rfc3339(), img.session_id],
    )?;
    Ok(())
}

pub fn get_images_for_session(conn: &Connection, session_id: &str) -> Result<Vec<ImageRecord>> {
    let mut stmt = conn.prepare(
        "SELECT id, session_id, prompt, model, parent_image_id, filename, file_type, created_at
         FROM images WHERE session_id = ?1 ORDER BY created_at ASC"
    )?;
    let rows = stmt.query_map(params![session_id], |row| {
        Ok(ImageRecord {
            id: row.get(0)?,
            session_id: row.get(1)?,
            prompt: row.get(2)?,
            model: row.get(3)?,
            parent_image_id: row.get(4)?,
            filename: row.get(5)?,
            file_type: row.get(6)?,
            created_at: row.get(7)?,
        })
    })?;
    rows.collect()
}

pub fn get_image(conn: &Connection, id: &str) -> Result<Option<ImageRecord>> {
    let mut stmt = conn.prepare(
        "SELECT id, session_id, prompt, model, parent_image_id, filename, file_type, created_at
         FROM images WHERE id = ?1"
    )?;
    let mut rows = stmt.query_map(params![id], |row| {
        Ok(ImageRecord {
            id: row.get(0)?,
            session_id: row.get(1)?,
            prompt: row.get(2)?,
            model: row.get(3)?,
            parent_image_id: row.get(4)?,
            filename: row.get(5)?,
            file_type: row.get(6)?,
            created_at: row.get(7)?,
        })
    })?;
    match rows.next() {
        Some(row) => Ok(Some(row?)),
        None => Ok(None),
    }
}

pub fn insert_log(conn: &Connection, log: &LogEntry) -> Result<()> {
    conn.execute(
        "INSERT INTO api_logs (id, timestamp, endpoint, prompt, status, detail, duration_ms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![log.id, log.timestamp, log.endpoint, log.prompt, log.status, log.detail, log.duration_ms],
    )?;
    Ok(())
}

pub fn get_logs(conn: &Connection) -> Result<Vec<LogEntry>> {
    let mut stmt = conn.prepare(
        "SELECT id, timestamp, endpoint, prompt, status, detail, duration_ms
         FROM api_logs ORDER BY timestamp DESC LIMIT 200"
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(LogEntry {
            id: row.get(0)?,
            timestamp: row.get(1)?,
            endpoint: row.get(2)?,
            prompt: row.get(3)?,
            status: row.get(4)?,
            detail: row.get(5)?,
            duration_ms: row.get::<_, i64>(6)? as u64,
        })
    })?;
    rows.collect()
}
