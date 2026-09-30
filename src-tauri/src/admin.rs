//! Права администратора. Windows не пускает ввод от обычной программы в
//! окна, запущенные от администратора (UIPI), — там Key Manager молчит.
//! Решение одно: работать от администратора самому. Здесь — узнать, как
//! мы запущены, перезапуститься с правами и запускаться так при входе.
//!
//! Обычный автозапуск (реестр Run) права поднимать не умеет, поэтому для
//! запуска от администратора — задача Планировщика с наивысшими правами.

use crate::win::wide;
use serde::Serialize;
use std::os::windows::process::CommandExt;
use std::process::Command;
use std::ptr::{null, null_mut};
use windows_sys::Win32::UI::Shell::{IsUserAnAdmin, ShellExecuteW};
use windows_sys::Win32::UI::WindowsAndMessaging::SW_HIDE;

const TASK: &str = "Key Manager (администратор)";
/// Не показывать чёрное окно консоли при вызове schtasks.
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminStatus {
    elevated: bool,
    task: bool,
    exe: String,
}

pub fn is_elevated() -> bool {
    unsafe { IsUserAnAdmin() != 0 }
}

fn exe() -> String {
    std::env::current_exe()
        .map(|p| p.display().to_string())
        .unwrap_or_default()
}

fn task_exists() -> bool {
    Command::new("schtasks")
        .args(["/Query", "/TN", TASK])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .is_ok_and(|o| o.status.success())
}

/// Запустить с запросом прав (окно UAC). Ok — человек согласился и запуск
/// пошёл; отказ в UAC — ошибка.
fn run_elevated(file: &str, args: &str) -> Result<(), String> {
    let (verb, file_w, args_w) = (wide("runas"), wide(file), wide(args));
    let code = unsafe {
        ShellExecuteW(
            null_mut(),
            verb.as_ptr(),
            file_w.as_ptr(),
            args_w.as_ptr(),
            null(),
            SW_HIDE,
        )
    } as isize;
    if code > 32 {
        Ok(())
    } else if code == 5 {
        Err("права администратора не выданы".into())
    } else {
        Err(format!("не удалось запустить (код {code})"))
    }
}

/// Описание задачи. XML, а не параметры schtasks: только так можно снять
/// «запускать только от сети» — иначе на ноутбуке от батареи задача
/// молча не стартует — и снять ограничение в 72 часа работы.
fn task_xml(exe: &str) -> String {
    let user = std::env::var("USERDOMAIN")
        .map(|d| format!("{d}\\{}", std::env::var("USERNAME").unwrap_or_default()))
        .unwrap_or_default();
    let esc = |s: &str| {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
    };
    format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo><Description>Key Manager с правами администратора — чтобы сочетания работали и в окнах администратора.</Description></RegistrationInfo>
  <Triggers><LogonTrigger><Enabled>true</Enabled><UserId>{user}</UserId><Delay>PT5S</Delay></LogonTrigger></Triggers>
  <Principals><Principal id="Author"><UserId>{user}</UserId><LogonType>InteractiveToken</LogonType><RunLevel>HighestAvailable</RunLevel></Principal></Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <Priority>7</Priority>
  </Settings>
  <Actions Context="Author"><Exec><Command>"{exe}"</Command></Exec></Actions>
</Task>"#,
        user = esc(&user),
        exe = esc(exe),
    )
}

#[tauri::command]
pub fn admin_status() -> AdminStatus {
    AdminStatus {
        elevated: is_elevated(),
        task: task_exists(),
        exe: exe(),
    }
}

/// Включить/выключить запуск от администратора при входе. Задачу с
/// наивысшими правами Windows разрешает создать только с правами — будет
/// окно UAC. Результат окно узнаёт повторным admin_status.
#[tauri::command]
pub fn admin_set_task(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    if !enabled {
        return run_elevated("schtasks.exe", &format!("/Delete /TN \"{TASK}\" /F"));
    }
    // schtasks ждёт XML в UTF-16 с меткой порядка байтов.
    let path = std::env::temp_dir().join("key-manager-task.xml");
    let mut bytes = vec![0xFF, 0xFE];
    for u in task_xml(&exe()).encode_utf16() {
        bytes.extend_from_slice(&u.to_le_bytes());
    }
    std::fs::write(&path, bytes).map_err(|e| format!("не записать описание задачи: {e}"))?;
    run_elevated(
        "schtasks.exe",
        &format!("/Create /TN \"{TASK}\" /XML \"{}\" /F", path.display()),
    )?;
    // Обычный автозапуск больше не нужен: иначе при входе стартовали бы
    // две копии, и вторая открыла бы окно первой.
    use tauri_plugin_autostart::ManagerExt;
    let _ = app.autolaunch().disable();
    Ok(())
}

/// Перезапуститься от администратора. Новая копия не может стартовать,
/// пока жива эта (одна копия на компьютер), поэтому запускаем её через
/// cmd с паузой в 2 секунды, а сами сразу выходим.
#[tauri::command]
pub fn admin_restart(app: tauri::AppHandle) -> Result<(), String> {
    let args = format!("/c timeout /t 2 /nobreak >nul & start \"\" \"{}\"", exe());
    run_elevated("cmd.exe", &args)?;
    app.exit(0);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_xml_escapes_path() {
        let x = task_xml(r"C:\Program Files\A&B\key-manager.exe");
        assert!(x.contains(r#"<Command>"C:\Program Files\A&amp;B\key-manager.exe"</Command>"#));
        assert!(x.contains("<DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>"));
        assert!(x.contains("<RunLevel>HighestAvailable</RunLevel>"));
    }
}
