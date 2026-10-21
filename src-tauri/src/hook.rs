//! Общая часть перехвата клавиатуры: что делать с нажатием — сочетание
//! это, набор текста или сокращение. Сам перехват свой у каждой системы:
//! `win/hook.rs` (WH_KEYBOARD_LL) и `mac/hook.rs` (CGEventTap). Они
//! переводят нажатие в общий вид — код клавиши Windows (VK), модификаторы,
//! окно — и спрашивают здесь, глотать ли его.
//!
//! Главное правило: решение принимается мгновенно. И Windows, и macOS
//! снимают перехват, который думает слишком долго, — молча. Поэтому здесь
//! только сопоставление, а всё исполнение — в потоке исполнителя.
//!
//! Сокращения: последние набранные символы помнятся только в памяти и
//! никуда не пишутся. Память сбрасывается щелчком мыши, сменой окна,
//! стрелками, Esc и сочетаниями с Ctrl/Alt/Win(Cmd).

use crate::chord::{Chord, ALT, CTRL, VK_PAUSE, WIN};
use crate::engine::Shared;
use crate::executor::Job;
use crate::platform::{foreground, input};
use std::cell::RefCell;
use std::sync::atomic::Ordering;
use std::sync::mpsc::Sender;
use std::sync::{Arc, OnceLock};

struct Ctx {
    shared: Arc<Shared>,
    tx: Sender<Job>,
}

static CTX: OnceLock<Ctx> = OnceLock::new();

/// Сколько последних символов помнить: длиннее сокращений не бывает.
const TYPED_MAX: usize = 64;

/// Набранное в текущем окне — для сокращений.
struct Typed {
    chars: Vec<char>,
    window: usize,
}

thread_local! {
    /// Клавиши, чьё нажатие мы проглотили: их отпускание тоже глотаем,
    /// а повторы при удержании — игнорируем (иначе Alt+1, зажатый на
    /// полсекунды, вставил бы текст десять раз).
    static SWALLOWED: RefCell<[bool; 256]> = const { RefCell::new([false; 256]) };
    static TYPED: RefCell<Typed> = const { RefCell::new(Typed { chars: Vec::new(), window: 0 }) };
}

/// Запустить перехват: свой поток у каждой системы.
pub fn start(shared: Arc<Shared>, tx: Sender<Job>) {
    if CTX.set(Ctx { shared, tx }).is_err() {
        return;
    }
    crate::platform::hook::run();
}

/// Набранное больше не относится к тому, что будет набрано дальше.
pub fn forget_typed() {
    TYPED.with(|t| t.borrow_mut().chars.clear());
}

/// Отпускание клавиши: глотаем, если глотали её нажатие.
pub fn on_key_up(vk: u16) -> bool {
    SWALLOWED.with(|sw| std::mem::replace(&mut sw.borrow_mut()[(vk & 0xFF) as usize], false))
}

/// Нажатие обычной (не модификатора) клавиши. `vk` — код клавиши Windows,
/// `window` — чем-то однозначное окно/приложение в фокусе, `ch` — как
/// узнать символ (спрашивается только для обычного набора, это дороже).
/// true — проглотить нажатие.
pub fn on_key_down(vk: u16, mods: u8, window: usize, ch: impl FnOnce() -> Option<char>) -> bool {
    let idx = (vk & 0xFF) as usize;
    if SWALLOWED.with(|sw| sw.borrow()[idx]) {
        return true; // автоповтор удерживаемого сочетания
    }
    let Some(ctx) = CTX.get() else { return false };
    let fkey = (0x70..=0x87).contains(&vk) || vk == VK_PAUSE;

    let swallow = if mods & (CTRL | ALT | WIN) != 0 || fkey {
        // Сочетание — это не набор текста.
        forget_typed();
        on_chord(ctx, Chord::new(mods, vk))
    } else {
        on_typing(ctx, vk, window, ch)
    };
    if swallow {
        SWALLOWED.with(|sw| sw.borrow_mut()[idx] = true);
    }
    swallow
}

