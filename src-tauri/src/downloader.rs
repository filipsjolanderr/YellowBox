use crate::models::MemoryItem;
use futures::StreamExt;
use reqwest::{header, Client};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tracing::warn;

const MAX_RETRIES: u32 = 5;

/// Downloads a memory file, returning the path to the saved raw file.
pub async fn download_memory(
    client: &Client,
    item: &MemoryItem,
    dest_dir: &Path,
) -> Result<PathBuf, String> {
    // Generate a raw file name.
    let file_name = format!("{}-raw.tmp", item.id);
    let dest_path = dest_dir.join(&file_name);

    if dest_path.exists() {
        return Ok(dest_path);
    }

    let mut attempt = 0;
    loop {
        attempt += 1;

        let request = client
            .get(&item.download_url)
            .header("X-Snap-Route-Tag", "mem-dmd")
            .send()
            .await;

        match request {
            Ok(response) => {
                let status = response.status();
                if status.is_success() {
                    let content_type = response
                        .headers()
                        .get(header::CONTENT_TYPE)
                        .and_then(|val| val.to_str().ok())
                        .unwrap_or("");

                    let extension = if content_type.contains("zip")
                        || content_type.contains("application/zip")
                        || content_type.contains("application/x-zip-compressed")
                    {
                        "zip"
                    } else if content_type.contains("video/")
                        || item.media_type == "Video"
                        || item.download_url.to_lowercase().contains("video")
                        || item.download_url.to_lowercase().contains(".mp4")
                        || item.download_url.to_lowercase().contains(".mov")
                    {
                        "mp4"
                    } else {
                        "jpg"
                    };

                    let final_file_name = format!("{}-raw.{}", item.id, extension);
                    let final_dest_path = dest_dir.join(&final_file_name);

                    if final_dest_path.exists() {
                        return Ok(final_dest_path);
                    }

                    // Use a temporary path for downloading
                    let tmp_dest_path = dest_dir.join(format!("{}-raw.{}.tmp", item.id, extension));
                    
                    let mut file = File::create(&tmp_dest_path)
                        .await
                        .map_err(|e| format!("Failed to create temp file: {}", e))?;
                    let mut stream = response.bytes_stream();

                    while let Some(chunk) = stream.next().await {
                        let data = chunk.map_err(|e| format!("Stream error: {}", e))?;
                        file.write_all(&data).await.map_err(|e| format!("Write error: {}", e))?;
                    }
                    
                    // Flush and sync to be sure
                    file.flush().await.map_err(|e| format!("Flush error: {}", e))?;
                    
                    // Rename to final path on success
                    tokio::fs::rename(&tmp_dest_path, &final_dest_path).await
                        .map_err(|e| format!("Failed to rename temp file: {}", e))?;
                        
                    return Ok(final_dest_path);
                } else {
                    let is_rate_limited = status.as_u16() == 429;
                    if attempt >= MAX_RETRIES {
                        return Err(format!("Download failed after {} attempts. Status: {}", MAX_RETRIES, status));
                    }

                    // Exponential backoff
                    let delay = if is_rate_limited {
                        Duration::from_secs(10 * attempt as u64) // Heavier delay for rate limits
                    } else {
                        Duration::from_secs(2u64.pow(attempt) + 1)
                    };
                    warn!(id = %item.id, attempt, %status, delay_secs = delay.as_secs(), "download: retrying after HTTP error");
                    tokio::time::sleep(delay).await;
                }
            }
            Err(e) => {
                if attempt >= MAX_RETRIES {
                    return Err(format!("Request failed after {} attempts: {}", MAX_RETRIES, e));
                }
                let delay = Duration::from_secs(2u64.pow(attempt) + 1);
                warn!(id = %item.id, attempt, error = %e, delay_secs = delay.as_secs(), "download: retrying after request error");
                tokio::time::sleep(delay).await;
            }
        }
    }
}
