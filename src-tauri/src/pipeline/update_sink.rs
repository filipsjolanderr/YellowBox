use crate::db::MemoryRepository;
use crate::error::Result;
use crate::infra::event_emitter::EventEmitter;
use crate::models::{MemoryItem, ProcessingState};
use std::sync::Arc;

/// Centralizes DB + UI updates for processing state changes.
///
/// DB writes are still performed immediately for correctness, but UI emission is coalesced
/// via `EventEmitter` to reduce chatter.
pub struct UpdateSink<R: MemoryRepository> {
    db: Arc<R>,
    emitter: EventEmitter,
}

impl<R: MemoryRepository> Clone for UpdateSink<R> {
    fn clone(&self) -> Self {
        Self {
            db: Arc::clone(&self.db),
            emitter: self.emitter.clone(),
        }
    }
}

impl<R: MemoryRepository> UpdateSink<R> {
    pub fn new(db: Arc<R>, emitter: EventEmitter) -> Self {
        Self { db, emitter }
    }

    pub async fn update_state_and_emit(
        &self,
        mut item: MemoryItem,
        state: ProcessingState,
        error_message: Option<String>,
        extension: Option<String>,
        has_overlay: Option<bool>,
        has_thumbnail: Option<bool>,
    ) -> Result<()> {
        let err_ref = error_message.as_deref();
        self.db
            .update_state(
                &item.id,
                state.clone(),
                err_ref,
                extension.clone(),
                has_overlay,
                has_thumbnail,
            )
            .await?;

        item.state = state;
        item.error_message = error_message;
        if extension.is_some() {
            item.extension = extension;
        }
        if let Some(v) = has_overlay {
            item.has_overlay = v;
        }
        self.emitter.memory_updated(item);
        Ok(())
    }
}

