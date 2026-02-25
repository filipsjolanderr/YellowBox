use serde::{Deserialize, Serialize};

use strum_macros::{AsRefStr, Display, EnumString};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Display, EnumString, AsRefStr)]
pub enum ProcessingState {
    Pending,
    Downloaded,
    Extracted,
    Combined,
    Completed,
    Failed,
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