/// Отладка в режиме разработки: что хук увидел и что решил. Пишем только
/// сочетания с Ctrl, Alt или Win — обычный набор текста (в том числе
/// заглавные буквы через Shift) сюда не попадает.
fn trace(chord: &Chord, msg: impl FnOnce() -> String) {
    if cfg!(debug_assertions) && chord.mods & (CTRL | ALT | WIN) != 0 {
        eprintln!("[otk] hook {chord}: {}", msg());
    }
}

/// Сочетание клавиш: есть правило — выполняем и глотаем нажатие.
fn on_chord(ctx: &Ctx, chord: Chord) -> bool {
    // try_read: если правила прямо сейчас подменяются, лучше пропустить
    // одно нажатие, чем заставить хук ждать.
    let Some(compiled) = ctx.shared.rules.try_read().map(|g| Arc::clone(&g)) else {
        trace(&chord, || "правила подменяются, пропускаю".into());
        return false;
    };

    let job = if compiled.pause == Some(chord) {
        Job::TogglePause
    } else if compiled.search == Some(chord) {
        if ctx.shared.paused.load(Ordering::Relaxed) {
            return false;
        }
        Job::ShowSearch(foreground::active_app())
    } else {
        if ctx.shared.paused.load(Ordering::Relaxed) {
            trace(&chord, || "пауза".into());
            return false;
        }
        if !compiled.by_chord.contains_key(&chord) {
            trace(&chord, || "нет правила".into());
            return false;
        }
        let app = foreground::active_app();
        let Some(rule) = compiled.pick(&chord, &app) else {
            trace(&chord, || format!("правило не для {}", app.exe));
            return false; // сочетание не для этого приложения — пусть работает как обычно
        };
        trace(&chord, || format!("«{}» в {}", rule.name, app.exe));
        Job::Run {
            rule,
            compiled,
            app,
            erase: 0,
            suffix: None,
        }
    };

    if ctx.tx.send(job).is_err() {
        return false;
    }
    if chord.has(ALT) || chord.has(WIN) {
        input::mask_menu();
    }
    true
}

/// Обычный набор: запоминаем символ и проверяем сокращения. Совпало —
/// глотаем последнюю клавишу, исполнитель сотрёт набранную часть и вставит.
fn on_typing(ctx: &Ctx, vk: u16, window: usize, ch: impl FnOnce() -> Option<char>) -> bool {
    if ctx.shared.paused.load(Ordering::Relaxed) {
        forget_typed();
        return false;
    }
    let Some(compiled) = ctx.shared.rules.try_read().map(|g| Arc::clone(&g)) else {
        return false;
    };
    if !compiled.has_abbrs() {
        return false;
    }

    TYPED.with(|t| {
        let mut t = t.borrow_mut();
        if t.window != window {
            t.chars.clear();
            t.window = window;
        }
        match vk {
            0x08 => {
                t.chars.pop(); // Backspace
                return false;
            }
            // Стрелки, Home/End, PgUp/PgDn, Insert/Delete, Esc — курсор ушёл
            // или набор прерван.
            0x1B | 0x21..=0x28 | 0x2D | 0x2E => {
                t.chars.clear();
                return false;
            }
            _ => {}
        }
        // Нет символа (мёртвая клавиша для «ё»/умлаутов, служебная клавиша) —
        // набранное не трогаем.
        let Some(ch) = ch() else { return false };

        let mut app = None;
        let hit = compiled.match_abbr(&t.chars, ch, || {
            let a = foreground::active_app();
            app = Some(a.clone());
            a
        });
        let Some(hit) = hit else {
            t.chars.push(ch);
            if t.chars.len() > TYPED_MAX {
                t.chars.remove(0);
            }
            return false;
        };
        t.chars.clear();
        let job = Job::Run {
            rule: hit.rule,
            compiled,
            app: app.unwrap_or_default(),
            erase: hit.erase,
            suffix: hit.suffix,
        };
        ctx.tx.send(job).is_ok()
    })
}
