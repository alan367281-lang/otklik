//! Тонкие обёртки над WinAPI. Всё unsafe живёт здесь и в hook.rs,
//! остальной код работает с обычными типами Rust.

pub mod clipboard;
pub mod foreground;
pub mod input;
pub mod modifiers;

/// Строка → UTF-16 с нулём в конце, как ждут функции `…W`.
pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}
