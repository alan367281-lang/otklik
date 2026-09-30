//! Всплывающий поиск заготовок: сочетание → окошко у курсора → выбрал →
//! окошко прячется, фокус возвращается в прежнее окно, заготовка
//! выполняется там.
//!
//! Окно создаётся скрытым при запуске: так оно появляется мгновенно, без
//! загрузки страницы на каждое нажатие.

use crate::commands::JobTx;
use crate::engine::Shared;
use crate::executor::Job;
use crate::rules::ActiveApp;
use crate::win::input;
use serde::Serialize;
use std::sync::Arc;
use tauri::{
    App, AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, State, WebviewUrl,
    WebviewWindowBuilder,
};
use windows_sys::Win32::Foundation::POINT;
use windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos;

pub const LABEL: &str = "search";
const WIDTH: f64 = 620.0;
const HEIGHT: f64 = 440.0;

pub fn create(app: &App) -> tauri::Result<()> {
    WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("search.html".into()))
        .title("Key Manager — поиск")
        .inner_size(WIDTH, HEIGHT)
        .decorations(false)
        .resizable(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .visible(false)
        .build()?;
    Ok(())
}

/// Показать поиск у курсора мыши, не вылезая за край монитора.
pub fn show(app: &AppHandle, target: ActiveApp) {
    *app.state::<Arc<Shared>>().search_target.lock() = Some(target);
    let Some(w) = app.get_webview_window(LABEL) else {
        return;
    };
    let mut pt = POINT { x: 0, y: 0 };
    unsafe { GetCursorPos(&mut pt) };
    let size = w
        .outer_size()
        .unwrap_or(PhysicalSize::new(WIDTH as u32, HEIGHT as u32));
    let (mut x, mut y) = (pt.x - 60, pt.y + 20);
    if let Ok(Some(m)) = app.monitor_from_point(pt.x as f64, pt.y as f64) {
        let (mx, my) = (m.position().x, m.position().y);
        let (mw, mh) = (m.size().width as i32, m.size().height as i32);
        let (w_, h_) = (size.width as i32, size.height as i32);
        x = x.clamp(mx + 8, (mx + mw - w_ - 8).max(mx + 8));
        // Не помещается под курсором — показываем над ним.
        if y + h_ > my + mh - 8 {
            y = pt.y - h_ - 12;
        }
        y = y.clamp(my + 8, (my + mh - h_ - 8).max(my + 8));
    }
    let _ = w.set_position(PhysicalPosition::new(x, y));
    // Windows отдаёт фокус процессу, от которого пришло последнее нажатие:
    // пустое нажатие делает этим процессом нас, и окно поиска получит фокус.
    input::mask_menu();
    let _ = w.show();
    let _ = w.set_focus();
    let _ = w.emit("search:open", ());
}

#[derive(Serialize)]
pub struct SearchItem {
    index: usize,
    name: String,
    hotkey: Option<String>,
    abbr: Option<String>,
    group: Option<String>,
    preview: String,
    detail: String,
}

/// Заготовки, которые работают в окне, откуда вызвали поиск.
#[tauri::command]
pub fn search_list(state: State<'_, Arc<Shared>>) -> Vec<SearchItem> {
    let target = state.search_target.lock().clone().unwrap_or_default();
    let compiled = state.rules.read().clone();
    compiled
        .views
        .iter()
        .zip(compiled.rules.iter())
        .enumerate()
        .filter_map(|(index, (v, r))| {
            let rule = r.as_ref()?;
            rule.applies_to(&target).then(|| SearchItem {
                index,
                name: v.name.clone(),
                hotkey: v.hotkey.clone(),
                abbr: v.abbr.clone(),
                group: v.group.clone(),
                preview: v.preview.clone(),
                detail: v.detail.clone(),
            })
        })
        .collect()
}

/// Выполнить выбранное: спрятать поиск, вернуть фокус прежнему окну и
/// уже там выполнить заготовку. Всё через исполнитель — по порядку.
#[tauri::command]
pub async fn search_run(
    app: AppHandle,
    index: usize,
    name: String,
    state: State<'_, Arc<Shared>>,
    tx: State<'_, JobTx>,
) -> Result<(), String> {
    let compiled = state.rules.read().clone();
    // Сверяем название: пока окно было открыто, файл могли перечитать,
    // и под этим номером оказалась бы другая заготовка.
    let rule = compiled
        .rules
        .get(index)
        .cloned()
        .flatten()
        .filter(|r| r.name == name)
        .ok_or("заготовка изменилась — откройте поиск заново")?;
    let target = state.search_target.lock().clone().unwrap_or_default();
    hide(&app);
    let send = |job| {
        tx.0.send(job)
            .map_err(|_| "исполнитель не отвечает".to_string())
    };
    send(Job::Focus(target.hwnd))?;
    send(Job::Run {
        rule,
        compiled,
        app: target,
        erase: 0,
        suffix: None,
    })
}

/// Закрыть поиск. `restore` — вернуть фокус прежнему окну (Esc); при
/// щелчке в другое окно возвращать не нужно — человек сам туда ушёл.
#[tauri::command]
pub async fn search_hide(
    app: AppHandle,
    restore: bool,
    state: State<'_, Arc<Shared>>,
    tx: State<'_, JobTx>,
) -> Result<(), String> {
    hide(&app);
    if restore {
        if let Some(t) = state.search_target.lock().clone() {
            let _ = tx.0.send(Job::Focus(t.hwnd));
        }
    }
    Ok(())
}

fn hide(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(LABEL) {
        let _ = w.hide();
    }
}
