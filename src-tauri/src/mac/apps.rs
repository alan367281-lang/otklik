//! Открытые программы — для выбора «где работает правило».

#![allow(unused_unsafe)]

use super::foreground::app_name;
use objc2_app_kit::{NSApplicationActivationPolicy, NSWorkspace};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize)]
pub struct RunningApp {
    /// Имя так, как его писать в правиле: «telegram.app».
    pub exe: String,
    /// Название для человека: «Telegram».
    pub title: String,
}

pub fn running_apps() -> Vec<RunningApp> {
    let own = std::process::id() as i32;
    let mut by_name: BTreeMap<String, String> = BTreeMap::new();
    unsafe {
        for app in NSWorkspace::sharedWorkspace()
            .runningApplications()
            .to_vec()
        {
            // Только обычные приложения с окнами и значком в Доке.
            if app.activationPolicy() != NSApplicationActivationPolicy::Regular
                || app.processIdentifier() == own
            {
                continue;
            }
            let name = app_name(&app);
            if name.is_empty() {
                continue;
            }
            let title = app
                .localizedName()
                .map(|s| s.to_string())
                .unwrap_or_default();
            by_name.entry(name).or_insert(title);
        }
    }
    by_name
        .into_iter()
        .map(|(exe, title)| RunningApp { exe, title })
        .collect()
}
