//! Из файла настроек — в быстрые таблицы для хука. Здесь же все
//! человеческие проверки (что не так с правилом) и выбор правила
//! по активному приложению. Без WinAPI: всё проверяется тестами.

use crate::chord::Chord;
use crate::config::{AppDef, Config, Method, OneOrMany, Snippet};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

/// Что сейчас в фокусе. exe и заголовок уже в нижнем регистре.
#[derive(Debug, Clone, Default)]
pub struct ActiveApp {
    pub exe: String,
    pub title: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AppMatch {
    exes: Vec<String>,
    title_contains: Option<String>,
}

impl AppMatch {
    fn from_def(def: &AppDef) -> AppMatch {
        AppMatch {
            exes: def.exe.to_vec().iter().map(|e| e.to_lowercase()).collect(),
            title_contains: def.title_contains.as_ref().map(|t| t.to_lowercase()),
        }
    }

    fn exe(name: &str) -> AppMatch {
        AppMatch {
            exes: vec![name.to_lowercase()],
            title_contains: None,
        }
    }

    pub fn matches(&self, app: &ActiveApp) -> bool {
        let exe_ok = self.exes.is_empty() || self.exes.contains(&app.exe);
        let title_ok = self
            .title_contains
            .as_ref()
            .is_none_or(|t| app.title.contains(t.as_str()));
        exe_ok && title_ok
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Text(String),
}

#[derive(Debug)]
pub struct Rule {
    pub name: String,
    pub action: Action,
    pub method: Method,
    only_in: Vec<AppMatch>,
    except_in: Vec<AppMatch>,
}

impl Rule {
    fn allowed_in(&self, app: &ActiveApp) -> bool {
        !self.except_in.iter().any(|m| m.matches(app))
    }
}

/// Строка для окна: что есть в файле и почему правило не работает, если так.
#[derive(Debug, Clone, Serialize)]
pub struct RuleView {
    pub name: String,
    pub hotkey: Option<String>,
    pub scope: String,
    pub preview: String,
    pub active: bool,
    pub problem: Option<String>,
}

#[derive(Debug)]
pub struct Compiled {
    pub by_chord: HashMap<Chord, Vec<Arc<Rule>>>,
    pub pause: Option<Chord>,
    pub paste_threshold: usize,
    pub restore_clipboard_ms: u64,
    paste_overrides: Vec<(AppMatch, Chord)>,
    pub views: Vec<RuleView>,
    /// Общие замечания, не привязанные к одному правилу.
    pub warnings: Vec<String>,
}

pub const DEFAULT_PASTE: Chord = Chord {
    mods: crate::chord::CTRL,
    vk: 0x56,
};

impl Default for Compiled {
    fn default() -> Self {
        compile(&Config::default())
    }
}

impl Compiled {
    /// Какое правило выполнить. Правило «только для этого приложения»
    /// важнее общего. Нет подходящего — None, и сочетание уходит в
    /// приложение как обычно.
    pub fn pick(&self, chord: &Chord, app: &ActiveApp) -> Option<Arc<Rule>> {
        let rules = self.by_chord.get(chord)?;
        rules
            .iter()
            .find(|r| {
                !r.only_in.is_empty()
                    && r.only_in.iter().any(|m| m.matches(app))
                    && r.allowed_in(app)
            })
            .or_else(|| {
                rules
                    .iter()
                    .find(|r| r.only_in.is_empty() && r.allowed_in(app))
            })
            .cloned()
    }

    pub fn paste_chord(&self, app: &ActiveApp) -> Chord {
        self.paste_overrides
            .iter()
            .find(|(m, _)| m.matches(app))
            .map(|(_, c)| *c)
            .unwrap_or(DEFAULT_PASTE)
    }

