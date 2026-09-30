//! Какое приложение сейчас в фокусе: имя exe и заголовок окна.

use crate::rules::ActiveApp;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::OnceLock;
use windows_sys::Win32::Foundation::CloseHandle;
use windows_sys::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId,
};

/// pid → exe. Узнать имя процесса — это открыть процесс, а хук должен
/// отвечать мгновенно; имя по pid не меняется, так что кэшируем.
fn cache() -> &'static Mutex<HashMap<u32, String>> {
    static C: OnceLock<Mutex<HashMap<u32, String>>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn exe_of(pid: u32) -> String {
    if let Some(e) = cache().lock().get(&pid) {
        return e.clone();
    }
    let exe = unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if h.is_null() {
            return String::new();
        }
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, buf.as_mut_ptr(), &mut len);
        CloseHandle(h);
        if ok == 0 {
            return String::new();
        }
        let full = String::from_utf16_lossy(&buf[..len as usize]);
        full.rsplit(['\\', '/']).next().unwrap_or("").to_lowercase()
    };
    let mut c = cache().lock();
    // pid переиспользуются, но редко; просто не даём кэшу расти бесконечно.
    if c.len() > 512 {
        c.clear();
    }
    c.insert(pid, exe.clone());
    exe
}

/// Активное окно сейчас — для сверки перед печатью.
pub fn current_window() -> usize {
    unsafe { GetForegroundWindow() as usize }
}

pub fn active_app() -> ActiveApp {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_null() {
            return ActiveApp::default();
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, &mut pid);
        let mut buf = [0u16; 512];
        let n = GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32).max(0) as usize;
        ActiveApp {
            exe: exe_of(pid),
            title: String::from_utf16_lossy(&buf[..n]).to_lowercase(),
            hwnd: hwnd as usize,
        }
    }
}
