//! Значок в трее: основной способ управлять программой, окно открывают редко.

use crate::engine;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{App, AppHandle, Manager, Wry};

const ICON: &[u8] = include_bytes!("../icons/tray.png");
const ICON_PAUSED: &[u8] = include_bytes!("../icons/tray-paused.png");

/// Пункт «Пауза / Продолжить» — меняем его подпись при переключении.
pub struct TrayUi {
    pause: MenuItem<Wry>,
}

pub fn build(app: &App) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Открыть Key Manager", true, None::<&str>)?;
    let pause = MenuItem::with_id(app, "pause", "Пауза", true, None::<&str>)?;
    let edit = MenuItem::with_id(app, "edit", "Открыть файл настроек", true, None::<&str>)?;
    let reload = MenuItem::with_id(app, "reload", "Перечитать настройки", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Выход", true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&open, &pause, &sep, &edit, &reload, &sep2, &quit])?;

    TrayIconBuilder::with_id("main")
        .icon(Image::from_bytes(ICON)?)
        .tooltip("Key Manager")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_main(app),
            "pause" => engine::toggle_pause(app),
            "edit" => crate::commands::open_config_file(app),
            "reload" => engine::reload(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main(tray.app_handle());
            }
        })
        .build(app)?;

    app.manage(TrayUi { pause });
    Ok(())
}

pub fn show_paused(app: &AppHandle, paused: bool) {
    if let Some(ui) = app.try_state::<TrayUi>() {
        let _ = ui.pause.set_text(if paused { "Продолжить" } else { "Пауза" });
    }
    if let Some(tray) = app.tray_by_id("main") {
        let bytes = if paused { ICON_PAUSED } else { ICON };
        if let Ok(img) = Image::from_bytes(bytes) {
            let _ = tray.set_icon(Some(img));
        }
        let _ = tray.set_tooltip(Some(if paused { "Key Manager — пауза" } else { "Key Manager" }));
    }
}

pub fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

pub fn is_paused(app: &AppHandle) -> bool {
    app.state::<Arc<engine::Shared>>().paused.load(Ordering::SeqCst)
}
