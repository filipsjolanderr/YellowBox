use crate::error::Result;
use crate::models::{MemoryItem, ProcessingState};
use rusqlite::{params, Connection};
use std::path::Path;
use std::str::FromStr;
use std::sync::Mutex;

pub trait MemoryRepository: Send + Sync {
    fn insert_or_ignore_memory(&self, item: &MemoryItem) -> Result<()>;
    fn update_state(
        &self,
        id: &str,
        state: ProcessingState,
        error_message: Option<&str>,
        extension: Option<String>,
        has_overlay: Option<bool>,
    ) -> Result<()>;
    fn get_all_memories(&self) -> Result<Vec<MemoryItem>>;
}

pub struct DbManager {
    conn: Mutex<Connection>,
}

impl DbManager {
    /// Opens the SQLite database at the specified path (usually inside the export directory)
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let conn = Connection::open(path)?;
        Self::create_schema(&conn)?;
        Ok(DbManager {
            conn: Mutex::new(conn),
        })
    }

    /// Creates the memories table if it doesn't already exist
    fn create_schema(conn: &Connection) -> Result<()> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS memories (
                id TEXT PRIMARY KEY,
                download_url TEXT NOT NULL,
                original_date TEXT NOT NULL,
                location TEXT,
                state TEXT NOT NULL,
                error_message TEXT,
                extension TEXT,
                has_overlay INTEGER DEFAULT 0,
                media_type TEXT
            )",
            [],
        )?;
        Ok(())
    }
}
impl MemoryRepository for DbManager {
    /// Inserts a new memory item or ignores it if the ID already exists
    fn insert_or_ignore_memory(&self, item: &MemoryItem) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO memories (id, download_url, original_date, location, state, error_message, extension, has_overlay, media_type)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                item.id,
                item.download_url,
                item.original_date,
                item.location,
                item.state.as_ref(), // Use strum's AsRefStr
                item.error_message,
                item.extension,
                item.has_overlay as i32,
                item.media_type
            ],
        )?;
        Ok(())
    }

    /// Updates the processing state and potentially an error message for an item
    fn update_state(
        &self,
        id: &str,
        state: ProcessingState,
        error_message: Option<&str>,
        extension: Option<String>,
        has_overlay: Option<bool>,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE memories SET 
                state = ?1, 
                error_message = ?2, 
                extension = COALESCE(?3, extension),
                has_overlay = COALESCE(?4, has_overlay)
             WHERE id = ?5",
            params![
                state.as_ref(),
                error_message,
                extension,
                has_overlay.map(|b| b as i32),
                id
            ],
        )?;

        Ok(())
    }

    /// Retrieves all memory items
    fn get_all_memories(&self) -> Result<Vec<MemoryItem>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id, download_url, original_date, location, state, error_message, extension, has_overlay, media_type FROM memories")?;
        let memories = stmt
            .query_map([], |row| {
                let state_str: String = row.get(4)?;
                Ok(MemoryItem {
                    id: row.get(0)?,
                    download_url: row.get(1)?,
                    original_date: row.get(2)?,
                    location: row.get(3)?,
                    state: ProcessingState::from_str(&state_str)
                        .unwrap_or(ProcessingState::Pending),
                    error_message: row.get(5)?,
                    extension: row.get(6)?,
                    has_overlay: row.get::<_, i32>(7)? != 0,
                    media_type: row.get(8)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(memories)
    }
}
