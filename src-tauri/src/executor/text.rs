//! Вставка текста: печатью или через буфер обмена.

use crate::config::Method;
use crate::rules::{ActiveApp, Compiled};
use crate::win::{clipboard, input};
use std::thread::sleep;
use std::time::Duration;
use windows_sys::Win32::Foundation::HWND;

const PRE_PASTE_MS: u64 = 100;

pub fn use_paste(text: &str, method: Method, threshold: usize) -> bool {
    match method {
        Method::Paste => true,
        Method::Type => false,
        Method::Auto => text.contains('\n') || text.chars().count() > threshold,
    }
}

pub fn insert(
    owner: HWND,
    text: &str,
    method: Method,
    compiled: &Compiled,
    target: &ActiveApp,
) -> Result<String, String> {
    let chars = text.chars().count();
    if !use_paste(text, method, compiled.paste_threshold) {
        input::type_text(text)?;
        return Ok(format!("{chars} симв., печатью"));
    }

    let saved = clipboard::save(owner)?;
    let seq = clipboard::set_text(owner, text)?;
    // На изменение буфера тут же откликаются его журнал (Win+V) и менеджеры
    // буфера — они на мгновение открывают его. Если Ctrl+V придёт в этот
    // момент, программа не сможет открыть буфер и молча ничего не вставит.
    // Espanso по той же причине ждёт 100 мс.
    sleep(Duration::from_millis(PRE_PASTE_MS));
    let pasted = input::send_chord(compiled.paste_chord(target));
    // Приложение читает буфер не мгновенно — ждём, потом возвращаем старое.
    sleep(Duration::from_millis(compiled.restore_clipboard_ms));
    // Если человек успел скопировать своё — не затираем его.
    let restored = if clipboard::sequence() == seq {
        clipboard::restore(owner, &saved).map(|_| "буфер возвращён")
    } else {
        Ok("буфер уже изменили — не трогаю")
    };
    pasted?;
    Ok(format!("{chars} симв., через буфер, {}", restored?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_picks_method() {
        assert!(!use_paste("коротко", Method::Auto, 100));
        assert!(use_paste("две\nстроки", Method::Auto, 100));
        assert!(use_paste(&"я".repeat(101), Method::Auto, 100));
        assert!(!use_paste(&"я".repeat(100), Method::Auto, 100));
        assert!(use_paste("x", Method::Paste, 100));
        assert!(!use_paste("a\nb", Method::Type, 100));
    }
}
