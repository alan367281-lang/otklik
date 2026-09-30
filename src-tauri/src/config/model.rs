//! Модель файла настроек `config.yaml` ровно так, как её пишет человек.
//! Всё необязательное имеет значения по умолчанию, чтобы короткий файл
//! из двух строк тоже был правильным.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub version: u32,
    pub settings: Settings,
    /// Короткие имена приложений для правил: `telegram: { exe: Telegram.exe }`.
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
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Default)]
#[serde(default, deny_unknown_fields)]
pub struct AppDef {
    pub exe: OneOrMany,
    /// Дополнительно: подстрока в заголовке окна (без учёта регистра).
    pub title_contains: Option<String>,
    /// Чем вставлять в этом приложении (терминалам нужен Ctrl+Shift+V).
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

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Snippet {
    pub name: String,
    pub enabled: bool,
    pub group: Option<String>,
    pub hotkey: Option<String>,
    /// Сокращение (этап 4).
    pub abbr: Option<String>,
    pub text: Option<String>,
    /// Цепочка клавиш (этап 3).
    pub keys: Option<String>,
    /// Открыть программу, папку или ссылку (этап 3).
    pub open: Option<String>,
    pub method: Method,
    /// Только в этих приложениях: имя из `apps` или сразу `Telegram.exe`.
    pub only_in: OneOrMany,
    /// Везде, кроме этих приложений.
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
            text: None,
            keys: None,
            open: None,
            method: Method::Auto,
            only_in: OneOrMany::None,
            except_in: OneOrMany::None,
        }
    }
}