    pub fn active_count(&self) -> usize {
        self.views.iter().filter(|v| v.active).count()
    }
}

fn resolve_apps(
    list: &OneOrMany,
    apps: &BTreeMap<String, AppDef>,
) -> Result<Vec<AppMatch>, String> {
    list.to_vec()
        .iter()
        .map(|name| {
            if let Some(def) = apps.get(name) {
                Ok(AppMatch::from_def(def))
            } else if name.to_lowercase().ends_with(".exe") {
                Ok(AppMatch::exe(name))
            } else {
                Err(format!(
                    "не знаю приложение «{name}»: добавьте его в apps или напишите имя exe-файла"
                ))
            }
        })
        .collect()
}

fn action_of(s: &Snippet) -> Result<Action, String> {
    let actions = [s.text.is_some(), s.keys.is_some(), s.open.is_some()]
        .iter()
        .filter(|x| **x)
        .count();
    if actions == 0 {
        return Err("нет действия: укажите text".into());
    }
    if actions > 1 {
        return Err("действие должно быть одно: text, keys или open".into());
    }
    match (&s.text, &s.keys, &s.open) {
        (Some(t), _, _) => Ok(Action::Text(t.clone())),
        (_, Some(_), _) => Err("цепочки клавиш (keys) появятся в следующей версии".into()),
        _ => Err("открытие программ (open) появится в следующей версии".into()),
    }
}

impl Rule {
    /// Разовое правило для кнопки «Проверить»: без сочетания и без
    /// ограничений по приложениям — только действие.
    pub fn adhoc(s: &Snippet) -> Result<Rule, String> {
        Ok(Rule {
            name: if s.name.trim().is_empty() {
                "Проверка".into()
            } else {
                s.name.clone()
            },
            action: action_of(s)?,
            method: s.method,
            only_in: vec![],
            except_in: vec![],
        })
    }
}

fn scope_text(only: &OneOrMany, except: &OneOrMany) -> String {
    let only = only.to_vec();
    let except = except.to_vec();
    match (only.is_empty(), except.is_empty()) {
        (true, true) => "везде".into(),
        (false, true) => format!("только: {}", only.join(", ")),
        (true, false) => format!("кроме: {}", except.join(", ")),
        (false, false) => format!("только: {}; кроме: {}", only.join(", "), except.join(", ")),
    }
}

fn preview(text: &str) -> String {
    let one_line = text.trim().replace(['\r', '\n'], " ⏎ ");
    let mut p: String = one_line.chars().take(80).collect();
    if one_line.chars().count() > 80 {
        p.push('…');
    }
    p
}

pub fn compile(cfg: &Config) -> Compiled {
    let mut warnings = Vec::new();

    let pause = match Chord::parse(&cfg.settings.pause_hotkey) {
        Ok(c) => Some(c),
        Err(e) => {
            warnings.push(format!(
                "Сочетание паузы «{}»: {e}",
                cfg.settings.pause_hotkey
            ));
            None
        }
    };

    let mut paste_overrides = Vec::new();
    for (name, def) in &cfg.apps {
        if let Some(keys) = &def.paste_keys {
            match Chord::parse_any(keys) {
                Ok(c) => paste_overrides.push((AppMatch::from_def(def), c)),
                Err(e) => warnings.push(format!("apps.{name}.paste_keys «{keys}»: {e}")),
            }
        }
    }

    let mut by_chord: HashMap<Chord, Vec<Arc<Rule>>> = HashMap::new();
    let mut views = Vec::new();

    for (i, s) in cfg.snippets.iter().enumerate() {
        let name = if s.name.trim().is_empty() {
            format!("Правило №{}", i + 1)
        } else {
            s.name.clone()
        };
        let mut view = RuleView {
            name: name.clone(),
            hotkey: s.hotkey.clone(),
            scope: scope_text(&s.only_in, &s.except_in),
            preview: s
                .text
                .as_deref()
                .or(s.keys.as_deref())
                .or(s.open.as_deref())
                .map(preview)
                .unwrap_or_default(),
            active: false,
            problem: None,
        };

        let built = (|| -> Result<Option<(Chord, Rule)>, String> {
            if !s.enabled {
                return Err("выключено".into());
            }
            let action = action_of(s)?;
            let only_in = resolve_apps(&s.only_in, &cfg.apps)?;
            let except_in = resolve_apps(&s.except_in, &cfg.apps)?;
            let Some(hk) = &s.hotkey else {
                if s.abbr.is_some() {
                    return Err(
                        "сокращения появятся в следующей версии — пока нужно сочетание (hotkey)"
                            .into(),
                    );
                }
                return Err("не задано сочетание (hotkey)".into());
            };
            let chord = Chord::parse(hk).map_err(|e| format!("сочетание «{hk}»: {e}"))?;
            if Some(chord) == pause {
                return Err(format!("{chord} уже занято паузой"));
            }
            Ok(Some((
                chord,
                Rule {
                    name: name.clone(),
                    action,
                    method: s.method,
                    only_in,
                    except_in,
                },
            )))
        })();

        match built {
            Ok(Some((chord, rule))) => {
                view.hotkey = Some(chord.to_string());
                let list = by_chord.entry(chord).or_default();
                // Два общих правила на одном сочетании — сработает первое.
                // Правила «только в …» друг другу не мешают, если приложения разные;
                // пересечение по одному приложению тоже предупреждаем.
                if let Some(other) = list.iter().find(|o| {
                    (o.only_in.is_empty() && rule.only_in.is_empty())
                        || o.only_in.iter().any(|m| rule.only_in.contains(m))
                }) {
                    view.problem = Some(format!(
                        "{chord} уже занято правилом «{}» — сработает то, что выше в файле",
                        other.name
                    ));
                } else {
                    view.active = true;
                }
                list.push(Arc::new(rule));
            }
            Ok(None) => {}
            Err(e) => view.problem = Some(e),
        }
        views.push(view);
    }

    Compiled {
        by_chord,
        pause,
        paste_threshold: cfg.settings.paste_threshold,
        restore_clipboard_ms: cfg.settings.restore_clipboard_ms,
        paste_overrides,
        views,
        warnings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::parse;

    fn app(exe: &str) -> ActiveApp {
        ActiveApp {
            exe: exe.to_lowercase(),
            title: String::new(),
        }
    }

    fn name(c: &Compiled, hk: &str, a: &ActiveApp) -> Option<String> {
        c.pick(&Chord::parse(hk).unwrap(), a)
            .map(|r| r.name.clone())
    }

    const CFG: &str = r#"
apps:
  tg: { exe: Telegram.exe }
  term: { exe: [WindowsTerminal.exe, cmd.exe], paste_keys: Ctrl+Shift+V }
snippets:
  - name: общий
    hotkey: Alt+1
    text: a
  - name: телеграм
    hotkey: Alt+1
    only_in: tg
    text: b
  - name: не в блокноте
    hotkey: Alt+2
    except_in: notepad.exe
    text: c
"#;

    #[test]
    fn app_specific_rule_wins() {
        let c = compile(&parse(CFG).unwrap());
        assert_eq!(
            name(&c, "Alt+1", &app("Telegram.exe")).as_deref(),
            Some("телеграм")
        );
        assert_eq!(
            name(&c, "Alt+1", &app("chrome.exe")).as_deref(),
            Some("общий")
        );
        assert!(c.views.iter().all(|v| v.active), "{:?}", c.views);
    }

    #[test]
    fn excluded_app_gets_the_key_back() {
        let c = compile(&parse(CFG).unwrap());
        assert_eq!(name(&c, "Alt+2", &app("notepad.exe")), None);
        assert_eq!(
            name(&c, "Alt+2", &app("word.exe")).as_deref(),
            Some("не в блокноте")
        );
        assert_eq!(name(&c, "Alt+3", &app("word.exe")), None);
    }

    #[test]
    fn paste_keys_per_app() {
        let c = compile(&parse(CFG).unwrap());
        assert_eq!(c.paste_chord(&app("cmd.exe")).to_string(), "Ctrl+Shift+V");
        assert_eq!(c.paste_chord(&app("chrome.exe")), DEFAULT_PASTE);
    }

    #[test]
    fn problems_are_explained_not_fatal() {
        let c = compile(
            &parse(
                r#"
snippets:
  - { name: плохое сочетание, hotkey: Shift+A, text: x }
  - { name: без действия, hotkey: Alt+3 }
  - { name: неизвестное приложение, hotkey: Alt+4, only_in: slack, text: x }
  - { name: хорошее, hotkey: Alt+5, text: x }
  - { name: дубль, hotkey: Alt+5, text: y }
  - { name: пауза, hotkey: Ctrl+Alt+Pause, text: y }
  - { name: выключено, hotkey: Alt+6, text: y, enabled: false }
"#,
            )
            .unwrap(),
        );
        let active: Vec<_> = c
            .views
            .iter()
            .filter(|v| v.active)
            .map(|v| v.name.as_str())
            .collect();
        assert_eq!(active, ["хорошее"]);
        assert!(c.views.iter().all(|v| v.active || v.problem.is_some()));
        assert_eq!(name(&c, "Alt+5", &app("x.exe")).as_deref(), Some("хорошее"));
        assert!(!c.by_chord.contains_key(&Chord::parse("Alt+3").unwrap()));
    }

    #[test]
    fn title_filter() {
        let c = compile(
            &parse(
                r#"
apps:
  gmail: { exe: chrome.exe, title_contains: Gmail }
snippets:
  - { name: почта, hotkey: Alt+1, only_in: gmail, text: x }
"#,
            )
            .unwrap(),
        );
        let mut a = app("chrome.exe");
        a.title = "входящие - gmail".into();
        assert!(c.pick(&Chord::parse("Alt+1").unwrap(), &a).is_some());
        a.title = "youtube".into();
        assert!(c.pick(&Chord::parse("Alt+1").unwrap(), &a).is_none());
    }
}
