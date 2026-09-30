//! Модель файла настроек `config.yaml` ровно так, как её пишет человек.
//! Всё необязательное имеет значения по умолчанию, чтобы короткий файл
//! из двух строк тоже был правильным. При записи из окна пустые и
//! стандартные значения пропускаем — файл остаётся коротким и читаемым.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub version: u32,
    pub settings: Settings,
    /// Короткие имена приложений для правил: `telegram: { exe: Telegram.exe }`.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub apps: BTreeMap<String, AppDef>,
    pub snippets: Vec<Snippet>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            version: 1,
            settings: Settings::default(),
            apps: BTreeMap::new(),
            snippets: Vec::new(),
        }
    }
}

impl Config {
    /// Привести к каноничному виду перед записью: список из одного
    /// элемента — одним значением, пустой список — никак, пустые строки — никак.
    pub fn normalize(&mut self) {
        for app in self.apps.values_mut() {
            app.exe.normalize();
            blank_to_none(&mut app.title_contains);
            blank_to_none(&mut app.paste_keys);
        }
        for s in &mut self.snippets {
            s.only_in.normalize();
            s.except_in.normalize();
            blank_to_none(&mut s.group);
            blank_to_none(&mut s.hotkey);
            blank_to_none(&mut s.abbr);
        }
    }
}

fn blank_to_none(v: &mut Option<String>) {
    if v.as_deref().is_some_and(|s| s.trim().is_empty()) {
        *v = None;
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub pause_hotkey: String,
    pub search_hotkey: String,
    /// Текст длиннее этого (или с переносами строк) вставляется через буфер.
    pub paste_threshold: usize,
    /// Сколько ждать, прежде чем вернуть в буфер то, что там было.
    pub restore_clipboard_ms: u64,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            pause_hotkey: "Ctrl+Alt+Pause".into(),
            search_hotkey: "Ctrl+Alt+Space".into(),
            paste_threshold: 100,
            restore_clipboard_ms: 300,
        }
    }
}

/// Одно значение или список: `exe: Telegram.exe` и `exe: [a.exe, b.exe]`.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Default)]
#[serde(untagged)]
pub enum OneOrMany {
    #[default]
    None,
    One(String),
    Many(Vec<String>),
}

impl OneOrMany {
    pub fn to_vec(&self) -> Vec<String> {
        match self {
            OneOrMany::None => vec![],
            OneOrMany::One(s) => vec![s.clone()],
            OneOrMany::Many(v) => v.clone(),
        }
    }

    pub fn is_none(&self) -> bool {
        self.to_vec().is_empty()
    }

    fn normalize(&mut self) {
        let mut v: Vec<String> = self
            .to_vec()
            .into_iter()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        v.dedup();
        *self = match v.len() {
            0 => OneOrMany::None,
            1 => OneOrMany::One(v.remove(0)),
            _ => OneOrMany::Many(v),
        };
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Default)]
#[serde(default, deny_unknown_fields)]
pub struct AppDef {
    #[serde(skip_serializing_if = "OneOrMany::is_none")]
    pub exe: OneOrMany,
    /// Дополнительно: подстрока в заголовке окна (без учёта регистра).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title_contains: Option<String>,
    /// Чем вставлять в этом приложении (терминалам нужен Ctrl+Shift+V).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paste_keys: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Method {
    /// Короткое — печатью, длинное или многострочное — через буфер.
    #[default]
    Auto,
    Paste,
    Type,
}

impl Method {
    fn is_auto(&self) -> bool {
        *self == Method::Auto
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum AbbrMode {
    /// Начинается со знака (`;адр`, `//п`) — сразу, иначе — после пробела.
    #[default]
    Auto,
    /// Сразу, как только набрано.
    Instant,
    /// После пробела, Enter или знака препинания — и только целым словом.
    Word,
}

impl AbbrMode {
    fn is_auto(&self) -> bool {
        *self == AbbrMode::Auto
    }
}

fn is_true(b: &bool) -> bool {
    *b
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Snippet {
    pub name: String,
    #[serde(skip_serializing_if = "is_true")]
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hotkey: Option<String>,
    /// Сокращение: набрал `;адр` — заменилось на текст заготовки.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub abbr: Option<String>,
    /// Когда срабатывает сокращение.
    #[serde(skip_serializing_if = "AbbrMode::is_auto")]
    pub abbr_mode: AbbrMode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Цепочка клавиш: `Иванов{Tab}{wait 200}{Enter}`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keys: Option<String>,
    /// Открыть программу, папку, файл или ссылку.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub open: Option<String>,
    /// Аргументы для `open`, если открываем программу.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<String>,
    /// Цепочка шагов: открыть → подождать → нажать клавиши → вставить текст.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub steps: Option<Vec<Step>>,
    #[serde(skip_serializing_if = "Method::is_auto")]
    pub method: Method,
    /// Только в этих приложениях: имя из `apps` или сразу `Telegram.exe`.
    #[serde(skip_serializing_if = "OneOrMany::is_none")]
    pub only_in: OneOrMany,
    /// Везде, кроме этих приложений.
    #[serde(skip_serializing_if = "OneOrMany::is_none")]
    pub except_in: OneOrMany,
}

impl Default for Snippet {
    fn default() -> Self {
        Snippet {
            name: String::new(),
            enabled: true,
            group: None,
            hotkey: None,
            abbr: None,
            abbr_mode: AbbrMode::Auto,
            text: None,
            keys: None,
            open: None,
            args: None,
            steps: None,
            method: Method::Auto,
            only_in: OneOrMany::None,
            except_in: OneOrMany::None,
        }
    }
}

/// Один шаг цепочки. Ровно одно из: text, keys, open, wait.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Default)]
#[serde(default, deny_unknown_fields)]
pub struct Step {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keys: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub open: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<String>,
    /// Пауза в миллисекундах.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wait: Option<u64>,
}
