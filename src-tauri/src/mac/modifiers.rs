//! Что зажато прямо сейчас — по состоянию клавиатуры в системе.

use super::ffi::*;
use crate::chord::{ALT, CTRL, SHIFT, WIN};
use std::thread::sleep;
use std::time::{Duration, Instant};

pub fn current() -> u8 {
    let f = unsafe { CGEventSourceFlagsState(kCGEventSourceStateHIDSystemState) };
    let mut m = 0;
    if f & kCGEventFlagMaskControl != 0 {
        m |= CTRL;
    }
    if f & kCGEventFlagMaskAlternate != 0 {
        m |= ALT;
    }
    if f & kCGEventFlagMaskShift != 0 {
        m |= SHIFT;
    }
    if f & kCGEventFlagMaskCommand != 0 {
        m |= WIN;
    }
    m
}

/// Дождаться, пока человек отпустит модификаторы сочетания. На Mac наши
/// события и так идут с явными флагами, поэтому по тайм-ауту просто
/// продолжаем — отпускать за человека не нужно.
pub fn wait_released(timeout: Duration) {
    let start = Instant::now();
    while current() != 0 && start.elapsed() < timeout {
        sleep(Duration::from_millis(10));
    }
}
