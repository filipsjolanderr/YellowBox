use crate::models::MemoryItem;
use futures::StreamExt;
use reqwest::{header, Client};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

const MAX_RETRIES: u32 = 3;

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
        // We probably don't know the exact extension yet if it was interrupted, but let's assume raw is ready.
        // Actually it's better to check if it's already extracted, which the orchestrator will handle.
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
                if response.status().is_success() {
                    // Check Content-Disposition or Content-Type to determine if it's a zip or media
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

                    let mut file = File::create(&final_dest_path)
                        .await
                        .map_err(|e| e.to_string())?;
                    let mut stream = response.bytes_stream();

                    while let Some(chunk) = stream.next().await {
                        let data = chunk.map_err(|e| e.to_string())?;
                        file.write_all(&data).await.map_err(|e| e.to_string())?;
                    }
                    return Ok(final_dest_path);
                } else if attempt >= MAX_RETRIES {
                    return Err(format!("HTTP Error: {}", response.status()));
                }
            }
            Err(e) => {
                if attempt >= MAX_RETRIES {
                    return Err(e.to_string());
                }
            }
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}
