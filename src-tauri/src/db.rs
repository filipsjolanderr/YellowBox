use crate::error::Result;
use crate::models::{MemoryItem, ProcessingState};
use std::path::Path;
use std::str::FromStr;
use tokio_rusqlite::Connection;

pub trait MemoryRepository: Send + Sync {
    fn insert_or_ignore_memory(&self, item: &MemoryItem) -> impl std::future::Future<Output = Result<()>> + Send;
    fn update_state(
        &self,
        id: &str,
        state: ProcessingState,
        error_message: Option<&str>,
        extension: Option<String>,
        has_overlay: Option<bool>,
        has_thumbnail: Option<bool>,
    ) -> impl std::future::Future<Output = Result<()>> + Send;
    fn get_all_memories(&self) -> impl std::future::Future<Output = Result<Vec<MemoryItem>>> + Send;
    fn get_memories_by_state(&self, state: ProcessingState) -> impl std::future::Future<Output = Result<Vec<MemoryItem>>> + Send;
    fn update_states(
        &self,
        from_state: ProcessingState,
        to_state: ProcessingState,
    ) -> impl std::future::Future<Output = Result<()>> + Send;
    fn reset_item_state(&self, id: &str) -> impl std::future::Future<Output = Result<()>> + Send;
}

pub struct DbManager {
    conn: Connection,
}

impl DbManager {
    /// Opens the SQLite database at the specified path (usually inside the export directory)
    pub async fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let conn = Connection::open(path).await.map_err(|e| crate::error::AppError::Database(e.into()))?;
        
        conn.call(|c| {
            c.execute_batch(
                "PRAGMA journal_mode = WAL;
                 PRAGMA synchronous = NORMAL;",
            )
        }).await.map_err(|e| crate::error::AppError::Database(e.into()))?;
        
