//! Отклик: сочетание клавиш → действие (пока — вставка текста).
//!
//! Потоки: otk-hook (перехват клавиатуры) → канал → otk-exec (исполнение),
//! otk-watch (следит за config.yaml). Подробно — docs/ARCHITECTURE.md.

pub mod chord;
pub mod config;
pub mod macros;
pub mod rules;

mod commands;
mod engine;
mod espanso;
mod executor;
mod hook;
#[cfg(target_os = "macos")]
mod mac;
mod platform;
mod search;
mod tray;
#[cfg(windows)]
mod win;

use std::sync::{mpsc, Arc};
use tauri::{Manager, WindowEvent};

/// Программа раньше называлась Key Manager и хранила настройки в
/// `%APPDATA%\app.keymanager.desktop`. Если новых настроек ещё нет, а старые
/// есть, — переносим их копией (старую папку не трогаем: вдруг откат).
fn migrate_from_key_manager(new_config: &std::path::Path) {
    if new_config.exists() {
        return;
    }
    let Some(appdata) = new_config.parent().and_then(|d| d.parent()) else {
        return;
    };
    let old = appdata.join("app.keymanager.desktop").join("config.yaml");
    if !old.exists() {
        return;
    }
    if let Some(dir) = new_config.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    match std::fs::copy(&old, new_config) {
        Ok(_) => eprintln!("[otk] info настройки перенесены из Key Manager"),
        Err(e) => eprintln!("[otk] err не удалось перенести настройки Key Manager: {e}"),
    }
}

/// Автозапуск хранит путь к программе. После переустановки или
/// обновления путь мог измениться — перезаписываем его при каждом старте.
fn sync_autostart(app: &tauri::AppHandle) {
    use tauri_plugin_autostart::ManagerExt;
    let al = app.autolaunch();
    if al.is_enabled().unwrap_or(false) {
        let _ = al.enable();
    }
}

pub fn run() {
    tauri::Builder::default()
        // Первым: второй экземпляр означал бы двойную вставку на каждое
        // сочетание. Вместо запуска второго — показываем окно первого.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            tray::show_main(app);
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .invoke_handler(tauri::generate_handler![
            commands::get_status,
            commands::get_log,
            commands::set_paused,
            commands::reload_config,
            commands::open_config,
            commands::open_config_folder,
            commands::get_config,
            commands::save_config,
            commands::check_config,
            commands::list_apps,
            commands::check_chord,
            commands::preview_text,
            search::search_list,
            search::search_run,
            search::search_hide,
            platform::admin::admin_status,
            platform::admin::admin_set_task,
            platform::admin::admin_restart,
            platform::admin::platform_info,
            platform::admin::open_permissions,
            espanso::import_espanso,
            commands::test_snippet,
            commands::get_autostart,
            commands::set_autostart,
        ])
        .on_window_event(|window, event| {
            // Крестик прячет окно в трей: программа должна работать дальше.
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .setup(|app| {
            let path = app.path().app_config_dir()?.join("config.yaml");
            migrate_from_key_manager(&path);
            let first_run = match config::ensure_exists(&path) {
                Ok(created) => created,
                Err(e) => {
                    eprintln!("[otk] err {e}");
                    false
                }
            };
            let shared = Arc::new(engine::Shared::new(path));
            app.manage(shared.clone());
            // На Mac — значок в строке меню без значка в Доке, и сразу
            // просьба о разрешении «Универсальный доступ»: без него
            // перехват клавиш молчит.
            #[cfg(target_os = "macos")]
            {
                app.set_activation_policy(tauri::ActivationPolicy::Accessory);
                mac::request_accessibility();
            }
            tray::build(app)?;
            sync_autostart(app.handle());
            search::create(app)?;

            let handle = app.handle().clone();
            engine::reload(&handle);
            let (tx, rx) = mpsc::channel();
            app.manage(commands::JobTx(tx.clone()));
            executor::spawn(handle.clone(), rx);
            hook::start(shared, tx);
            engine::watch(handle.clone());

            if first_run {
                tray::show_main(&handle);
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("Отклик не запустился");
}
