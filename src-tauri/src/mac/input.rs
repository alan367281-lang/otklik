//! Эмуляция клавиатуры на macOS через CGEventPost. Каждое наше событие
//! помечено TAG в поле «данные источника» — по метке хук узнаёт своё.

use super::ffi::*;
use super::keys;
use crate::chord::{Chord, ALT, CTRL, SHIFT, WIN};
use std::ptr::null;
use std::thread::sleep;
use std::time::Duration;

/// «KMKM» — метка своих событий.
pub const TAG: i64 = 0x4B4D_4B4D;

const KC_RETURN: u16 = 0x24;
const KC_TAB: u16 = 0x30;

fn flags_of(c: &Chord) -> u64 {
    let mut f = 0;
    if c.has(CTRL) {
        f |= kCGEventFlagMaskControl;
    }
    if c.has(ALT) {
        f |= kCGEventFlagMaskAlternate;
    }
    if c.has(SHIFT) {
        f |= kCGEventFlagMaskShift;
    }
    if c.has(WIN) {
        f |= kCGEventFlagMaskCommand;
    }
    f
}

/// Одно нажатие или отпускание. Флаги ставим явно (в том числе пустые):
/// иначе к событию прилипли бы модификаторы, которые человек ещё держит.
unsafe fn post(keycode: u16, down: bool, flags: u64, text: Option<&[u16]>) -> Result<(), String> {
    let e = CGEventCreateKeyboardEvent(null(), keycode, down);
    if e.is_null() {
        return Err(
            "macOS не дала создать нажатие — проверьте разрешение «Универсальный доступ»".into(),
        );
    }
    CGEventSetFlags(e, flags);
    CGEventSetIntegerValueField(e, kCGEventSourceUserData, TAG);
    if let Some(t) = text {
        CGEventKeyboardSetUnicodeString(e, t.len(), t.as_ptr());
    }
    CGEventPost(kCGHIDEventTap, e);
    CFRelease(e as CFTypeRef);
    Ok(())
}

/// Нажать сочетание целиком. На Mac модификаторы — флаги события, отдельно
/// жать Cmd или Option не нужно.
pub fn send_chord(c: Chord) -> Result<(), String> {
    let keycode = keys::from_vk(c.vk).ok_or_else(|| format!("клавишу {c} на Mac нажать нельзя"))?;
    let flags = flags_of(&c);
    unsafe {
        post(keycode, true, flags, None)?;
        post(keycode, false, flags, None)
    }
}

pub fn send_chord_times(c: Chord, times: u32) -> Result<(), String> {
    for _ in 0..times {
        send_chord(c)?;
    }
    Ok(())
}

/// На Mac нет меню, которое открывает одиночный Alt, — гасить нечего.
pub fn mask_menu() {}

/// Напечатать текст юникодом — раскладка не важна. Перенос строки и
/// табуляцию жмём настоящими клавишами.
pub fn type_text(text: &str) -> Result<(), String> {
    let mut count = 0;
    for ch in text.chars() {
        unsafe {
            match ch {
                '\r' => continue,
                '\n' => {
                    post(KC_RETURN, true, 0, None)?;
                    post(KC_RETURN, false, 0, None)?;
                }
                '\t' => {
                    post(KC_TAB, true, 0, None)?;
                    post(KC_TAB, false, 0, None)?;
                }
                _ => {
                    let mut buf = [0u16; 2];
                    let units = ch.encode_utf16(&mut buf);
                    post(0, true, 0, Some(units))?;
                    post(0, false, 0, Some(units))?;
                }
            }
        }
        count += 1;
        // Порциями с короткой паузой: часть программ теряет символы, если
        // вывалить на них сотни событий разом.
        if count % 20 == 0 {
            sleep(Duration::from_millis(4));
        }
    }
    Ok(())
}
