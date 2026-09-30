//! Команды для окна настроек.

use crate::engine::{self, LogEntry, Shared};
use crate::rules::RuleView;
use serde::Serialize;
use std::sync::Arc;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_opener::OpenerExt;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    paused: bool,
    config_path: String,
    loaded_at: Option<String>,
    error: Option<String>,
    warnings: Vec<String>,
    pause_hotkey: Option<String>,
    active_count: usize,
    rules: Vec<RuleView>,
}

#[tauri::command]
pub fn get_status(app: AppHandle, state: State<'_, Arc<Shared>>) -> Status {
    let compiled = state.rules.read().clone();
    let load = state.status.lock().clone();
    Status {
        paused: crate::tray::is_paused(&app),
        config_path: state.config_path.display().to_string(),
        loaded_at: load.loaded_at,
        error: load.error,
        warnings: compiled.warnings.clone(),
        pause_hotkey: compiled.pause.map(|c| c.to_string()),
        active_count: compiled.active_count(),
        rules: compiled.views.clone(),
    }
}

#[tauri::command]
pub fn get_log(state: State<'_, Arc<Shared>>) -> Vec<LogEntry> {
    state.journal()
}

#[tauri::command]
pub fn set_paused(app: AppHandle, paused: bool) {
    engine::set_paused(&app, paused);
}

#[tauri::command]
pub fn reload_config(app: AppHandle) {
    engine::reload(&app);
}

#[tauri::command]
pub fn open_config(app: AppHandle) {
    open_config_file(&app);
}

#[tauri::command]
pub fn open_config_folder(app: AppHandle, state: State<'_, Arc<Shared>>) {
    if let Err(e) = app.opener().reveal_item_in_dir(&state.config_path) {
        engine::log(&app, "err", format!("Не открыть папку настроек: {e}"));
    }
}

/// Открыть config.yaml. У .yaml часто нет программы по умолчанию —
/// тогда открываем Блокнотом, а не показываем системный диалог выбора.
pub fn open_config_file(app: &AppHandle) {
    let path = app.state::<Arc<Shared>>().config_path.clone();
    let path_str = path.display().to_string();
    if app.opener().open_path(&path_str, None::<&str>).is_err() {
        if let Err(e) = std::process::Command::new("notepad.exe").arg(&path).spawn() {
            engine::log(app, "err", format!("Не открыть файл настроек: {e}"));
        }
    }
}

