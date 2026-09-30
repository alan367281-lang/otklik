//! Общее состояние программы и операции над ним: перечитать настройки,
//! пауза, журнал. Хук, исполнитель, трей и окно ходят сюда.

use crate::config;
use crate::rules::{compile, Compiled};
use parking_lot::{Mutex, RwLock};
use serde::Serialize;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;

const JOURNAL_SIZE: usize = 200;

#[derive(Debug, Clone, Serialize)]
pub struct LogEntry {
    pub time: String,
    /// ok — сработало, err — ошибка, info — служебное.
    pub kind: &'static str,
    pub text: String,
}

#[derive(Debug, Clone, Default)]
pub struct LoadStatus {
    pub loaded_at: Option<String>,
    pub error: Option<String>,
}

pub struct Shared {
    /// Снимок правил. Хук берёт его через try_read и никогда не ждёт.
    pub rules: RwLock<Arc<Compiled>>,
    pub paused: AtomicBool,
    pub config_path: PathBuf,
    pub status: Mutex<LoadStatus>,
    journal: Mutex<VecDeque<LogEntry>>,
    last_notice: Mutex<Option<Instant>>,
}

impl Shared {
    pub fn new(config_path: PathBuf) -> Self {
        Shared {
            rules: RwLock::new(Arc::new(Compiled::default())),
            paused: AtomicBool::new(false),
            config_path,
            status: Mutex::new(LoadStatus::default()),
            journal: Mutex::new(VecDeque::with_capacity(JOURNAL_SIZE)),
            last_notice: Mutex::new(None),
        }
    }

    pub fn journal(&self) -> Vec<LogEntry> {
        self.journal.lock().iter().rev().cloned().collect()
    }
}

fn now() -> String {
    chrono::Local::now().format("%H:%M:%S").to_string()
}

fn shared(app: &AppHandle) -> Arc<Shared> {
    app.state::<Arc<Shared>>().inner().clone()
}

/// Запись в журнал. Набранные человеком символы сюда не попадают никогда —
/// только названия правил, имена программ и ошибки.
pub fn log(app: &AppHandle, kind: &'static str, text: impl Into<String>) {
    let entry = LogEntry {
        time: now(),
        kind,
        text: text.into(),
    };
    eprintln!("[km] {} {}", entry.kind, entry.text);
    {
        let s = shared(app);
        let mut j = s.journal.lock();
        if j.len() == JOURNAL_SIZE {
            j.pop_front();
        }
        j.push_back(entry.clone());
    }
    let _ = app.emit("log", entry);
}

/// Всплывающее уведомление Windows, но не чаще раза в 20 секунд:
/// сбойное правило, нажатое десять раз подряд, не должно заспамить экран.
pub fn notify(app: &AppHandle, body: &str) {
    let s = shared(app);
    {
        let mut last = s.last_notice.lock();
        if last.is_some_and(|t| t.elapsed() < Duration::from_secs(20)) {
            return;
        }
        *last = Some(Instant::now());
    }
    let _ = app
        .notification()
        .builder()
        .title("Key Manager")
        .body(body)
        .show();
}

pub fn emit_status(app: &AppHandle) {
    let _ = app.emit("status", ());
}

/// Перечитать файл. При ошибке остаёмся на прошлых правилах.
pub fn reload(app: &AppHandle) {
    let s = shared(app);
    match config::load(&s.config_path) {
        Ok(cfg) => {
            let compiled = compile(&cfg);
            let active = compiled.active_count();
            let problems = compiled.views.iter().filter(|v| v.problem.is_some()).count();
            *s.rules.write() = Arc::new(compiled);
            *s.status.lock() = LoadStatus {
                loaded_at: Some(now()),
                error: None,
            };
            let mut msg = format!("Настройки загружены: работает правил — {active}");
            if problems > 0 {
                msg.push_str(&format!(", с замечаниями — {problems}"));
            }
            log(app, "info", msg);
        }
        Err(e) => {
            s.status.lock().error = Some(e.clone());
            log(app, "err", format!("Настройки не применены: {e}"));
            notify(app, "В файле настроек ошибка — работаю на прошлой версии");
        }
    }
    emit_status(app);
}

pub fn set_paused(app: &AppHandle, paused: bool) {
    shared(app).paused.store(paused, Ordering::SeqCst);
    crate::tray::show_paused(app, paused);
    log(
        app,
        "info",
        if paused { "Пауза: сочетания не перехватываются" } else { "Снова работаю" },
    );
    emit_status(app);
}

pub fn toggle_pause(app: &AppHandle) {
    let now_paused = shared(app).paused.load(Ordering::SeqCst);
    set_paused(app, !now_paused);
}

/// Следить за файлом: раз в секунду сверять время изменения. Своя
/// зависимость (notify) ради этого не нужна, а редакторы, которые
/// сохраняют через «записать рядом и переименовать», так тоже ловятся.
pub fn watch(app: AppHandle) {
    std::thread::Builder::new()
        .name("km-watch".into())
        .spawn(move || {
            let path = shared(&app).config_path.clone();
            let stamp = |p: &PathBuf| -> Option<SystemTime> {
                std::fs::metadata(p).and_then(|m| m.modified()).ok()
            };
            let mut last = stamp(&path);
            loop {
                std::thread::sleep(Duration::from_secs(1));
                let cur = stamp(&path);
                if cur != last && cur.is_some() {
                    last = cur;
                    // Редактор мог ещё не дописать файл — даём ему мгновение.
                    std::thread::sleep(Duration::from_millis(150));
                    reload(&app);
                }
            }
        })
        .expect("не удалось запустить наблюдение за файлом настроек");
}
