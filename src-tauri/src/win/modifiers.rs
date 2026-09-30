//! Состояние Ctrl/Alt/Shift/Win прямо сейчас.

use crate::chord::{ALT, CTRL, SHIFT, WIN};
use std::thread::sleep;
use std::time::{Duration, Instant};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;

const MODS: [(u8, u16); 8] = [
    (SHIFT, 0xA0),
    (SHIFT, 0xA1),
    (CTRL, 0xA2),
    (CTRL, 0xA3),
    (ALT, 0xA4),
    (ALT, 0xA5),
    (WIN, 0x5B),
    (WIN, 0x5C),
];

pub fn is_modifier(vk: u16) -> bool {
    matches!(vk, 0x10..=0x12 | 0xA0..=0xA5 | 0x5B | 0x5C)
}

fn is_down(vk: u16) -> bool {
    unsafe { GetAsyncKeyState(vk as i32) as u16 & 0x8000 != 0 }
}

/// Какие модификаторы зажаты. Спрашиваем систему, а не ведём свой учёт:
/// свой учёт ломается, если отпускание потерялось (Win+L, экран UAC),
/// и тогда «залипший» Win портит все следующие сочетания.
pub fn current() -> u8 {
    MODS.iter()
        .filter(|(_, vk)| is_down(*vk))
        .fold(0, |acc, (m, _)| acc | m)
}

fn held_keys() -> Vec<u16> {
    MODS.iter().map(|(_, vk)| *vk).filter(|vk| is_down(*vk)).collect()
}

/// Дождаться, пока человек отпустит модификаторы сочетания. Иначе вместо
/// Ctrl+V приложение получит, например, Ctrl+Alt+V. Если держат дольше
/// тайм-аута — отпускаем за человека.
pub fn wait_released(timeout: Duration) {
    let start = Instant::now();
    loop {
        let held = held_keys();
        if held.is_empty() {
            return;
        }
        if start.elapsed() >= timeout {
            let _ = super::input::release(&held);
            return;
        }
        sleep(Duration::from_millis(10));
    }
}
