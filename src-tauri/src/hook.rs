//! Низкоуровневый перехват клавиатуры (WH_KEYBOARD_LL) и мыши (WH_MOUSE_LL).
//!
//! Главное правило: обработчик возвращается мгновенно. Если он думает
//! дольше LowLevelHooksTimeout (от 300 мс), Windows молча снимает хук, и
//! программа «перестаёт работать» без единой ошибки. Поэтому здесь только
//! сопоставление, а всё исполнение — в потоке исполнителя через канал.
//!
//! Проверено на деле: пока в фокусе окно самого Key Manager, Windows хук
//! не вызывает вовсе. Поэтому сочетание для настроек записывает окно, а
//! не хук, и сочетания не срабатывают внутри окна настроек.
//!
//! Сокращения: хук помнит последние набранные символы — только в памяти,
//! никуда не пишет. Память сбрасывается щелчком мыши, сменой окна,
//! стрелками, Esc и сочетаниями с Ctrl/Alt/Win.

use crate::chord::{Chord, ALT, CTRL, SHIFT, VK_CANCEL, VK_PAUSE, WIN};
use crate::engine::Shared;
use crate::executor::Job;
use crate::win::{foreground, input, modifiers};
use std::cell::RefCell;
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, OnceLock};
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

struct Ctx {
    shared: Arc<Shared>,
    tx: Sender<Job>,
}

static CTX: OnceLock<Ctx> = OnceLock::new();
/// Когда хук вызывался в последний раз (GetTickCount) — для сторожа.
static LAST_CALL: AtomicU32 = AtomicU32::new(0);

/// Сколько последних символов помнить: длиннее сокращений не бывает.
const TYPED_MAX: usize = 64;
/// Клавиша, которую другие программы шлют с символом вместо кода
/// (эмуляция юникодом): символ лежит в скан-коде.
const VK_PACKET: u16 = 0xE7;

/// Набранное в текущем окне — для сокращений.
struct Typed {
    chars: Vec<char>,
    hwnd: usize,
}

thread_local! {
    /// Клавиши, чьё нажатие мы проглотили: их отпускание тоже глотаем,
    /// а повторы при удержании — игнорируем (иначе Alt+1, зажатый на
    /// полсекунды, вставил бы текст десять раз).
    static SWALLOWED: RefCell<[bool; 256]> = const { RefCell::new([false; 256]) };
    static HOOKS: RefCell<[HHOOK; 2]> = const { RefCell::new([null_mut(); 2]) };
    static TYPED: RefCell<Typed> = const { RefCell::new(Typed { chars: Vec::new(), hwnd: 0 }) };
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

fn forget_typed() {
    TYPED.with(|t| t.borrow_mut().chars.clear());
}

/// Отладка в режиме разработки: что хук увидел и что решил. Пишем только
/// сочетания с Ctrl, Alt или Win — обычный набор текста (в том числе
/// заглавные буквы через Shift) сюда не попадает.
fn trace(chord: &Chord, msg: impl FnOnce() -> String) {
    if cfg!(debug_assertions) && chord.mods & (CTRL | ALT | WIN) != 0 {
        eprintln!("[km] hook {chord}: {}", msg());
    }
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
        let Some(ctx) = CTX.get() else { return false };
        let key = if vk == VK_CANCEL { VK_PAUSE } else { vk };
        let mods = modifiers::current();
        let fkey = (0x70..=0x87).contains(&key) || key == VK_PAUSE;

        let swallow = if mods & (CTRL | ALT | WIN) != 0 || fkey {
            // Сочетание — это не набор текста.
            forget_typed();
            on_chord(ctx, Chord::new(mods, key))
        } else {
            on_typing(ctx, vk, kb, mods & SHIFT != 0)
        };
        if swallow {
            sw[idx] = true;
        }
        swallow
    })
}

/// Сочетание клавиш: есть правило — выполняем и глотаем нажатие.
fn on_chord(ctx: &Ctx, chord: Chord) -> bool {
    // try_read: если правила прямо сейчас подменяются, лучше пропустить
    // одно нажатие, чем заставить хук ждать.
    let Some(compiled) = ctx.shared.rules.try_read().map(|g| Arc::clone(&g)) else {
        trace(&chord, || "правила подменяются, пропускаю".into());
        return false;
    };

    let job = if compiled.pause == Some(chord) {
        Job::TogglePause
    } else if compiled.search == Some(chord) {
        if ctx.shared.paused.load(Ordering::Relaxed) {
            return false;
        }
        Job::ShowSearch(foreground::active_app())
    } else {
        if ctx.shared.paused.load(Ordering::Relaxed) {
            trace(&chord, || "пауза".into());
            return false;
        }
        if !compiled.by_chord.contains_key(&chord) {
            trace(&chord, || "нет правила".into());
            return false;
        }
        let app = foreground::active_app();
        let Some(rule) = compiled.pick(&chord, &app) else {
            trace(&chord, || format!("правило не для {}", app.exe));
            return false; // сочетание не для этого приложения — пусть работает как обычно
        };
        trace(&chord, || format!("«{}» в {}", rule.name, app.exe));
        Job::Run {
            rule,
            compiled,
            app,
            erase: 0,
            suffix: None,
        }
    };

    if ctx.tx.send(job).is_err() {
        return false;
    }
    if chord.has(ALT) || chord.has(WIN) {
        input::mask_menu();
    }
    true
}

/// Обычный набор: запоминаем символ и проверяем сокращения. Совпало —
/// глотаем последнюю клавишу, исполнитель сотрёт набранную часть и вставит.
fn on_typing(ctx: &Ctx, vk: u16, kb: &KBDLLHOOKSTRUCT, shift: bool) -> bool {
    if ctx.shared.paused.load(Ordering::Relaxed) {
        forget_typed();
        return false;
    }
    let Some(compiled) = ctx.shared.rules.try_read().map(|g| Arc::clone(&g)) else {
        return false;
    };
    if !compiled.has_abbrs() {
        return false;
    }
    let hwnd = unsafe { GetForegroundWindow() };

    TYPED.with(|t| {
        let mut t = t.borrow_mut();
        if t.hwnd != hwnd as usize {
            t.chars.clear();
            t.hwnd = hwnd as usize;
        }
        match vk {
            0x08 => {
                t.chars.pop(); // Backspace
                return false;
            }
            // Стрелки, Home/End, PgUp/PgDn, Insert/Delete, Esc — курсор ушёл
            // или набор прерван.
            0x1B | 0x21..=0x28 | 0x2D | 0x2E => {
                t.chars.clear();
                return false;
            }
            _ => {}
        }
        let ch = if vk == VK_PACKET {
            char::from_u32(kb.scanCode)
        } else {
            to_char(vk, kb.scanCode, shift, hwnd)
        };
        // Нет символа (мёртвая клавиша для «ё»/умлаутов, служебная клавиша) —
        // набранное не трогаем.
        let Some(ch) = ch else { return false };

        let mut app = None;
        let hit = compiled.match_abbr(&t.chars, ch, || {
            let a = foreground::active_app();
            app = Some(a.clone());
            a
        });
        let Some(hit) = hit else {
            t.chars.push(ch);
            if t.chars.len() > TYPED_MAX {
                t.chars.remove(0);
            }
            return false;
        };
        t.chars.clear();
        let job = Job::Run {
            rule: hit.rule,
            compiled,
            app: app.unwrap_or_default(),
            erase: hit.erase,
            suffix: hit.suffix,
        };
        ctx.tx.send(job).is_ok()
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