        Self::create_schema(&conn).await?;
        Ok(DbManager { conn })
    }

    /// Creates the memories table if it doesn't already exist
    async fn create_schema(conn: &Connection) -> Result<()> {
        conn.call(|c| {
            c.execute(
                "CREATE TABLE IF NOT EXISTS memories (
                    id TEXT PRIMARY KEY,
                    download_url TEXT NOT NULL,
                    original_date TEXT NOT NULL,
                    location TEXT,
                    state TEXT NOT NULL,
                    error_message TEXT,
                    extension TEXT,
                    has_overlay INTEGER DEFAULT 0,
                    has_thumbnail INTEGER DEFAULT 0,
                    media_type TEXT,
                    segment_ids TEXT
                )",
                [],
            )
        }).await.map_err(|e| crate::error::AppError::Database(e.into()))?;
        // Migration: add segment_ids if missing (ignore if already exists)
        let _ = conn
            .call(|c| c.execute("ALTER TABLE memories ADD COLUMN segment_ids TEXT", []))
            .await;
        Ok(())
    }
}
impl MemoryRepository for DbManager {
    /// Inserts a new memory item or ignores it if the ID already exists
    async fn insert_or_ignore_memory(&self, item: &MemoryItem) -> Result<()> {
        let cloned_item = item.clone();
        let segment_ids_json = cloned_item
            .segment_ids
            .as_ref()
            .and_then(|s| serde_json::to_string(s).ok());

        self.conn.call(move |conn| {
            conn.execute(
                "INSERT OR IGNORE INTO memories (id, download_url, original_date, location, state, error_message, extension, has_overlay, has_thumbnail, media_type, segment_ids)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                rusqlite::params![
                    cloned_item.id,
                    cloned_item.download_url,
                    cloned_item.original_date,
                    cloned_item.location,
                    cloned_item.state.as_ref(),
                    cloned_item.error_message,
                    cloned_item.extension,
                    cloned_item.has_overlay as i32,
                    cloned_item.has_thumbnail as i32,
                    cloned_item.media_type,
                    segment_ids_json
                ],
            )
        }).await.map_err(|e| crate::error::AppError::Database(e.into()))?;
        Ok(())
    }

    /// Updates the processing state and potentially an error message for an item
    async fn update_state(
        &self,
        id: &str,
        state: ProcessingState,
        error_message: Option<&str>,
        extension: Option<String>,
        has_overlay: Option<bool>,
        has_thumbnail: Option<bool>,
    ) -> Result<()> {
        let cloned_id = id.to_string();
        let cloned_state = state.clone();
        let cloned_err = error_message.map(|s| s.to_string());
        let cloned_ext = extension.clone();
        let cloned_overlay = has_overlay.clone();
        let cloned_thumb = has_thumbnail.clone();

        self.conn.call(move |conn| {
            conn.execute(
                "UPDATE memories SET 
                    state = ?1, 
                    error_message = ?2, 
                    extension = COALESCE(?3, extension),
                    has_overlay = COALESCE(?4, has_overlay),
                    has_thumbnail = COALESCE(?5, has_thumbnail)
                 WHERE id = ?6",
                rusqlite::params![
                    cloned_state.as_ref(),
                    cloned_err,
                    cloned_ext,
                    cloned_overlay.map(|b| b as i32),
                    cloned_thumb.map(|b| b as i32),
                    cloned_id
                ],
            )
        }).await.map_err(|e| crate::error::AppError::Database(e.into()))?;

        Ok(())
    }

    /// Retrieves all memory items
    async fn get_all_memories(&self) -> Result<Vec<MemoryItem>> {
        let memories = self.conn.call(|conn| {
            let mut stmt = conn.prepare("SELECT id, download_url, original_date, location, state, error_message, extension, has_overlay, has_thumbnail, media_type, segment_ids FROM memories")?;
            let mut memories = Vec::new();
            let rows = stmt.query_map([], |row| {
                let state_str: String = row.get(4)?;
                let segment_ids: Option<Vec<String>> = row
                    .get::<_, Option<String>>(10)
                    .ok()
                    .flatten()
                    .and_then(|s| serde_json::from_str(&s).ok());
                Ok(MemoryItem {
                    id: row.get(0)?,
                    segment_ids,
                    download_url: row.get(1)?,
                    original_date: row.get(2)?,
                    location: row.get(3)?,
                    state: ProcessingState::from_str(&state_str).unwrap_or(ProcessingState::Pending),
                    error_message: row.get(5)?,
                    extension: row.get(6)?,
                    has_overlay: row.get::<_, i32>(7)? != 0,
                    has_thumbnail: row.get::<_, i32>(8)? != 0,
                    media_type: row.get(9)?,
                })
            })?;
            for row in rows {
                if let Ok(item) = row {
                    memories.push(item);
                }
            }
            Ok(memories)
        }).await.map_err(|e| crate::error::AppError::Database(e.into()))?;
        
        Ok(memories)
    }

    /// Retrieves memory items filtered by state (e.g. for retry loop)
    async fn get_memories_by_state(&self, state: ProcessingState) -> Result<Vec<MemoryItem>> {
        let state_str = state.as_ref().to_string();
        let memories = self.conn.call(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, download_url, original_date, location, state, error_message, extension, has_overlay, has_thumbnail, media_type, segment_ids FROM memories WHERE state = ?1",
            )?;
            let mut memories = Vec::new();
            let rows = stmt.query_map([state_str], |row| {
                let state_str: String = row.get(4)?;
                let segment_ids: Option<Vec<String>> = row
                    .get::<_, Option<String>>(10)
                    .ok()
                    .flatten()
                    .and_then(|s| serde_json::from_str(&s).ok());
                Ok(MemoryItem {
                    id: row.get(0)?,
                    segment_ids,
                    download_url: row.get(1)?,
                    original_date: row.get(2)?,
                    location: row.get(3)?,
                    state: ProcessingState::from_str(&state_str).unwrap_or(ProcessingState::Pending),
                    error_message: row.get(5)?,
                    extension: row.get(6)?,
                    has_overlay: row.get::<_, i32>(7)? != 0,
                    has_thumbnail: row.get::<_, i32>(8)? != 0,
                    media_type: row.get(9)?,
                })
            })?;
            for row in rows {
                if let Ok(item) = row {
                    memories.push(item);
                }
            }
            Ok(memories)
        }).await.map_err(|e| crate::error::AppError::Database(e.into()))?;

        Ok(memories)
    }

    /// Bulk updates items from one state to another (e.g. Pending -> Paused)
    async fn update_states(
        &self,
        from_state: ProcessingState,
        to_state: ProcessingState,
    ) -> Result<()> {
        let from_str = from_state.as_ref().to_string();
        let to_str = to_state.as_ref().to_string();

        self.conn.call(move |conn| {
            conn.execute(
                "UPDATE memories SET state = ?1, error_message = NULL WHERE state = ?2",
                rusqlite::params![to_str, from_str],
            )
        }).await.map_err(|e| crate::error::AppError::Database(e.into()))?;

        Ok(())
    }

    /// Resets a specific item to Pending to retry it
    async fn reset_item_state(&self, id: &str) -> Result<()> {
        let cloned_id = id.to_string();
        let pending_str = ProcessingState::Pending.as_ref().to_string();

        self.conn.call(move |conn| {
            conn.execute(
                "UPDATE memories SET state = ?1, error_message = NULL WHERE id = ?2",
                rusqlite::params![pending_str, cloned_id],
            )
        }).await.map_err(|e| crate::error::AppError::Database(e.into()))?;

        Ok(())
    }
}
