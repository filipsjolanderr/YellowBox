use crate::error::AppError;
use crate::error::Result;
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use zip::ZipArchive;

/// Provides cached, thread-safe access to ZIP archives for blocking extraction code.
///
/// Important: `ZipArchive<BufReader<File>>` is inherently synchronous and not `Send` across awaits.
/// This type is designed to be used inside `spawn_blocking` (or other blocking contexts),
/// avoiding async locks and `block_on` re-entrancy.
#[derive(Clone, Default)]
pub struct ZipAccess {
    pool: Arc<Mutex<HashMap<PathBuf, Arc<Mutex<ZipArchive<BufReader<File>>>>>>>,
}

impl ZipAccess {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_or_open(&self, zip_path: &Path) -> Result<Arc<Mutex<ZipArchive<BufReader<File>>>>> {
        let mut pool = self
            .pool
            .lock()
            .map_err(|e| AppError::Internal(format!("zip pool poisoned: {}", e)))?;
        if let Some(a) = pool.get(zip_path) {
            return Ok(Arc::clone(a));
        }

        let file = File::open(zip_path)
            .map_err(|e| AppError::Message(format!("Open ZIP failed ({}): {}", zip_path.display(), e)))?;
        let reader = BufReader::new(file);
        let archive =
            ZipArchive::new(reader).map_err(|e| AppError::Message(format!("Invalid ZIP: {}", e)))?;
        let a = Arc::new(Mutex::new(archive));
        pool.insert(zip_path.to_path_buf(), Arc::clone(&a));
        Ok(a)
    }
}

