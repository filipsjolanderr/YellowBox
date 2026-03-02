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
    Paused,
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
    pub has_thumbnail: bool,
    pub media_type: String,
}

impl MemoryItem {
    /// Determines the fallback extension and generates the final output filename.
    pub fn generated_filename_and_ext(&self) -> (String, String) {
        let ext = self.extension.clone().unwrap_or_else(|| {
            let url = self.download_url.to_lowercase();
            if url.contains(".mp4") || url.contains(".mov") || url.contains("video") {
                "mp4".to_string()
            } else {
                "jpg".to_string()
            }
        });
        let clean_name =
            crate::metadata::generate_clean_filename(&self.original_date, &self.id, &ext);
        (clean_name, ext)
    }
}
