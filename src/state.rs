use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::db;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageRecord {
    pub id: String,
    pub session_id: String,
    pub prompt: String,
    pub model: String,
    pub parent_image_id: Option<String>,
    pub filename: String,
    pub file_type: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub id: String,
    pub timestamp: String,
    pub endpoint: String,
    pub prompt: String,
    pub status: String,
    pub detail: String,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GenerationStatus {
    Idle,
    Generating,
    Error(String),
}

pub const STYLES: &[(&str, &str)] = &[
    ("vector_illustration/flat_2", "Flat"),
    ("vector_illustration/line_art", "Line Art"),
    ("vector_illustration/cartoon", "Cartoon"),
    ("vector_illustration/linocut", "Linocut"),
    ("vector_illustration/doodle_line_art", "Doodle"),
    ("vector_illustration/engraving", "Engraving"),
];

pub struct AppState {
    pub db: Arc<Mutex<Connection>>,
    pub images_path: PathBuf,
    pub sessions: Vec<Session>,
    pub active_session_id: Option<String>,
    pub session_images: Vec<ImageRecord>,
    pub selected_image_id: Option<String>,
    pub selected_style_idx: usize,
    pub status: GenerationStatus,
    pub fal_key: Option<String>,
    pub logs: Vec<LogEntry>,
}

impl AppState {
    pub fn new(db_path: &str, images_path: PathBuf) -> Self {
        std::fs::create_dir_all(&images_path).ok();
        if let Some(parent) = PathBuf::from(db_path).parent() {
            std::fs::create_dir_all(parent).ok();
        }

        let conn = Connection::open(db_path).expect("Failed to open database");
        db::init_db(&conn).expect("Failed to init database");

        let sessions = db::get_sessions(&conn).unwrap_or_default();
        let fal_key = db::get_setting(&conn, "fal_key").unwrap_or(None);

        let logs = db::get_logs(&conn).unwrap_or_default();

        Self {
            db: Arc::new(Mutex::new(conn)),
            images_path,
            sessions,
            active_session_id: None,
            session_images: Vec::new(),
            selected_image_id: None,
            selected_style_idx: 0,
            status: GenerationStatus::Idle,
            fal_key,
            logs,
        }
    }

    pub fn reload_sessions(&mut self) {
        let conn = self.db.lock().unwrap();
        self.sessions = db::get_sessions(&conn).unwrap_or_default();
    }

    pub fn create_session(&mut self, name: &str) -> Session {
        let session = Session {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
            updated_at: chrono::Utc::now().to_rfc3339(),
        };
        {
            let conn = self.db.lock().unwrap();
            db::insert_session(&conn, &session).ok();
        }
        self.sessions.insert(0, session.clone());
        session
    }

    pub fn select_session(&mut self, id: &str) {
        self.active_session_id = Some(id.to_string());
        self.selected_image_id = None;
        let conn = self.db.lock().unwrap();
        self.session_images = db::get_images_for_session(&conn, id).unwrap_or_default();
    }

    pub fn delete_session(&mut self, id: &str) {
        let conn = self.db.lock().unwrap();
        // Delete image files
        if let Ok(images) = db::get_images_for_session(&conn, id) {
            for img in images {
                let path = self.images_path.join(&img.filename);
                let _ = std::fs::remove_file(path);
            }
        }
        db::delete_session(&conn, id).ok();
        drop(conn);
        self.sessions.retain(|s| s.id != id);
        if self.active_session_id.as_deref() == Some(id) {
            self.active_session_id = None;
            self.session_images.clear();
            self.selected_image_id = None;
        }
    }

    pub fn rename_session(&mut self, id: &str, name: &str) {
        let conn = self.db.lock().unwrap();
        db::rename_session(&conn, id, name).ok();
        drop(conn);
        if let Some(s) = self.sessions.iter_mut().find(|s| s.id == id) {
            s.name = name.to_string();
        }
    }

    pub fn save_fal_key(&mut self, key: &str) {
        let conn = self.db.lock().unwrap();
        db::set_setting(&conn, "fal_key", key).ok();
        drop(conn);
        self.fal_key = Some(key.to_string());
    }

    pub fn add_image(&mut self, img: ImageRecord) {
        let conn = self.db.lock().unwrap();
        db::insert_image(&conn, &img).ok();
        drop(conn);
        self.session_images.push(img.clone());
        self.selected_image_id = Some(img.id);
        self.reload_sessions();
    }

    pub fn add_log(&mut self, log: LogEntry) {
        let conn = self.db.lock().unwrap();
        db::insert_log(&conn, &log).ok();
        drop(conn);
        self.logs.insert(0, log);
    }

    pub fn selected_image(&self) -> Option<&ImageRecord> {
        self.selected_image_id
            .as_ref()
            .and_then(|id| self.session_images.iter().find(|i| &i.id == id))
    }

    pub fn active_session(&self) -> Option<&Session> {
        self.active_session_id
            .as_ref()
            .and_then(|id| self.sessions.iter().find(|s| &s.id == id))
    }

    pub fn style_value(&self) -> &str {
        STYLES[self.selected_style_idx].0
    }

    pub fn next_style(&mut self) {
        self.selected_style_idx = (self.selected_style_idx + 1) % STYLES.len();
    }
}
