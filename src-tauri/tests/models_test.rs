use std::str::FromStr;
use yellowbox_lib::models::{MemoryItem, ProcessingState};

#[test]
fn test_processing_state_to_string() {
    assert_eq!(ProcessingState::Pending.to_string(), "Pending");
    // Serialized names kept for DB/API compatibility
    assert_eq!(ProcessingState::Acquired.to_string(), "Downloaded");
    assert_eq!(ProcessingState::Unpacked.to_string(), "Extracted");
    assert_eq!(ProcessingState::Composited.to_string(), "Combined");
    assert_eq!(ProcessingState::Completed.to_string(), "Completed");
    assert_eq!(ProcessingState::Failed.to_string(), "Failed");
}

#[test]
fn test_processing_state_from_string() {
    assert_eq!(
        ProcessingState::from_str("Pending").unwrap(),
        ProcessingState::Pending
    );
    assert_eq!(
        ProcessingState::from_str("Completed").unwrap(),
        ProcessingState::Completed
    );
    assert!(ProcessingState::from_str("InvalidState").is_err());
}

#[test]
fn test_processing_state_as_ref() {
    assert_eq!(ProcessingState::Failed.as_ref(), "Failed");
}

#[test]
fn test_memory_item_serialization() {
    let item = MemoryItem {
        id: "test1".to_string(),
        segment_ids: None,
        candidate_ids: None,
        download_url: "http://example.com/mem.zip".to_string(),
        original_date: "2022-01-01 12:00:00 UTC".to_string(),
        location: Some("12.34, 56.78".to_string()),
        state: ProcessingState::Acquired,
        error_message: None,
        extension: Some("mp4".to_string()),
        has_overlay: true,
        has_thumbnail: false,
        media_type: "video".to_string(),
    };

    let serialized = serde_json::to_string(&item).unwrap();
    assert!(serialized.contains(r#""id":"test1""#));
    assert!(serialized.contains(r#""state":"Downloaded""#));
    assert!(serialized.contains(r#""hasOverlay":true"#)); // Testing camelCase formatting

    let deserialized: MemoryItem = serde_json::from_str(&serialized).unwrap();
    assert_eq!(deserialized.id, "test1");
    assert_eq!(deserialized.state, ProcessingState::Acquired);
}

#[test]
fn test_memory_item_default_extension_optional() {
    let json = r#"{
        "id": "test2",
        "downloadUrl": "url",
        "originalDate": "date",
        "location": null,
        "state": "Pending",
        "errorMessage": null,
        "extension": null,
        "hasOverlay": false,
        "hasThumbnail": false,
        "mediaType": "image"
    }"#;
    let item: MemoryItem = serde_json::from_str(json).unwrap();
    assert_eq!(item.id, "test2");
    assert!(item.extension.is_none());
}
