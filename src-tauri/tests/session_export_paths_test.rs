use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use yellowbox_lib::services::session::{apply_export_paths, SessionState};

#[test]
fn apply_export_paths_replaces_and_clears_indexes() {
    let mut s = SessionState::new();
    s.export_paths = vec![PathBuf::from("a.zip"), PathBuf::from("b.zip")];
    s.main_index = Some(Arc::new(HashMap::from([(
        "k".to_string(),
        (0usize, 0usize, "p".to_string()),
    )])));
    s.overlay_index = Some(Arc::new(HashMap::from([(
        "k".to_string(),
        (0usize, 0usize, "p".to_string()),
    )])));

    let changed = apply_export_paths(&mut s, vec![PathBuf::from("b.zip")]);
    assert!(changed);
    assert_eq!(s.export_paths, vec![PathBuf::from("b.zip")]);
    assert!(s.main_index.is_none());
    assert!(s.overlay_index.is_none());
}

#[test]
fn apply_export_paths_noop_when_unchanged() {
    let mut s = SessionState::new();
    s.export_paths = vec![PathBuf::from("a.zip")];
    s.main_index = None;
    s.overlay_index = None;

    let changed = apply_export_paths(&mut s, vec![PathBuf::from("a.zip")]);
    assert!(!changed);
    assert_eq!(s.export_paths, vec![PathBuf::from("a.zip")]);
}

