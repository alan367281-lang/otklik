//! Открыть программу, папку, файл или ссылку — как «Выполнить» (Win+R).

use super::wide;
use std::path::Path;
use std::ptr::{null, null_mut};
use windows_sys::Win32::System::Environment::ExpandEnvironmentStringsW;
use windows_sys::Win32::UI::Shell::ShellExecuteW;
use windows_sys::Win32::UI::WindowsAndMessaging::{AllowSetForegroundWindow, SW_SHOWNORMAL};

/// `%USERPROFILE%\Desktop` → `C:\Users\…\Desktop`.
fn expand_env(s: &str) -> String {
    let src = wide(s);
    let mut buf = vec![0u16; 4096];
    let n = unsafe { ExpandEnvironmentStringsW(src.as_ptr(), buf.as_mut_ptr(), buf.len() as u32) };
    if n == 0 || n as usize > buf.len() {
        return s.to_string();
    }
    String::from_utf16_lossy(&buf[..n as usize - 1])
}

pub fn open(target: &str, args: Option<&str>) -> Result<(), String> {
    let target = expand_env(target.trim().trim_matches('"'));
    // Рабочая папка программы — её собственная папка: так многие старые
    // программы находят свои файлы рядом с собой.
    let dir = Path::new(&target)
        .parent()
        .filter(|p| p.is_dir())
        .map(|p| p.display().to_string());
    let file = wide(&target);
    let params = args.map(|a| wide(&expand_env(a)));
    let dir_w = dir.as_deref().map(wide);
    let verb = wide("open");
    // Мы в фоне, а открываемая программа должна выйти на передний план —
    // разрешаем ей это явно, иначе Windows может оставить её позади.
    unsafe { AllowSetForegroundWindow(u32::MAX) };
    let code = unsafe {
        ShellExecuteW(
            null_mut(),
            verb.as_ptr(),
            file.as_ptr(),
            params.as_ref().map_or(null(), |p| p.as_ptr()),
            dir_w.as_ref().map_or(null(), |d| d.as_ptr()),
            SW_SHOWNORMAL,
        )
    } as isize;
    if code > 32 {
        return Ok(());
    }
    Err(match code {
        2 | 3 => format!("не найдено: {target}"),
        5 => format!("нет доступа: {target}"),
        31 => format!("нет программы, которая открывает {target}"),
        _ => format!("Windows не смогла открыть {target} (код {code})"),
    })
}
