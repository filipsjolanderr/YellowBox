#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_processing_state_display() {
        assert_eq!(ProcessingState::Pending.to_string(), "Pending");
        assert_eq!(ProcessingState::Downloaded.to_string(), "Downloaded");
        assert_eq!(ProcessingState::Extracted.to_string(), "Extracted");
        assert_eq!(ProcessingState::Combined.to_string(), "Combined");
        assert_eq!(ProcessingState::Completed.to_string(), "Completed");
        assert_eq!(ProcessingState::Failed.to_string(), "Failed");
    }

    #[test]
    fn test_processing_state_from_str() {
        use std::str::FromStr;
        assert_eq!(
            ProcessingState::from_str("Pending").unwrap(),
            ProcessingState::Pending
        );
        assert_eq!(
            ProcessingState::from_str("Completed").unwrap(),
            ProcessingState::Completed
        );
        assert!(ProcessingState::from_str("UnknownState").is_err());
    }
}
