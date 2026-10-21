//! Команды для окна настроек.

use crate::config::{self, Config, Snippet};
use crate::engine::{self, LogEntry, Shared};
use crate::executor::Job;
use crate::platform::{apps, clipboard, foreground};
use crate::rules::{compile, Rule, RuleView};
use serde::Serialize;
use std::sync::mpsc::Sender;
use std::sync::Arc;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_opener::OpenerExt;

/// Канал к исполнителю — для кнопки «Проверить».
pub struct JobTx(pub Sender<Job>);

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

/// Настройки для редактора и «версия» файла, с которой окно начало работу.
#[derive(Serialize)]
pub struct ConfigDoc {
    config: Option<Config>,
    error: Option<String>,
    stamp: Option<u64>,
}

#[tauri::command]
pub fn get_config(state: State<'_, Arc<Shared>>) -> ConfigDoc {
    let stamp = config::stamp(&state.config_path);
    match config::load(&state.config_path) {
        Ok(c) => ConfigDoc {
            config: Some(c),
            error: None,
            stamp,
        },
        Err(e) => ConfigDoc {
            config: None,
            error: Some(e),
            stamp,
        },
    }
}

#[tauri::command]
pub fn save_config(
    app: AppHandle,
    config: Config,
    stamp: Option<u64>,
) -> Result<Option<u64>, String> {
    engine::save_config(&app, &config, stamp)
}

/// Проверка черновика до сохранения: что с каждым правилом и общие замечания.
#[derive(Serialize)]
pub struct Check {
    rules: Vec<RuleView>,
    warnings: Vec<String>,
}

#[tauri::command]
pub fn check_config(config: Config) -> Check {
    let c = compile(&config);
    Check {
        rules: c.views,
        warnings: c.warnings,
    }
}

#[tauri::command]
pub fn list_apps() -> Vec<apps::RunningApp> {
    apps::running_apps()
}

/// Предпросмотр текста с подстановками: что вставится прямо сейчас.
/// Место курсора отмечено символом ‸.
#[tauri::command]
pub fn preview_text(text: String) -> Result<String, String> {
    let pieces = crate::macros::parse_text(&text)?;
    let now = chrono::Local::now().naive_local();
    let mut clip = || clipboard::get_text(clipboard::no_owner());
    let (out, back) = crate::macros::expand_text(&pieces, now, &mut clip);
    if back == 0 {
        return Ok(out);
    }
    // Вставить ‸ за `back` шагов от конца (перенос строки — один шаг).
    let chars: Vec<char> = out.replace("\r\n", "\n").chars().collect();
    let at = chars.len().saturating_sub(back);
    Ok(chars[..at]
        .iter()
        .chain(['‸'].iter())
        .chain(chars[at..].iter())
        .collect())
}

/// Проверить сочетание, записанное окном: те же правила, что для файла.
/// None — можно занимать, иначе — почему нельзя.
#[tauri::command]
pub fn check_chord(chord: String) -> Option<String> {
    crate::chord::Chord::parse(&chord).err()
}

/// Выполнить заготовку прямо сейчас — в поле проверки окна, которое
/// интерфейс заранее поставил в фокус.
#[tauri::command]
pub fn test_snippet(
    snippet: Snippet,
    state: State<'_, Arc<Shared>>,
    tx: State<'_, JobTx>,
) -> Result<(), String> {
    let rule = Rule::adhoc(&snippet)?;
    let compiled = state.rules.read().clone();
    tx.0.send(Job::Run {
        rule: Arc::new(rule),
        compiled,
        app: foreground::active_app(),
        erase: 0,
        suffix: None,
    })
    .map_err(|_| "исполнитель не отвечает".to_string())
}

#[tauri::command]
pub fn get_autostart(app: AppHandle) -> bool {
    app.autolaunch().is_enabled().unwrap_or(false)
}

#[tauri::command]
pub fn set_autostart(app: AppHandle, enabled: bool) -> Result<(), String> {
    let al = app.autolaunch();
    if enabled { al.enable() } else { al.disable() }.map_err(|e| format!("автозапуск: {e}"))
}
