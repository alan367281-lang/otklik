//! Эмуляция клавиатуры через SendInput. Каждое наше нажатие помечено TAG —
//! по этой метке хук узнаёт своё и не перехватывает его повторно.

use crate::chord::{Chord, ALT, CTRL, SHIFT, WIN};
use std::mem::size_of;
use std::thread::sleep;
use std::time::Duration;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    MapVirtualKeyW, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_EXTENDEDKEY,
    KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, MAPVK_VK_TO_VSC,
};

/// «KMKM» — метка в dwExtraInfo.
pub const TAG: usize = 0x4B4D_4B4D;

/// Клавиши, которые Windows считает «расширенными»: без этого флага
/// Insert/стрелки часть программ принимает за клавиши цифрового блока.
fn is_extended(vk: u16) -> bool {
    matches!(
        vk,
        0x21..=0x28 | 0x2D | 0x2E | 0x5B | 0x5C | 0x6F | 0xA3 | 0xA5
    )
}

fn key(vk: u16, up: bool) -> INPUT {
    // Скан-код тоже заполняем: игры, RDP и часть старых программ смотрят
    // только на него.
    let scan = unsafe { MapVirtualKeyW(vk as u32, MAPVK_VK_TO_VSC) } as u16;
    let mut flags = if up { KEYEVENTF_KEYUP } else { 0 };
    if is_extended(vk) {
        flags |= KEYEVENTF_EXTENDEDKEY;
    }
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: scan,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: TAG,
            },
        },
    }
}

fn unicode(unit: u16, up: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: 0,
                wScan: unit,
                dwFlags: KEYEVENTF_UNICODE | if up { KEYEVENTF_KEYUP } else { 0 },
                time: 0,
                dwExtraInfo: TAG,
            },
        },
    }
}

fn send(inputs: &[INPUT]) -> Result<(), String> {
    if inputs.is_empty() {
        return Ok(());
    }
    let sent = unsafe {
        SendInput(
            inputs.len() as u32,
            inputs.as_ptr(),
            size_of::<INPUT>() as i32,
        )
    };
    if sent as usize != inputs.len() {
        // Почти всегда это UIPI: активное окно запущено от администратора,
        // а мы — нет. Windows молча отбрасывает ввод.
        return Err(
            "Windows не принял нажатия — скорее всего, окно запущено от имени администратора (включить: Общие → Права администратора)"
                .into(),
        );
    }
    Ok(())
}

const MOD_KEYS: [(u8, u16); 4] = [(CTRL, 0xA2), (ALT, 0xA4), (SHIFT, 0xA0), (WIN, 0x5B)];

/// Нажать сочетание целиком: модификаторы, клавиша, отпустить в обратном порядке.
pub fn send_chord(c: Chord) -> Result<(), String> {
    let mut v = Vec::with_capacity(10);
    for (m, vk) in MOD_KEYS {
        if c.has(m) {
            v.push(key(vk, false));
        }
    }
    v.push(key(c.vk, false));
    v.push(key(c.vk, true));
    for (m, vk) in MOD_KEYS.iter().rev() {
        if c.has(*m) {
            v.push(key(*vk, true));
        }
    }
    send(&v)
}

/// Нажать сочетание несколько раз подряд ({Tab 3}, возврат курсора).
pub fn send_chord_times(c: Chord, times: u32) -> Result<(), String> {
    for _ in 0..times {
        send_chord(c)?;
    }
    Ok(())
}

/// Отпустить клавиши (для модификаторов, которые человек держит слишком долго).
pub fn release(vks: &[u16]) -> Result<(), String> {
    let v: Vec<INPUT> = vks.iter().map(|vk| key(*vk, true)).collect();
    send(&v)
}

/// Пустое нажатие неиспользуемой клавиши 0xE8. Если сочетание с Alt или Win
/// «проглочено», система видит «Alt нажали и отпустили» и открывает меню
/// окна (или «Пуск» для Win). Лишнее нажатие между ними это гасит —
/// тот же приём, что у AutoHotkey.
pub fn mask_menu() {
    let _ = send(&[key(0xE8, false), key(0xE8, true)]);
}

/// Напечатать текст посимвольно юникодом — раскладка RU/EN не важна.
/// Перенос строки и табуляцию жмём настоящими клавишами: так их
/// понимают все поля ввода.
pub fn type_text(text: &str) -> Result<(), String> {
    const CHUNK: usize = 40;
    let mut batch: Vec<INPUT> = Vec::with_capacity(CHUNK * 4);
    let mut chars = 0;
    for ch in text.chars() {
        match ch {
            '\r' => continue,
            '\n' => batch.extend([key(0x0D, false), key(0x0D, true)]),
            '\t' => batch.extend([key(0x09, false), key(0x09, true)]),
            _ => {
                let mut buf = [0u16; 2];
                for unit in ch.encode_utf16(&mut buf) {
                    batch.push(unicode(*unit, false));
                    batch.push(unicode(*unit, true));
                }
            }
        }
        chars += 1;
        // Порциями с короткой паузой: некоторые программы (браузеры,
        // Electron) теряют символы, если вывалить на них тысячу событий разом.
        if chars % CHUNK == 0 {
            send(&batch)?;
            batch.clear();
            sleep(Duration::from_millis(5));
        }
    }
    send(&batch)
}
