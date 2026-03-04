use serde::{Deserialize, Serialize};

use strum_macros::{AsRefStr, Display, EnumString};

/// Processing state for a memory item through the pipeline.
/// Serialized names are kept for DB/API compatibility.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Display, EnumString, AsRefStr)]
pub enum ProcessingState {
    /// Queued for processing
    Pending,

    /// Raw file available (from memory folder, source ZIP, or CDN)
    #[strum(serialize = "Downloaded")]
    #[serde(rename = "Downloaded")]
    Acquired,

    /// Unpacked from ZIP; main media and optional overlay extracted
    #[strum(serialize = "Extracted")]
    #[serde(rename = "Extracted")]
    Unpacked,

    /// Overlay composited onto main media
    #[strum(serialize = "Combined")]
    #[serde(rename = "Combined")]
    Composited,

    /// Fully processed; ready for export
    Completed,

    /// Processing failed (see error_message)
    Failed,

    /// Paused by user
    Paused,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryItem {
    pub id: String, // Primary ID (first segment for split videos)
    /// For split videos: segment IDs in playback order. Empty/None for single-segment.
    #[serde(default)]
    pub segment_ids: Option<Vec<String>>,
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
        let is_video = self.media_type.eq_ignore_ascii_case("Video")
            || self.download_url.to_lowercase().contains(".mp4")
            || self.download_url.to_lowercase().contains(".mov")
            || self.download_url.to_lowercase().contains("video");
        let ext = if is_video {
            // Videos always output as mp4 (path may have wrong ext e.g. .jpg)
            "mp4".to_string()
        } else {
            self.extension.clone().unwrap_or_else(|| "jpg".to_string())
        };
        let clean_name =
            crate::metadata::generate_clean_filename(&self.original_date, &self.id, &ext);
        (clean_name, ext)
    }
}
