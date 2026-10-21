//! Обновления: проверить на GitHub, сообщить, по кнопке — скачать,
//! поставить и перезапуститься. Ставим только по согласию человека.
//!
//! Где лежит новая версия — `latest.json` последнего выпуска (адрес в
//! tauri.conf.json). Каждое обновление подписано ключом; открытый ключ
//! вшит в программу, поэтому подменить обновление чужим файлом нельзя.

use crate::engine;
use parking_lot::Mutex;
use serde::Serialize;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};

/// Как часто проверять, пока программа работает.
const EVERY: Duration = Duration::from_secs(6 * 60 * 60);
/// Первая проверка — не сразу при запуске: дать системе и сети проснуться.
const FIRST: Duration = Duration::from_secs(20);

/// Найденное, но ещё не поставленное обновление.
#[derive(Default)]
pub struct Pending(Mutex<Option<Update>>);

impl Pending {
    pub fn has(&self) -> bool {
        self.0.lock().is_some()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct UpdateInfo {
    pub version: String,
    pub current: String,
    pub notes: Option<String>,
}

fn info(u: &Update) -> UpdateInfo {
    UpdateInfo {
        version: u.version.clone(),
        current: u.current_version.clone(),
        notes: u.body.clone(),
    }
}

/// Проверить. Нашлось — запомнить, сообщить окну и трею. `quiet` — без
/// уведомления «у вас последняя версия» (для фоновых проверок).
pub async fn check(app: &AppHandle, quiet: bool) -> Result<Option<UpdateInfo>, String> {
    let updater = app
        .updater()
        .map_err(|e| format!("обновления недоступны: {e}"))?;
    let found = updater
        .check()
        .await
        .map_err(|e| format!("не удалось проверить обновления: {e}"))?;
    let Some(update) = found else {
        if !quiet {
            engine::log(app, "info", "Обновлений нет — у вас последняя версия");
        }
        return Ok(None);
    };
    let i = info(&update);
    let already = app
        .state::<Pending>()
        .0
        .lock()
        .as_ref()
        .is_some_and(|p| p.version == i.version);
    *app.state::<Pending>().0.lock() = Some(update);
    crate::tray::show_update(app, Some(&i.version));
    let _ = app.emit("update:available", i.clone());
    if !already {
        engine::log(app, "info", format!("Доступна версия {}", i.version));
        engine::notify(
            app,
            &format!(
                "Доступна версия {} — обновить можно в окне Отклика или из трея",
                i.version
            ),
        );
    }
    Ok(Some(i))
}

/// Фоновая проверка: через 20 секунд после запуска и дальше раз в 6 часов.
pub fn spawn_checker(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(FIRST).await;
        loop {
            if let Err(e) = check(&app, true).await {
                // Нет сети — обычное дело, в журнал без уведомлений.
                eprintln!("[otk] info {e}");
            }
            tokio::time::sleep(EVERY).await;
        }
    });
}

/// Скачать, поставить и перезапуститься.
pub async fn install(app: &AppHandle) -> Result<(), String> {
    let update = app.state::<Pending>().0.lock().take();
    let update = match update {
        Some(u) => u,
        None => match check(app, true).await? {
            Some(_) => app
                .state::<Pending>()
                .0
                .lock()
                .take()
                .ok_or("обновление пропало — попробуйте ещё раз")?,
            None => return Err("обновлений нет — у вас последняя версия".into()),
        },
    };
    let version = update.version.clone();
    engine::log(app, "info", format!("Скачиваю и ставлю версию {version}…"));
    let _ = app.emit("update:installing", version.clone());
    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|e| format!("не удалось поставить обновление: {e}"))?;
    engine::log(
        app,
        "info",
        format!("Версия {version} поставлена, перезапускаюсь"),
    );
    app.restart();
}

#[tauri::command]
pub fn app_version(app: AppHandle) -> String {
    app.package_info().version.to_string()
}

/// Найденное раньше обновление (окно открылось позже проверки).
#[tauri::command]
pub fn update_status(app: AppHandle) -> Option<UpdateInfo> {
    app.state::<Pending>().0.lock().as_ref().map(info)
}

#[tauri::command]
pub async fn update_check(app: AppHandle) -> Result<Option<UpdateInfo>, String> {
    check(&app, false).await
}

#[tauri::command]
pub async fn update_install(app: AppHandle) -> Result<(), String> {
    install(&app).await
}
