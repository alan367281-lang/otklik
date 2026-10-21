//! Те же команды, что `win/admin.rs`, для macOS. Прав администратора здесь
//! нет и не нужно: вместо них разрешение «Универсальный доступ», которое
//! окно показывает отдельной карточкой.

use serde::Serialize;

const NOT_ON_MAC: &str =
    "на macOS это не нужно: вместо прав администратора — разрешение «Универсальный доступ»";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminStatus {
    elevated: bool,
    task: bool,
    exe: String,
}

#[tauri::command]
pub fn admin_status() -> AdminStatus {
    AdminStatus {
        elevated: false,
        task: false,
        exe: std::env::current_exe()
            .map(|p| p.display().to_string())
            .unwrap_or_default(),
    }
}

#[tauri::command]
pub fn admin_set_task(enabled: bool) -> Result<(), String> {
    let _ = enabled;
    Err(NOT_ON_MAC.into())
}

#[tauri::command]
pub fn admin_restart() -> Result<(), String> {
    Err(NOT_ON_MAC.into())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformInfo {
    os: &'static str,
    /// Дано ли разрешение «Универсальный доступ».
    trusted: bool,
}

#[tauri::command]
pub fn platform_info() -> PlatformInfo {
    PlatformInfo {
        os: "macos",
        trusted: super::accessibility_trusted(),
    }
}

/// Открыть Настройки → Конфиденциальность → Универсальный доступ.
#[tauri::command]
pub fn open_permissions() {
    super::open_accessibility_settings();
}
