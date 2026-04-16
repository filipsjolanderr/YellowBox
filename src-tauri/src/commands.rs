use crate::error::Result;
use crate::models::MemoryItem;
use crate::services::pipeline_orchestrator::PipelineOrchestrator;
use crate::services::session::{AppState, SessionService};
use tauri::{AppHandle, State};

#[tauri::command]
pub async fn check_zip_structure(
    session_id: String,
    path: String,
    state: State<'_, AppState>,
) -> Result<Vec<MemoryItem>> {
    SessionService::check_zip_structure(session_id, path, state).await
}

#[tauri::command]
pub async fn set_export_paths(
    session_id: String,
    paths: Vec<String>,
    state: State<'_, AppState>,
) -> Result<()> {
    SessionService::set_export_paths(session_id, paths, state).await
}

#[tauri::command]
pub async fn initialize_and_load(
    session_id: String,
    output_path: String,
    items: Vec<MemoryItem>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<MemoryItem>> {
    SessionService::initialize_and_load(session_id, output_path, items, app, state).await
}

#[tauri::command]
pub async fn get_memories_state(session_id: String, state: State<'_, AppState>) -> Result<Vec<MemoryItem>> {
    SessionService::get_memories_state(session_id, state).await
}

#[tauri::command]
pub async fn reset_application(session_id: String, state: State<'_, AppState>) -> Result<()> {
    SessionService::reset_application(session_id, state).await
}

#[tauri::command]
pub async fn cleanup_database(session_id: String, app: AppHandle, state: State<'_, AppState>) -> Result<()> {
    SessionService::cleanup_database(session_id, app, state).await
}

#[tauri::command]
pub async fn clear_all_data(app: AppHandle, state: State<'_, AppState>) -> Result<()> {
    SessionService::clear_all_data(app, state).await
}

#[tauri::command]
pub async fn start_pipeline(
    session_id: String,
    overwrite_existing: bool,
    max_concurrency: Option<usize>,
    output_path: Option<String>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<()> {
    PipelineOrchestrator::start_pipeline(
        session_id,
        overwrite_existing,
        max_concurrency,
        output_path,
        app,
        state,
    )
    .await
}

#[tauri::command]
pub async fn pause_pipeline(session_id: String, state: State<'_, AppState>) -> Result<()> {
    PipelineOrchestrator::pause_pipeline(session_id, state).await
}

// DELETED extract_preview_media command

#[tauri::command]
pub async fn resolve_local_media_paths(
    session_id: String,
    memory_ids: Vec<String>,
    state: State<'_, AppState>,
) -> Result<std::collections::HashMap<String, String>> {
    SessionService::resolve_local_media_paths(session_id, memory_ids, state).await
}

#[tauri::command]
pub async fn check_overlay_exists(output_dir: String, memory_id: String, clean_date: String) -> Result<bool> {
    SessionService::check_overlay_exists(output_dir, memory_id, clean_date)
}

#[tauri::command]
pub async fn retry_item(session_id: String, item_id: String, state: State<'_, AppState>) -> Result<()> {
    SessionService::retry_item(session_id, item_id, state).await
}
