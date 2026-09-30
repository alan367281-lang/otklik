//! Из файла настроек — в быстрые таблицы для хука. Здесь же все
//! человеческие проверки (что не так с правилом) и выбор правила
//! по активному приложению. Без WinAPI: всё проверяется тестами.

use crate::chord::Chord;
use crate::config::{AppDef, Config, Method, OneOrMany, Snippet};
use crate::macros::{self, KeyOp, TextPiece};
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
    Text(Vec<TextPiece>),
    Keys(Vec<KeyOp>),
    Open {
        target: String,
        args: Option<String>,
    },
    /// Пауза — только как шаг цепочки.
    Wait(u64),
    Steps(Vec<Action>),
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

fn one_action(
    text: &Option<String>,
    keys: &Option<String>,
    open: &Option<String>,
    args: &Option<String>,
    extra: usize,
) -> Result<Option<Action>, String> {
    let count = [text.is_some(), keys.is_some(), open.is_some()]
        .iter()
        .filter(|x| **x)
        .count()
        + extra;
    if count > 1 {
        return Err("действие должно быть одно: text, keys, open или steps".into());
    }
    if let Some(t) = text {
        return Ok(Some(Action::Text(macros::parse_text(t)?)));
    }
    if let Some(k) = keys {
        let ops = macros::parse_keys(k).map_err(|e| format!("клавиши: {e}"))?;
        if ops.is_empty() {
            return Err("цепочка клавиш пустая".into());
        }
        return Ok(Some(Action::Keys(ops)));
    }
    if let Some(o) = open {
        if o.trim().is_empty() {
            return Err("не указано, что открыть".into());
        }
        return Ok(Some(Action::Open {
            target: o.trim().to_string(),
            args: args.clone().filter(|a| !a.trim().is_empty()),
        }));
    }
    Ok(None)
}

fn action_of(s: &Snippet) -> Result<Action, String> {
    let has_steps = usize::from(s.steps.is_some());
    if let Some(a) = one_action(&s.text, &s.keys, &s.open, &s.args, has_steps)? {
        return Ok(a);
    }
    let Some(steps) = &s.steps else {
        return Err("нет действия: укажите текст, клавиши, что открыть или цепочку".into());
    };
    if steps.is_empty() {
        return Err("в цепочке нет шагов".into());
    }
    let mut out = Vec::with_capacity(steps.len());
    for (i, st) in steps.iter().enumerate() {
        let n = i + 1;
        let wait = usize::from(st.wait.is_some());
        let step = one_action(&st.text, &st.keys, &st.open, &st.args, wait)
            .map_err(|e| format!("шаг {n}: {e}"))?;
        match (step, st.wait) {
            (Some(a), _) => out.push(a),
            (None, Some(ms)) if ms <= 60_000 => out.push(Action::Wait(ms)),
            (None, Some(_)) => return Err(format!("шаг {n}: пауза больше минуты")),
            (None, None) => return Err(format!("шаг {n}: пустой")),
        }
    }
    Ok(Action::Steps(out))
}

/// Короткое описание действия для списка в окне.
fn describe(a: &Action) -> String {
    match a {
        Action::Text(_) => "текст".into(),
        Action::Keys(ops) => format!("клавиши: {}", macros::describe_keys(ops)),
        Action::Open { target, .. } => format!("открыть {target}"),
        Action::Wait(ms) => format!("пауза {ms} мс"),
        Action::Steps(v) => v.iter().map(describe).collect::<Vec<_>>().join(" → "),
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
            preview: match (&s.text, action_of(s)) {
                (Some(t), _) => preview(t),
                (None, Ok(a)) => preview(&describe(&a)),
                (None, Err(_)) => String::new(),
            },
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

    #[test]
    fn macros_compile_and_explain() {
        let c = compile(
            &parse(
                r#"
snippets:
  - { name: клавиши, hotkey: Alt+1, keys: "Иванов{Tab}{Enter}" }
  - { name: открыть, hotkey: Alt+2, open: "https://ya.ru" }
  - name: цепочка
    hotkey: Alt+3
    steps:
      - open: notepad.exe
      - wait: 500
      - text: "Сегодня {date}"
  - { name: опечатка, hotkey: Alt+4, keys: "{Tabb}" }
  - { name: два действия, hotkey: Alt+5, text: a, open: b }
  - name: плохой шаг
    hotkey: Alt+6
    steps: [ { wait: 100, text: x } ]
"#,
            )
            .unwrap(),
        );
        let v: Vec<(&str, bool)> = c
            .views
            .iter()
            .map(|v| (v.name.as_str(), v.active))
            .collect();
        assert_eq!(
            v,
            [
                ("клавиши", true),
                ("открыть", true),
                ("цепочка", true),
                ("опечатка", false),
                ("два действия", false),
                ("плохой шаг", false)
            ]
        );
        assert!(c.views[3].problem.as_deref().unwrap().contains("Tabb"));
        assert!(c.views[5].problem.as_deref().unwrap().starts_with("шаг 1"));
        assert_eq!(
            c.views[2].preview,
            "открыть notepad.exe → пауза 500 мс → текст"
        );
        let r = name(&c, "Alt+3", &app("x.exe"));
        assert_eq!(r.as_deref(), Some("цепочка"));
    }
}
