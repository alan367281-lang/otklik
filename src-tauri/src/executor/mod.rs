//! Поток исполнения: берёт задания из хука по одному и выполняет.
//! По одному — чтобы две вставки подряд не перемешали буфер обмена.

mod text;

use crate::chord::Chord;
use crate::config::Method;
use crate::engine;
use crate::macros::{self, KeyOp};
use crate::rules::{Action, ActiveApp, Compiled, Rule};
use crate::win::{clipboard, input, modifiers, shell};
use std::sync::mpsc::Receiver;
use std::sync::Arc;
use std::thread::sleep;
use std::time::Duration;
use tauri::AppHandle;
use windows_sys::Win32::Foundation::HWND;

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

/// Всё, что нужно действию во время исполнения.
struct Ctx<'a> {
    owner: HWND,
    method: Method,
    compiled: &'a Compiled,
    target: &'a ActiveApp,
}

fn run(
    owner: HWND,
    rule: &Rule,
    compiled: &Compiled,
    target: &ActiveApp,
) -> Result<String, String> {
    modifiers::wait_released(Duration::from_millis(1000));
    let ctx = Ctx {
        owner,
        method: rule.method,
        compiled,
        target,
    };
    act(&ctx, &rule.action)
}

fn act(ctx: &Ctx, action: &Action) -> Result<String, String> {
    match action {
        Action::Text(pieces) => {
            let now = chrono::Local::now().naive_local();
            let (text, back) =
                macros::expand_text(pieces, now, &mut || clipboard::get_text(ctx.owner));
            let how = text::insert(ctx.owner, &text, ctx.method, ctx.compiled, ctx.target)?;
            if back > 0 {
                // Вернуть курсор на место {cursor}.
                input::send_chord_times(Chord::new(0, 0x25), back as u32)?;
            }
            Ok(how)
        }
        Action::Keys(ops) => {
            let now = chrono::Local::now().naive_local();
            for op in ops {
                match op {
                    KeyOp::Type(s) => input::type_text(s)?,
                    KeyOp::Var(v) => input::type_text(&macros::expand_var(v, now, &mut || {
                        clipboard::get_text(ctx.owner)
                    }))?,
                    KeyOp::Press(c, n) => input::send_chord_times(*c, *n)?,
                    KeyOp::Wait(ms) => sleep(Duration::from_millis(*ms)),
                }
            }
            Ok(format!("клавиши, шагов: {}", ops.len()))
        }
        Action::Open { target, args } => {
            shell::open(target, args.as_deref())?;
            Ok(format!("открыто {target}"))
        }
        Action::Wait(ms) => {
            sleep(Duration::from_millis(*ms));
            Ok(format!("пауза {ms} мс"))
        }
        Action::Steps(steps) => {
            for (i, st) in steps.iter().enumerate() {
                act(ctx, st).map_err(|e| format!("шаг {}: {e}", i + 1))?;
            }
            Ok(format!("цепочка, шагов: {}", steps.len()))
        }
    }
}
