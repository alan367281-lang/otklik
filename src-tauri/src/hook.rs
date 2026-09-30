//! Низкоуровневый перехват клавиатуры (WH_KEYBOARD_LL).
//!
//! Главное правило: обработчик возвращается мгновенно. Если он думает
//! дольше LowLevelHooksTimeout (от 300 мс), Windows молча снимает хук, и
//! программа «перестаёт работать» без единой ошибки. Поэтому здесь только
//! сопоставление, а всё исполнение — в потоке исполнителя через канал.

use crate::chord::{Chord, ALT, VK_CANCEL, VK_PAUSE, WIN};
use crate::engine::Shared;
use crate::executor::Job;
use crate::win::{foreground, input, modifiers};
use std::cell::RefCell;
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, OnceLock};
use windows_sys::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::SystemInformation::GetTickCount;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetMessageW, SetTimer, SetWindowsHookExW, UnhookWindowsHookEx, HC_ACTION,
    HHOOK, KBDLLHOOKSTRUCT, MSG, WH_KEYBOARD_LL, WM_KEYDOWN, WM_SYSKEYDOWN, WM_TIMER,
};

struct Ctx {
    shared: Arc<Shared>,
    tx: Sender<Job>,
}

static CTX: OnceLock<Ctx> = OnceLock::new();
/// Когда хук вызывался в последний раз (GetTickCount) — для сторожа.
static LAST_CALL: AtomicU32 = AtomicU32::new(0);

thread_local! {
    /// Клавиши, чьё нажатие мы проглотили: их отпускание тоже глотаем,
    /// а повторы при удержании — игнорируем (иначе Alt+1, зажатый на
    /// полсекунды, вставил бы текст десять раз).
    static SWALLOWED: RefCell<[bool; 256]> = const { RefCell::new([false; 256]) };
    static HOOK: RefCell<HHOOK> = const { RefCell::new(null_mut()) };
}

pub fn start(shared: Arc<Shared>, tx: Sender<Job>) {
    if CTX.set(Ctx { shared, tx }).is_err() {
        return;
    }
    std::thread::Builder::new()
        .name("km-hook".into())
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
    HOOK.with(|h| {
        let mut h = h.borrow_mut();
        if !h.is_null() {
            UnhookWindowsHookEx(*h);
        }
        *h = SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook_proc), GetModuleHandleW(null()), 0);
        if h.is_null() {
            eprintln!("[km] err не удалось поставить перехват клавиатуры");
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

unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        let kb = &*(lparam as *const KBDLLHOOKSTRUCT);
        LAST_CALL.store(kb.time, Ordering::Relaxed);
        if handle(wparam as u32, kb) {
            return 1;
        }
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
    let down = msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN;

    SWALLOWED.with(|sw| {
        let mut sw = sw.borrow_mut();
        let idx = vk as usize;
        if !down {
            return std::mem::replace(&mut sw[idx], false);
        }
        if sw[idx] {
            return true; // автоповтор удерживаемого сочетания
        }

        let key = if vk == VK_CANCEL { VK_PAUSE } else { vk };
        let mods = modifiers::current();
        let bare_ok = (0x70..=0x87).contains(&key) || key == VK_PAUSE;
        if mods == 0 && !bare_ok {
            return false; // обычный набор текста — самый частый путь, выходим сразу
        }
        let Some(ctx) = CTX.get() else { return false };
        // try_read: если правила прямо сейчас подменяются, лучше пропустить
        // одно нажатие, чем заставить хук ждать.
        let Some(compiled) = ctx.shared.rules.try_read().map(|g| Arc::clone(&g)) else {
            return false;
        };
        let chord = Chord::new(mods, key);

        let job = if compiled.pause == Some(chord) {
            Job::TogglePause
        } else {
            if ctx.shared.paused.load(Ordering::Relaxed) || !compiled.by_chord.contains_key(&chord) {
                return false;
            }
            let app = foreground::active_app();
            let Some(rule) = compiled.pick(&chord, &app) else {
                return false; // сочетание не для этого приложения — пусть работает как обычно
            };
            Job::Run { rule, compiled, app }
        };

        if ctx.tx.send(job).is_err() {
            return false;
        }
        sw[idx] = true;
        if chord.has(ALT) || chord.has(WIN) {
            input::mask_menu();
        }
        true
    })
}
