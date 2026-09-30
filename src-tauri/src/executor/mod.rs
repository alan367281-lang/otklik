//! Поток исполнения: берёт задания из хука по одному и выполняет.
//! По одному — чтобы две вставки подряд не перемешали буфер обмена.

mod text;

use crate::chord::Chord;
use crate::config::Method;
use crate::engine;
use crate::macros::{self, KeyOp};
use crate::rules::{Action, ActiveApp, Compiled, Rule};
use crate::win::{clipboard, foreground, input, modifiers, shell};
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
        /// Сокращение: сколько набранных символов стереть перед действием.
        erase: usize,
        /// Сокращение «целым словом»: разделитель, проглоченный хуком, —
        /// напечатать после действия.
        suffix: Option<char>,
    },
    TogglePause,
    /// Показать поиск; окно, откуда его вызвали.
    ShowSearch(ActiveApp),
    /// Вернуть фокус окну (после поиска).
    Focus(usize),
}

pub fn spawn(app: AppHandle, rx: Receiver<Job>) {
    std::thread::Builder::new()
        .name("km-exec".into())
        .spawn(move || {
            let owner = clipboard::owner_window();
            for job in rx {
                match job {
                    Job::TogglePause => engine::toggle_pause(&app),
                    Job::ShowSearch(target) => crate::search::show(&app, target),
                    Job::Focus(hwnd) => {
                        foreground::activate(hwnd);
                    }
                    Job::Run {
                        rule,
                        compiled,
                        app: target,
                        erase,
                        suffix,
                    } => {
                        let where_ = if target.exe.is_empty() {
                            "?".into()
                        } else {
                            target.exe.clone()
                        };
                        match run(owner, &rule, &compiled, &target, erase, suffix) {
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
    erase: usize,
    suffix: Option<char>,
) -> Result<String, String> {
    modifiers::wait_released(Duration::from_millis(1000));
    // Пока ждали, человек мог переключиться в другое окно — тогда не
    // печатаем: текст (а в чате — и Enter) ушёл бы не туда.
    if target.hwnd != 0 && foreground::current_window() != target.hwnd {
        return Err("окно сменилось до вставки — отменено, чтобы не напечатать не туда".into());
    }
    if erase > 0 {
        // Стереть набранное сокращение (последний его символ хук проглотил).
        input::send_chord_times(Chord::new(0, 0x08), erase as u32)?;
    }
    let ctx = Ctx {
        owner,
        method: rule.method,
        compiled,
        target,
    };
    let how = act(&ctx, &rule.action)?;
    if let Some(c) = suffix {
        // Enter приходит как \r — печатаем его настоящей клавишей (\n).
        let text = if c == '\r' {
            "\n".to_string()
        } else {
            c.to_string()
        };
        input::type_text(&text)?;
    }
    Ok(how)
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
