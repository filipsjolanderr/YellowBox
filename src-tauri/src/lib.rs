// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
pub mod combiner;
pub mod commands;
pub mod db;
pub mod downloader;
pub mod extractor;
pub mod fs;
pub mod metadata;
pub mod models;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(commands::AppState {
            db: std::sync::Mutex::new(None),
            export_path: std::sync::Mutex::new(None),
            output_dir: std::sync::Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![
            commands::check_zip_structure,
            commands::initialize_and_load,
            commands::get_memories_state,
            commands::start_pipeline,
            commands::reset_application,
            commands::cleanup_database
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
