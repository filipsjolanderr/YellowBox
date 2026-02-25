use crate::models::{MemoryItem, ProcessingState};
use rusqlite::{params, Connection, Result};
use std::path::Path;

pub struct DbManager {
    conn: Connection,
}

impl DbManager {
    /// Opens the SQLite database at the specified path (usually inside the export directory)
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let conn = Connection::open(path)?;
        Self::create_schema(&conn)?;
        Ok(DbManager { conn })
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

    /// Inserts a new memory item or ignores it if the ID already exists
    pub fn insert_or_ignore_memory(&self, item: &MemoryItem) -> Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO memories (id, download_url, original_date, location, state, error_message, extension, has_overlay, media_type)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                item.id,
                item.download_url,
                item.original_date,
                item.location,
                item.state.as_str(),
                item.error_message,
                item.extension,
                item.has_overlay as i32,
                item.media_type
            ],
        )?;
        Ok(())
    }

    /// Updates the processing state and potentially an error message for an item
    pub fn update_state(
        &self,
        id: &str,
        state: ProcessingState,
        error_message: Option<&str>,
        extension: Option<String>,
        has_overlay: Option<bool>,
    ) -> Result<()> {
        // Simple update with fixed params is safer than building query strings if we know the schema
        // Since sqlite params are positional, we use COALESCE to keep existing values when None is provided.

        self.conn.execute(
            "UPDATE memories SET 
                state = ?1, 
                error_message = ?2, 
                extension = COALESCE(?3, extension),
                has_overlay = COALESCE(?4, has_overlay)
             WHERE id = ?5",
            params![
                state.as_str(),
                error_message,
                extension,
                has_overlay.map(|b| b as i32),
                id
            ],
        )?;

        Ok(())
    }

    /// Retrieves all memory items
    pub fn get_all_memories(&self) -> Result<Vec<MemoryItem>> {
        let mut stmt = self.conn.prepare("SELECT id, download_url, original_date, location, state, error_message, extension, has_overlay, media_type FROM memories")?;
        let memories = stmt
            .query_map([], |row| {
                Ok(MemoryItem {
                    id: row.get(0)?,
                    download_url: row.get(1)?,
                    original_date: row.get(2)?,
                    location: row.get(3)?,
                    state: ProcessingState::from_str(&row.get::<_, String>(4)?),
                    error_message: row.get(5)?,
                    extension: row.get(6)?,
                    has_overlay: row.get::<_, i32>(7)? != 0,
                    media_type: row.get(8)?,
                })
            })?
            .filter_map(Result::ok)
            .collect();
        Ok(memories)
    }
}
