//! Key Manager: сочетание клавиш → действие (пока — вставка текста).
//!
//! Потоки: km-hook (перехват клавиатуры) → канал → km-exec (исполнение),
//! km-watch (следит за config.yaml). Подробно — docs/ARCHITECTURE.md.

pub mod chord;
pub mod config;
pub mod rules;

mod commands;
mod engine;
mod executor;
mod hook;
mod tray;
mod win;

use std::sync::{mpsc, Arc};
use tauri::{Manager, WindowEvent};

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
            let first_run = match config::ensure_exists(&path) {
                Ok(created) => created,
                Err(e) => {
                    eprintln!("[km] err {e}");
                    false
                }
            };
            let shared = Arc::new(engine::Shared::new(path));
            app.manage(shared.clone());
            tray::build(app)?;

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
        .expect("Key Manager не запустился");
}
