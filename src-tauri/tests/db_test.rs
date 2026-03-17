use yellowbox_lib::db::{DbManager, MemoryRepository};
use yellowbox_lib::models::{MemoryItem, ProcessingState};

async fn create_test_db() -> DbManager {
    // SQLite can open an in-memory database by passing ":memory:"
    DbManager::new(":memory:").await.expect("Failed to create in-memory db")
}

fn create_mock_item(id: &str) -> MemoryItem {
    MemoryItem {
        segment_ids: None,
        id: id.to_string(),
        download_url: format!("http://example.com/{}", id),
        original_date: "2024-01-01 12:00:00 UTC".to_string(),
        location: None,
        state: ProcessingState::Pending,
        error_message: None,
        extension: None,
        has_overlay: false,
        has_thumbnail: false,
        media_type: "image".to_string(),
    }
}

#[tokio::test]
async fn test_db_initialization() {
    let _db = create_test_db().await;
}

#[tokio::test]
async fn test_insert_and_get_memories() {
    let db = create_test_db().await;
    let item1 = create_mock_item("mem1");
    let item2 = create_mock_item("mem2");

    db.insert_or_ignore_memory(&item1).await.unwrap();
    db.insert_or_ignore_memory(&item2).await.unwrap();

    let memories = db.get_all_memories().await.unwrap();
    assert_eq!(memories.len(), 2);
    assert!(memories.iter().any(|m| m.id == "mem1"));
    assert!(memories.iter().any(|m| m.id == "mem2"));
}

#[tokio::test]
async fn test_insert_or_ignore_duplicate() {
    let db = create_test_db().await;
    let item = create_mock_item("mem1");

    // Insert first time
    db.insert_or_ignore_memory(&item).await.unwrap();

    // Insert second time with same ID (should just ignore and not error)
    db.insert_or_ignore_memory(&item).await.unwrap();

    let memories = db.get_all_memories().await.unwrap();
    assert_eq!(memories.len(), 1);
}

#[tokio::test]
async fn test_update_state() {
    let db = create_test_db().await;
    let item = create_mock_item("mem_upd");
    db.insert_or_ignore_memory(&item).await.unwrap();

    // Update state to Extract, set extension to "png" and has_overlay to true
    db.update_state(
        "mem_upd",
        ProcessingState::Unpacked,
        Some("No error"),
        Some("png".to_string()),
        Some(true),
        None,
    )
    .await
    .unwrap();

    let memories = db.get_all_memories().await.unwrap();
    assert_eq!(memories.len(), 1);

    let updated = &memories[0];
    assert_eq!(updated.state, ProcessingState::Unpacked);
    assert_eq!(updated.error_message, Some("No error".to_string()));
    assert_eq!(updated.extension, Some("png".to_string()));
    assert_eq!(updated.has_overlay, true);
}

#[tokio::test]
async fn test_update_partial_fields() {
    let db = create_test_db().await;
    let item = create_mock_item("mem_part");
    db.insert_or_ignore_memory(&item).await.unwrap();

    // First update gives it an extension and overlay
    db.update_state(
        "mem_part",
        ProcessingState::Acquired,
        None,
        Some("jpg".to_string()),
        Some(true),
        None,
    )
    .await
    .unwrap();

    // Second update should preserve extension/overlay due to COALESCE in SQL
    db.update_state("mem_part", ProcessingState::Completed, None, None, None, None)
        .await
        .unwrap();

    let memories = db.get_all_memories().await.unwrap();
    assert_eq!(memories.len(), 1);

    let updated = &memories[0];
    assert_eq!(updated.state, ProcessingState::Completed);
    assert_eq!(updated.extension, Some("jpg".to_string()));
    assert_eq!(updated.has_overlay, true);
}

#[tokio::test]
async fn test_bulk_insert() {
    let db = create_test_db().await;
    let items = vec![
        create_mock_item("bulk1"),
        create_mock_item("bulk2"),
        create_mock_item("bulk3"),
    ];

    db.bulk_insert_memories(items).await.unwrap();

    let memories = db.get_all_memories().await.unwrap();
    assert_eq!(memories.len(), 3);
    assert!(memories.iter().any(|m| m.id == "bulk1"));
    assert!(memories.iter().any(|m| m.id == "bulk2"));
    assert!(memories.iter().any(|m| m.id == "bulk3"));
}
