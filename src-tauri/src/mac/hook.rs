#![allow(non_upper_case_globals)]

//! Перехват на macOS: CGEventTap. Здесь только перевод событий Quartz в
//! общий вид; решения — в `crate::hook`.
//!
//! Перехват требует разрешения «Универсальный доступ». Пока его нет,
//! CGEventTapCreate возвращает пусто — поток ждёт и пробует снова, так что
//! после выдачи разрешения всё начинает работать без перезапуска.
//!
//! Если обработчик думает слишком долго, macOS отключает перехват и
//! присылает событие «отключён по таймауту» — включаем обратно.

use super::ffi::*;
use super::{input, keys};
use crate::chord::{ALT, CTRL, SHIFT, WIN};
use crate::hook::{forget_typed, on_key_down, on_key_up};
use std::ptr::null_mut;
use std::sync::atomic::{AtomicPtr, Ordering};
use std::time::Duration;

/// Сам перехват — чтобы включить его обратно после отключения системой.
static TAP: AtomicPtr<std::ffi::c_void> = AtomicPtr::new(null_mut());

pub fn run() {
    std::thread::Builder::new()
        .name("otk-hook".into())
        .spawn(|| unsafe {
            let mask: u64 = (1 << kCGEventKeyDown)
                | (1 << kCGEventKeyUp)
                | (1 << kCGEventLeftMouseDown)
                | (1 << kCGEventRightMouseDown)
                | (1 << kCGEventOtherMouseDown);
            let tap = loop {
                let tap = CGEventTapCreate(
                    kCGSessionEventTap,
                    kCGHeadInsertEventTap,
                    kCGEventTapOptionDefault,
                    mask,
                    callback,
                    null_mut(),
                );
                if !tap.is_null() {
                    break tap;
                }
                // Нет разрешения «Универсальный доступ» — ждём, пока дадут.
                std::thread::sleep(Duration::from_secs(3));
            };
            TAP.store(tap, Ordering::SeqCst);
            let source = CFMachPortCreateRunLoopSource(std::ptr::null(), tap, 0);
            CFRunLoopAddSource(CFRunLoopGetCurrent(), source, kCFRunLoopCommonModes);
            CGEventTapEnable(tap, true);
            eprintln!("[otk] info перехват клавиатуры включён");
            CFRunLoopRun();
        })
        .expect("не удалось запустить поток перехвата клавиатуры");
}

fn mods_of(flags: u64) -> u8 {
    let mut m = 0;
    if flags & kCGEventFlagMaskControl != 0 {
        m |= CTRL;
    }
    if flags & kCGEventFlagMaskAlternate != 0 {
        m |= ALT;
    }
    if flags & kCGEventFlagMaskShift != 0 {
        m |= SHIFT;
    }
    if flags & kCGEventFlagMaskCommand != 0 {
        m |= WIN;
    }
    m
}

/// Символ, который даст нажатие, — macOS кладёт его прямо в событие, уже
/// с учётом раскладки и Shift.
unsafe fn char_of(event: CGEventRef) -> Option<char> {
    let mut buf = [0u16; 4];
    let mut len = 0usize;
    CGEventKeyboardGetUnicodeString(event, buf.len(), &mut len, buf.as_mut_ptr());
    if len == 0 {
        return None;
    }
    char::decode_utf16(buf[..len].iter().copied())
        .next()
        .and_then(Result::ok)
        .filter(|c| !c.is_control() || matches!(c, '\r' | '\t'))
}

unsafe extern "C" fn callback(
    _proxy: *mut std::ffi::c_void,
    kind: u32,
    event: CGEventRef,
    _info: *mut std::ffi::c_void,
) -> CGEventRef {
    match kind {
        kCGEventTapDisabledByTimeout | kCGEventTapDisabledByUserInput => {
            let tap = TAP.load(Ordering::SeqCst);
            if !tap.is_null() {
                CGEventTapEnable(tap, true);
            }
        }
        kCGEventLeftMouseDown | kCGEventRightMouseDown | kCGEventOtherMouseDown => forget_typed(),
        kCGEventKeyDown | kCGEventKeyUp => {
            // Своё — пропускаем без разбора.
            if CGEventGetIntegerValueField(event, kCGEventSourceUserData) == input::TAG {
                return event;
            }
            let keycode = CGEventGetIntegerValueField(event, kCGKeyboardEventKeycode) as u16;
            // Клавиша не из таблицы — для сочетаний не годится, но символ
            // для сокращений может дать: код 0xFF, «прочая клавиша».
            let vk = keys::to_vk(keycode).unwrap_or(0xFF);
            let swallow = if kind == kCGEventKeyUp {
                on_key_up(vk)
            } else {
                let mods = mods_of(CGEventGetFlags(event));
                // Окно для сокращений — процесс, которому идёт нажатие.
                let pid = CGEventGetIntegerValueField(event, kCGEventTargetUnixProcessID) as usize;
                on_key_down(vk, mods, pid, || char_of(event))
            };
            if swallow {
                // Так на macOS «глотают» событие: оно становится пустым.
                CGEventSetType(event, kCGEventNull);
            }
        }
        _ => {}
    }
    event
}
