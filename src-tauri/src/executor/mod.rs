//! Поток исполнения: берёт задания из хука по одному и выполняет.
//! По одному — чтобы две вставки подряд не перемешали буфер обмена.

mod text;

use crate::engine;
use crate::rules::{Action, ActiveApp, Compiled, Rule};
use crate::win::{clipboard, modifiers};
use std::sync::mpsc::Receiver;
use std::sync::Arc;
use std::time::Duration;
use tauri::AppHandle;

pub enum Job {
    Run {
        rule: Arc<Rule>,
        compiled: Arc<Compiled>,
        app: ActiveApp,
    },
    TogglePause,
}

pub fn spawn(app: AppHandle, rx: Receiver<Job>) {
    std::thread::Builder::new()
        .name("km-exec".into())
        .spawn(move || {
            let owner = clipboard::owner_window();
            for job in rx {
                match job {
                    Job::TogglePause => engine::toggle_pause(&app),
                    Job::Run {
                        rule,
                        compiled,
                        app: target,
                    } => {
                        let where_ = if target.exe.is_empty() {
                            "?".into()
                        } else {
                            target.exe.clone()
                        };
                        match run(owner, &rule, &compiled, &target) {
                            Ok(how) => engine::log(
                                &app,
                                "ok",
                                format!("«{}» → {where_} ({how})", rule.name),
                            ),
                            Err(e) => {
                                engine::log(
                                    &app,
                                    "err",
                                    format!("«{}» → {where_}: {e}", rule.name),
                                );
                                engine::notify(&app, &format!("«{}» не выполнено: {e}", rule.name));
                            }
                        }
                    }
                }
            }
        })
        .expect("не удалось запустить поток исполнения");
}

fn run(
    owner: windows_sys::Win32::Foundation::HWND,
    rule: &Rule,
    compiled: &Compiled,
    target: &ActiveApp,
) -> Result<String, String> {
    modifiers::wait_released(Duration::from_millis(1000));
    match &rule.action {
        Action::Text(t) => text::insert(owner, t, rule.method, compiled, target),
    }
}
