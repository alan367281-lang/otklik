//! Перехват на Windows: WH_KEYBOARD_LL и WH_MOUSE_LL. Здесь только перевод
//! событий Windows в общий вид; решения — в `crate::hook`.
//!
//! Проверено на деле: пока в фокусе окно самого Отклика, Windows хук
//! не вызывает вовсе. Поэтому сочетание для настроек записывает окно, а
//! не хук, и сочетания не срабатывают внутри окна настроек.

use super::{input, modifiers};
use crate::chord::{VK_CANCEL, VK_PAUSE};
use crate::hook::{forget_typed, on_key_down, on_key_up};
use std::cell::RefCell;
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicU32, Ordering};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::SystemInformation::GetTickCount;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, GetKeyboardLayout, GetLastInputInfo, ToUnicodeEx, LASTINPUTINFO,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetForegroundWindow, GetMessageW, GetWindowThreadProcessId, SetTimer,
    SetWindowsHookExW, UnhookWindowsHookEx, HC_ACTION, HHOOK, KBDLLHOOKSTRUCT, MSG, WH_KEYBOARD_LL,
    WH_MOUSE_LL, WM_KEYDOWN, WM_LBUTTONDOWN, WM_MBUTTONDOWN, WM_RBUTTONDOWN, WM_SYSKEYDOWN,
    WM_TIMER,
};

/// Когда хук вызывался в последний раз (GetTickCount) — для сторожа.
static LAST_CALL: AtomicU32 = AtomicU32::new(0);
/// Клавиша, которую другие программы шлют с символом вместо кода
/// (эмуляция юникодом): символ лежит в скан-коде.
const VK_PACKET: u16 = 0xE7;

thread_local! {
    static HOOKS: RefCell<[HHOOK; 2]> = const { RefCell::new([null_mut(); 2]) };
}

pub fn run() {
    std::thread::Builder::new()
        .name("otk-hook".into())
        .spawn(|| unsafe {
            install();
            // Сторож раз в 5 секунд. Таймер без окна приходит в очередь потока.
            SetTimer(null_mut(), 0, 5000, None);
            let mut msg: MSG = std::mem::zeroed();
            while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
                if msg.message == WM_TIMER {
                    watchdog();
                }
            }
        })
        .expect("не удалось запустить поток перехвата клавиатуры");
}

unsafe fn install() {
    HOOKS.with(|h| {
        let mut h = h.borrow_mut();
        for old in h.iter() {
            if !old.is_null() {
                UnhookWindowsHookEx(*old);
            }
        }
        let module = GetModuleHandleW(null());
        h[0] = SetWindowsHookExW(WH_KEYBOARD_LL, Some(key_proc), module, 0);
        h[1] = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), module, 0);
        if h[0].is_null() {
            eprintln!("[otk] err не удалось поставить перехват клавиатуры");
        }
    });
    LAST_CALL.store(GetTickCount(), Ordering::Relaxed);
}

/// Windows не сообщает, что снял хук. Косвенный признак: клавиатурой или
/// мышью пользовались, а хук давно не вызывался. Тогда ставим его заново —
/// это дёшево и безвредно, даже если человек просто работал мышью.
unsafe fn watchdog() {
    let mut info = LASTINPUTINFO {
        cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
        dwTime: 0,
    };
    if GetLastInputInfo(&mut info) == 0 {
        return;
    }
    let last_call = LAST_CALL.load(Ordering::Relaxed);
    let now = GetTickCount();
    let input_after_call = info.dwTime.wrapping_sub(last_call) as i32 > 0;
    if input_after_call && now.wrapping_sub(last_call) > 60_000 {
        install();
    }
}

unsafe extern "system" fn key_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        let kb = &*(lparam as *const KBDLLHOOKSTRUCT);
        LAST_CALL.store(kb.time, Ordering::Relaxed);
        if handle(wparam as u32, kb) {
            return 1;
        }
    }
    CallNextHookEx(null_mut(), code, wparam, lparam)
}

/// Щелчок мышью обычно переносит курсор ввода — набранное до щелчка к
/// тому, что будет набрано после, уже не относится.
unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32
        && matches!(
            wparam as u32,
            WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN
        )
    {
        forget_typed();
    }
    CallNextHookEx(null_mut(), code, wparam, lparam)
}

/// true — проглотить нажатие (приложение его не увидит).
fn handle(msg: u32, kb: &KBDLLHOOKSTRUCT) -> bool {
    if kb.dwExtraInfo == input::TAG {
        return false;
    }
    let vk = (kb.vkCode & 0xFF) as u16;
    if modifiers::is_modifier(vk) {
        return false;
    }
    if msg != WM_KEYDOWN && msg != WM_SYSKEYDOWN {
        return on_key_up(vk);
    }
    // Ctrl+Pause Windows присылает как Break — переводим обратно в Pause.
    let key = if vk == VK_CANCEL { VK_PAUSE } else { vk };
    let mods = modifiers::current();
    let hwnd = unsafe { GetForegroundWindow() };
    let shift = mods & crate::chord::SHIFT != 0;
    on_key_down(key, mods, hwnd as usize, || {
        if vk == VK_PACKET {
            char::from_u32(kb.scanCode)
        } else {
            to_char(vk, kb.scanCode, shift, hwnd)
        }
    })
}

/// Какой символ даст клавиша в раскладке активного окна. Раскладку
/// берём у окна, а не у себя: у каждого окна она своя (RU в Telegram,
/// EN в редакторе кода).
fn to_char(vk: u16, scan: u32, shift: bool, hwnd: HWND) -> Option<char> {
    unsafe {
        let thread = GetWindowThreadProcessId(hwnd, null_mut());
        let layout = GetKeyboardLayout(thread);
        let mut state = [0u8; 256];
        if shift {
            state[0x10] = 0x80;
            state[0xA0] = 0x80;
        }
        if GetKeyState(0x14) & 1 != 0 {
            state[0x14] = 0x01; // Caps Lock включён
        }
        let mut buf = [0u16; 8];
        // Флаг 0x4 — не менять состояние клавиатуры ядра: иначе наш вызов
        // «съел» бы мёртвую клавишу, и «ё» или «é» у человека не набрались бы.
        let n = ToUnicodeEx(
            vk as u32,
            scan,
            state.as_ptr(),
            buf.as_mut_ptr(),
            buf.len() as i32,
            0x4,
            layout,
        );
        if n <= 0 {
            return None;
        }
        char::decode_utf16(buf[..n as usize].iter().copied())
            .next()
            .and_then(Result::ok)
    }
}
