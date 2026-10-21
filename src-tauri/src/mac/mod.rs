//! macOS: те же модули, что в `win`, на средствах Quartz и AppKit.
//! Вместо «прав администратора» здесь разрешения системы:
//! «Универсальный доступ» — без него перехват и эмуляция нажатий молчат.

pub mod apps;
pub mod clipboard;
mod ffi;
pub mod foreground;
pub mod hook;
pub mod input;
mod keys;
pub mod modifiers;
pub mod shell;

/// Дано ли разрешение «Универсальный доступ».
pub fn accessibility_trusted() -> bool {
    unsafe { ffi::AXIsProcessTrusted() }
}

/// Попросить разрешение: macOS покажет своё окно со ссылкой в Настройки.
/// Если разрешение уже есть — ничего не происходит.
pub fn request_accessibility() -> bool {
    unsafe {
        let keys = [ffi::kAXTrustedCheckOptionPrompt as ffi::CFTypeRef];
        let values = [ffi::kCFBooleanTrue];
        let dict = ffi::CFDictionaryCreate(
            std::ptr::null(),
            keys.as_ptr(),
            values.as_ptr(),
            1,
            &ffi::kCFTypeDictionaryKeyCallBacks as *const _ as *const std::ffi::c_void,
            &ffi::kCFTypeDictionaryValueCallBacks as *const _ as *const std::ffi::c_void,
        );
        let trusted = ffi::AXIsProcessTrustedWithOptions(dict);
        if !dict.is_null() {
            ffi::CFRelease(dict);
        }
        trusted
    }
}

/// Открыть нужный раздел Настроек: Конфиденциальность → Универсальный доступ.
pub fn open_accessibility_settings() {
    let _ = std::process::Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
        .spawn();
}
