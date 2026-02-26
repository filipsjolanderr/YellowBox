use std::io::{Error, ErrorKind};
use yellowbox_lib::error::AppError;

#[test]
fn test_error_from_str() {
    let err: AppError = "Something went wrong".into();
    match err {
        AppError::Message(msg) => assert_eq!(msg, "Something went wrong"),
        _ => panic!("Expected AppError::Message"),
    }
}

#[test]
fn test_error_from_string() {
    let err: AppError = String::from("Another issue").into();
    match err {
        AppError::Message(msg) => assert_eq!(msg, "Another issue"),
        _ => panic!("Expected AppError::Message"),
    }
}

#[test]
fn test_error_display() {
    let err = AppError::Internal("Disk full".to_string());
    assert_eq!(format!("{}", err), "Internal error: Disk full");
}

#[test]
fn test_error_from_io_error() {
    let io_err = Error::new(ErrorKind::NotFound, "file not found");
    let app_err: AppError = io_err.into();
    assert_eq!(format!("{}", app_err), "IO error: file not found");
}

#[test]
fn test_error_serialization() {
    let err = AppError::Internal("A bad error".to_string());
    let serialized = serde_json::to_string(&err).unwrap();
    // Serialize returns just the error message formatted as string literal
    assert_eq!(serialized, r#""Internal error: A bad error""#);
}
