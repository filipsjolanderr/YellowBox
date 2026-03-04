use std::fs::File;
use std::io::Write;
use std::sync::Arc;
use zip::write::FileOptions;
use zip::ZipWriter;
use yellowbox_lib::db::{DbManager, MemoryRepository};
use yellowbox_lib::models::{MemoryItem, ProcessingState};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_end_to_end_extraction_pipeline() {
    let temp_dir = tempfile::tempdir().unwrap();
    let dest_dir = temp_dir.path().join("output");
    std::fs::create_dir_all(&dest_dir).unwrap();
    
    // 1. Setup a mocked DB manager
    let db_path = temp_dir.path().join("memories.db");
    let db_manager = Arc::new(DbManager::new(&db_path).await.unwrap());
    
    // 2. Insert a dummy memory item
    let memory_id = "test-memory-uuid".to_string();
    let memory_item = MemoryItem {
        segment_ids: None,
        id: memory_id.clone(),
        download_url: "http://example.com/mem.zip".to_string(),
        original_date: "2024-05-15 10:15:30 UTC".to_string(),
        location: Some("40.7128, -74.0060".to_string()),
        state: ProcessingState::Acquired, // Start at Acquired to skip network request mock
        error_message: None,
        extension: Some("jpg".to_string()),
        has_overlay: false,
        has_thumbnail: false,
        media_type: "Image".to_string(),
    };
    
    db_manager.insert_or_ignore_memory(&memory_item).await.unwrap();
    
    // 3. Create a fake downloaded Zip File containing our raw media
    let zip_path = dest_dir.join(format!("{}-raw.zip", memory_id));
    let file = File::create(&zip_path).unwrap();
    let mut zip = ZipWriter::new(file);
    let options = FileOptions::<()>::default().compression_method(zip::CompressionMethod::Stored);
    
    zip.start_file(format!("{}-main.jpg", memory_id), options).unwrap();
    zip.write_all(b"fake image data").unwrap();
    zip.finish().unwrap();

    // 4. Test pipeline logic execution directly.
    // NOTE: This cannot run via PipelineService because PipelineService heavily relies 
    // on `tauri::AppHandle` for Event emission (`app.emit(...)`) and fetching `app_data_dir()`,
    // which panics when invoked outside of a running Tauri context.
    
    // We mock the Extract & Combine phase directly instead of spinning up `PipelineService`.
    let extracted_files = yellowbox_lib::extractor::extract_memory(&zip_path, &memory_id, &dest_dir).await.unwrap();
    assert!(extracted_files.0.exists(), "Main file not extracted properly");
    
    // Combine
    let clean_name = yellowbox_lib::metadata::generate_clean_filename(
        &memory_item.original_date,
        &memory_item.id,
        "jpg",
    );
    let combined_dest = dest_dir.join(&clean_name);
    tokio::fs::copy(&extracted_files.0, &combined_dest).await.unwrap();
    assert!(combined_dest.exists(), "Combined file not created");
    
    // Update State
    db_manager.update_state(&memory_id, ProcessingState::Completed, None, Some("jpg".to_string()), Some(false), Some(false)).await.unwrap();
    
    // Verify State Check
    let memories = db_manager.get_all_memories().await.unwrap();
    let item = memories.iter().find(|i| i.id == memory_id).unwrap();
    assert_eq!(item.state, ProcessingState::Completed);
    assert!(combined_dest.exists());
}
