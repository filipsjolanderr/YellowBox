use crate::models::MemoryItem;
use std::collections::HashMap;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

/// Coalesces frequent per-item UI updates to reduce IPC/event overhead.
///
/// Contract: emits the existing `memory-updated-{session_id}` event, but may delay updates
/// slightly and will drop intermediate updates for the same id (only latest per tick).
#[derive(Clone)]
pub struct EventEmitter {
    tx: tokio::sync::mpsc::UnboundedSender<MemoryItem>,
}

impl EventEmitter {
    pub fn new(app: AppHandle, session_id: String) -> Self {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<MemoryItem>();

        tokio::spawn(async move {
            let mut pending: HashMap<String, MemoryItem> = HashMap::new();
            let mut ticker = tokio::time::interval(Duration::from_millis(150));
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            loop {
                tokio::select! {
                    maybe = rx.recv() => {
                        match maybe {
                            Some(item) => {
                                pending.insert(item.id.clone(), item);
                            }
                            None => break,
                        }
                    }
                    _ = ticker.tick() => {
                        if pending.is_empty() {
                            continue;
                        }
                        let items: Vec<MemoryItem> = pending.drain().map(|(_, v)| v).collect();
                        for item in items {
                            let _ = app.emit(&format!("memory-updated-{}", session_id), item);
                        }
                    }
                }
            }

            // flush any remaining updates
            for (_, item) in pending {
                let _ = app.emit(&format!("memory-updated-{}", session_id), item);
            }
        });

        Self { tx }
    }

    pub fn memory_updated(&self, item: MemoryItem) {
        let _ = self.tx.send(item);
    }
}

