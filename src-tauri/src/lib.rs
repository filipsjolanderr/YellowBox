// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
use std::path::PathBuf;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

pub mod combiner;
pub mod commands;
pub mod db;
pub mod downloader;
pub mod error;
pub mod extractor;
pub mod fs;
pub mod metadata;
pub mod models;
pub mod pipeline;

/// Returns the log directory path. Creates it if it doesn't exist.
fn log_dir() -> Option<PathBuf> {
    let dirs = directories::ProjectDirs::from("", "", "YellowBox")?;
    let log_dir = dirs.data_local_dir().join("logs");
    let _ = std::fs::create_dir_all(&log_dir);
    Some(log_dir)
}

/// Initializes tracing/logging. Safe to call multiple times (no-op after first init).
/// Logs to both stdout and a daily-rotating file in the app data directory.
pub fn init_logging() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,little_exif=off"));
    let console_layer = tracing_subscriber::fmt::layer().with_target(true);

    if let Some(log_dir) = log_dir() {
        if let Ok(file_appender) = tracing_appender::rolling::Builder::new()
            .rotation(tracing_appender::rolling::Rotation::DAILY)
            .build(log_dir.join("yellowbox.log"))
        {
            let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
            let _ = Box::leak(Box::new(guard)); // Keep worker alive for process lifetime
            let file_layer = tracing_subscriber::fmt::layer()
                .with_target(true)
                .with_ansi(false)
                .with_writer(non_blocking);
            let _ = tracing_subscriber::registry()
                .with(filter)
                .with(console_layer)
                .with(file_layer)
                .try_init();
            return;
        }
    }

    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(console_layer)
        .try_init();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    init_logging();
    if let Some(log_dir) = log_dir() {
        tracing::info!(path = %log_dir.display(), "YellowBox starting; logs written to file");
    } else {
        tracing::info!("YellowBox starting");
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(commands::AppState {
            sessions: std::sync::Mutex::new(std::collections::HashMap::new()),
        })
        .invoke_handler(tauri::generate_handler![
            commands::check_zip_structure,
            commands::initialize_and_load,
            commands::get_memories_state,
            commands::clear_preview_temp,
            commands::extract_preview_media,
            commands::resolve_local_media_paths,
            commands::start_pipeline,
            commands::pause_pipeline,
            commands::check_overlay_exists,
            commands::retry_item,
            commands::reset_application,
            commands::cleanup_database
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
