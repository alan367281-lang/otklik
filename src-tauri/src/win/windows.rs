//! Список открытых окон — для выбора «где работает правило».

use super::foreground::exe_of;
use serde::Serialize;
use std::collections::BTreeMap;
use windows_sys::Win32::Foundation::{HWND, LPARAM};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindow, GetWindowLongW, GetWindowTextW, GetWindowThreadProcessId,
    IsWindowVisible, GWL_EXSTYLE, GW_OWNER, WS_EX_TOOLWINDOW,
};

#[derive(Debug, Clone, Serialize)]
pub struct RunningApp {
    /// Имя exe так, как его писать в правиле (в нижнем регистре).
    pub exe: String,
    /// Заголовок одного из окон — чтобы человек узнал программу.
    pub title: String,
}

unsafe extern "system" fn collect(hwnd: HWND, lparam: LPARAM) -> i32 {
    let out = &mut *(lparam as *mut Vec<(u32, String)>);
    // Только «настоящие» окна: видимые, без владельца, не служебные панели.
    if IsWindowVisible(hwnd) == 0 || !GetWindow(hwnd, GW_OWNER).is_null() {
        return 1;
    }
    if GetWindowLongW(hwnd, GWL_EXSTYLE) as u32 & WS_EX_TOOLWINDOW != 0 {
        return 1;
    }
    let mut buf = [0u16; 256];
    let n = GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32).max(0) as usize;
    if n == 0 {
        return 1;
    }
    let mut pid = 0u32;
    GetWindowThreadProcessId(hwnd, &mut pid);
    out.push((pid, String::from_utf16_lossy(&buf[..n])));
    1
}

pub fn running_apps() -> Vec<RunningApp> {
    let mut raw: Vec<(u32, String)> = Vec::new();
    unsafe { EnumWindows(Some(collect), &mut raw as *mut _ as LPARAM) };
    let own = std::process::id();
    let mut by_exe: BTreeMap<String, String> = BTreeMap::new();
    for (pid, title) in raw {
        if pid == own {
            continue;
        }
        let exe = exe_of(pid);
        // Проводник держит «окно» рабочего стола — оно человеку не нужно.
        if exe.is_empty() || title == "Program Manager" {
            continue;
        }
        by_exe.entry(exe).or_insert(title);
    }
    by_exe
        .into_iter()
        .map(|(exe, title)| RunningApp { exe, title })
        .collect()
}
