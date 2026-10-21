//! Какое приложение в фокусе на macOS. «Окно» у нас — номер процесса:
//! по нему сверяем, что человек не переключился, и возвращаем фокус.
//! Имя — как у пакета приложения: «telegram.app», «google chrome.app».
//! Заголовок окна — через «Универсальный доступ» (то же разрешение, что
//! нужно перехвату).

#![allow(unused_unsafe)]

use super::ffi::*;
use crate::rules::ActiveApp;
use objc2::rc::Retained;
use objc2_app_kit::{NSApplicationActivationOptions, NSRunningApplication, NSWorkspace};
use objc2_foundation::NSString;
use std::time::Duration;

fn frontmost() -> Option<Retained<NSRunningApplication>> {
    unsafe { NSWorkspace::sharedWorkspace().frontmostApplication() }
}

/// Имя приложения так, как его пишут в правилах: «telegram.app».
pub fn app_name(app: &NSRunningApplication) -> String {
    unsafe {
        app.bundleURL()
            .and_then(|u| u.lastPathComponent())
            .or_else(|| app.localizedName())
            .map(|s| s.to_string().to_lowercase())
            .unwrap_or_default()
    }
}

/// Заголовок окна в фокусе. Без разрешения или у окна без заголовка — пусто.
fn focused_title(pid: i32) -> String {
    unsafe {
        let app = AXUIElementCreateApplication(pid);
        if app.is_null() {
            return String::new();
        }
        let mut window: CFTypeRef = std::ptr::null();
        let attr = NSString::from_str("AXFocusedWindow");
        let ok =
            AXUIElementCopyAttributeValue(app, Retained::as_ptr(&attr) as CFStringRef, &mut window)
                == 0;
        CFRelease(app);
        if !ok || window.is_null() {
            return String::new();
        }
        let mut title: CFTypeRef = std::ptr::null();
        let attr = NSString::from_str("AXTitle");
        let ok = AXUIElementCopyAttributeValue(
            window,
            Retained::as_ptr(&attr) as CFStringRef,
            &mut title,
        ) == 0;
        CFRelease(window);
        if !ok || title.is_null() {
            return String::new();
        }
        // CFString и NSString — одно и то же (toll-free bridging).
        let s = (*(title as *const NSString)).to_string();
        CFRelease(title);
        s
    }
}

pub fn active_app() -> ActiveApp {
    let Some(app) = frontmost() else {
        return ActiveApp::default();
    };
    let pid = unsafe { app.processIdentifier() };
    ActiveApp {
        exe: app_name(&app),
        title: focused_title(pid).to_lowercase(),
        hwnd: pid as usize,
    }
}

/// Активное «окно» сейчас — номер процесса приложения в фокусе.
pub fn current_window() -> usize {
    frontmost()
        .map(|a| unsafe { a.processIdentifier() } as usize)
        .unwrap_or(0)
}

/// Вернуть фокус приложению и дождаться этого (до 300 мс).
pub fn activate(pid: usize) -> bool {
    if pid == 0 {
        return false;
    }
    unsafe {
        let Some(app) = NSRunningApplication::runningApplicationWithProcessIdentifier(pid as i32)
        else {
            return false;
        };
        app.activateWithOptions(NSApplicationActivationOptions::ActivateAllWindows);
    }
    for _ in 0..30 {
        if current_window() == pid {
            return true;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    false
}
