use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ProcessingState {
    Pending,
    Downloaded,
    Extracted,
    Combined,
    Completed,
    Failed,
}

impl ProcessingState {
    pub fn as_str(&self) -> &'static str {
        match self {
            ProcessingState::Pending => "Pending",
            ProcessingState::Downloaded => "Downloaded",
            ProcessingState::Extracted => "Extracted",
            ProcessingState::Combined => "Combined",
            ProcessingState::Completed => "Completed",
            ProcessingState::Failed => "Failed",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "Pending" => ProcessingState::Pending,
            "Downloaded" => ProcessingState::Downloaded,
            "Extracted" => ProcessingState::Extracted,
            "Combined" => ProcessingState::Combined,
            "Completed" => ProcessingState::Completed,
            "Failed" => ProcessingState::Failed,
            _ => ProcessingState::Pending,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryItem {
    pub id: String, // We'll extract a unique ID or hash the URL
    pub download_url: String,
    pub original_date: String,
    pub location: Option<String>,
    pub state: ProcessingState,
    pub error_message: Option<String>,
    pub extension: Option<String>,
    pub has_overlay: bool,
    pub media_type: String,
}
