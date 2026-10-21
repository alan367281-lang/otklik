//! Из файла настроек — в быстрые таблицы для хука. Здесь же все
//! человеческие проверки (что не так с правилом) и выбор правила
//! по активному приложению. Без WinAPI: всё проверяется тестами.

use crate::chord::Chord;
use crate::config::{AbbrMode, AppDef, Config, Method, OneOrMany, Snippet};
use crate::macros::{self, KeyOp, TextPiece};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

/// Что сейчас в фокусе. exe и заголовок уже в нижнем регистре.
#[derive(Debug, Clone, Default)]
pub struct ActiveApp {
    pub exe: String,
    pub title: String,
    /// Окно, в котором сработало правило, — исполнитель сверяет, что оно
    /// всё ещё активно, прежде чем печатать.
    pub hwnd: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AppMatch {
    exes: Vec<String>,
    title_contains: Option<String>,
}

/// Имя программы без системного хвоста: «Telegram.exe» и «Telegram.app»
/// — одно и то же «telegram». Так одно правило работает на обеих системах.
fn norm(name: &str) -> String {
    let l = name.trim().to_lowercase();
    l.strip_suffix(".exe")
        .or_else(|| l.strip_suffix(".app"))
        .unwrap_or(&l)
        .to_string()
}

impl AppMatch {
    fn from_def(def: &AppDef) -> AppMatch {
        AppMatch {
            exes: def.exe.to_vec().iter().map(|e| norm(e)).collect(),
            title_contains: def.title_contains.as_ref().map(|t| t.to_lowercase()),
        }
    }

    fn exe(name: &str) -> AppMatch {
        AppMatch {
            exes: vec![norm(name)],
            title_contains: None,
        }
    }

    pub fn matches(&self, app: &ActiveApp) -> bool {
        let exe_ok = self.exes.is_empty() || self.exes.contains(&norm(&app.exe));
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

    /// Работает ли правило в этом приложении — для списка в поиске.
    pub fn applies_to(&self, app: &ActiveApp) -> bool {
        self.allowed_in(app)
            && (self.only_in.is_empty() || self.only_in.iter().any(|m| m.matches(app)))
    }
}

/// Строка для окна: что есть в файле и почему правило не работает, если так.
#[derive(Debug, Clone, Serialize)]
pub struct RuleView {
    pub name: String,
    pub hotkey: Option<String>,
    pub abbr: Option<String>,
    pub group: Option<String>,
    /// Текст подробнее, чем preview, — для просмотра в поиске.
    pub detail: String,
    pub scope: String,
    pub preview: String,
    pub active: bool,
    pub problem: Option<String>,
}

#[derive(Debug)]
pub struct Compiled {
    pub by_chord: HashMap<Chord, Vec<Arc<Rule>>>,
    abbrs: Vec<Abbr>,
    pub pause: Option<Chord>,
    /// Сочетание всплывающего поиска.
    pub search: Option<Chord>,
    /// Рабочее правило для каждой строки views (None — у строки проблема
    /// или она выключена): по этому номеру поиск выполняет выбранное.
    pub rules: Vec<Option<Arc<Rule>>>,
    pub paste_threshold: usize,
    pub restore_clipboard_ms: u64,
    paste_overrides: Vec<(AppMatch, Chord)>,
    pub views: Vec<RuleView>,
    /// Общие замечания, не привязанные к одному правилу.
    pub warnings: Vec<String>,
}

/// Чем вставлять из буфера: Ctrl+V, на Mac — Cmd+V.
#[cfg(not(target_os = "macos"))]
pub const DEFAULT_PASTE: Chord = Chord {
    mods: crate::chord::CTRL,
    vk: 0x56,
};
#[cfg(target_os = "macos")]
pub const DEFAULT_PASTE: Chord = Chord {
    mods: crate::chord::WIN,
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

    pub fn has_abbrs(&self) -> bool {
        !self.abbrs.is_empty()
    }

    /// Набрано ли сокращение. `typed` — что уже было набрано до этой
    /// клавиши, `ch` — символ только что нажатой. Приложение узнаём, только
    /// если по тексту что-то совпало: это самый частый путь хука.
    pub fn match_abbr(
        &self,
        typed: &[char],
        ch: char,
        app: impl FnOnce() -> ActiveApp,
    ) -> Option<AbbrHit> {
        let mut found: Vec<Found> = Vec::new();
        for a in &self.abbrs {
            let n = a.chars.len();
            if a.word {
                if !is_separator(ch) || typed.len() < n || typed[typed.len() - n..] != a.chars[..] {
                    continue;
                }
                // Целым словом: «спс» не должно срабатывать в «мспс».
                if typed.len() > n && is_word_char(typed[typed.len() - n - 1]) {
                    continue;
                }
                found.push((a, n, Some(ch)));
            } else {
                if a.chars[n - 1] != ch
                    || typed.len() < n - 1
                    || typed[typed.len() - (n - 1)..] != a.chars[..n - 1]
                {
                    continue;
                }
                found.push((a, n - 1, None));
            }
        }
        if found.is_empty() {
            return None;
        }
        let app = app();
        // Правило для этого приложения важнее общего, длинное — короткого,
        // при прочих равных — то, что выше в файле.
        let mut best: Option<((bool, usize), &Found)> = None;
        for f in &found {
            let r = &f.0.rule;
            if !r.allowed_in(&app) {
                continue;
            }
            let specific = !r.only_in.is_empty();
            if specific && !r.only_in.iter().any(|m| m.matches(&app)) {
                continue;
            }
            let score = (specific, f.0.chars.len());
            if best.as_ref().is_none_or(|(b, _)| score > *b) {
                best = Some((score, f));
            }
        }
        best.map(|(_, (a, erase, suffix))| AbbrHit {
            rule: a.rule.clone(),
            erase: *erase,
            suffix: *suffix,
        })
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
            } else if [".exe", ".app"].iter().any(|s| name.to_lowercase().ends_with(s)) {
                Ok(AppMatch::exe(name))
            } else {
                Err(format!(
                    "не знаю приложение «{name}»: добавьте его в apps или напишите имя программы (Telegram.exe, Telegram.app)"
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

/// Сокращение, готовое к сравнению с набранным.
#[derive(Debug)]
struct Abbr {
    chars: Vec<char>,
    /// true — после пробела/знака и целым словом, false — сразу.
    word: bool,
    rule: Arc<Rule>,
}

/// Совпавшее по тексту сокращение: (оно, сколько стереть, разделитель).
type Found<'a> = (&'a Abbr, usize, Option<char>);

/// Что сделать, когда сокращение набрано.
#[derive(Debug)]
pub struct AbbrHit {
    pub rule: Arc<Rule>,
    /// Сколько символов стереть Backspace'ом (уже набранную часть сокращения).
    pub erase: usize,
    /// Разделитель, который человек нажал последним: мы его проглотили
    /// и напечатаем после вставки.
    pub suffix: Option<char>,
}

fn check_abbr(a: &str, mode: AbbrMode) -> Result<(Vec<char>, bool), String> {
    let chars: Vec<char> = a.trim().chars().collect();
    if chars.len() < 2 {
        return Err("сокращение — хотя бы два символа".into());
    }
    if chars.len() > 32 {
        return Err("сокращение длиннее 32 символов".into());
    }
    if chars.iter().any(|c| c.is_whitespace()) {
        return Err("в сокращении не должно быть пробелов".into());
    }
    let word = match mode {
        AbbrMode::Word => true,
        AbbrMode::Instant => false,
        AbbrMode::Auto => is_word_char(chars[0]),
    };
    Ok((chars, word))
}

pub fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Разделитель слов: пробел, Enter, Tab, знаки препинания.
pub fn is_separator(c: char) -> bool {
    matches!(c, ' ' | '\r' | '\n' | '\t') || (!is_word_char(c) && !c.is_control())
}

fn contains(hay: &[char], needle: &[char]) -> bool {
    hay.windows(needle.len()).any(|w| w == needle)
}

/// Мешают ли два правила друг другу: оба общие или есть общее приложение.
fn overlap(a: &Rule, b: &Rule) -> bool {
    (a.only_in.is_empty() && b.only_in.is_empty())
        || a.only_in.iter().any(|m| b.only_in.contains(m))
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

    let search = match Chord::parse(&cfg.settings.search_hotkey) {
        Ok(c) if Some(c) == pause => {
            warnings.push(format!(
                "Поиск и пауза на одном сочетании {c} — поиск не будет работать"
            ));
            None
        }
        Ok(c) => Some(c),
        Err(e) => {
            warnings.push(format!(
                "Сочетание поиска «{}»: {e}",
                cfg.settings.search_hotkey
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
    let mut abbrs: Vec<Abbr> = Vec::new();
    // Номер строки в views для каждого сокращения — для замечаний о конфликтах.
    let mut abbr_views: Vec<usize> = Vec::new();
    let mut views = Vec::new();
    let mut rules: Vec<Option<Arc<Rule>>> = Vec::new();

    for (i, s) in cfg.snippets.iter().enumerate() {
        let name = if s.name.trim().is_empty() {
            format!("Правило №{}", i + 1)
        } else {
            s.name.clone()
        };
        let mut view = RuleView {
            name: name.clone(),
            hotkey: s.hotkey.clone(),
            abbr: s.abbr.clone(),
            group: s.group.clone(),
            detail: match (&s.text, action_of(s)) {
                (Some(t), _) => t.chars().take(600).collect(),
                (None, Ok(a)) => describe(&a),
                (None, Err(_)) => String::new(),
            },
            scope: scope_text(&s.only_in, &s.except_in),
            preview: match (&s.text, action_of(s)) {
                (Some(t), _) => preview(t),
                (None, Ok(a)) => preview(&describe(&a)),
                (None, Err(_)) => String::new(),
            },
            active: false,
            problem: None,
        };

        type Built = (Option<Chord>, Option<(Vec<char>, bool)>, Rule);
        let built = (|| -> Result<Built, String> {
            if !s.enabled {
                return Err("выключено".into());
            }
            let action = action_of(s)?;
            let only_in = resolve_apps(&s.only_in, &cfg.apps)?;
            let except_in = resolve_apps(&s.except_in, &cfg.apps)?;
            if s.hotkey.is_none() && s.abbr.is_none() {
                return Err("не задано ни сочетание, ни сокращение".into());
            }
            let chord = match &s.hotkey {
                Some(hk) => {
                    let c = Chord::parse(hk).map_err(|e| format!("сочетание «{hk}»: {e}"))?;
                    if Some(c) == pause {
                        return Err(format!("{c} уже занято паузой"));
                    }
                    if Some(c) == search {
                        return Err(format!("{c} уже занято поиском заготовок"));
                    }
                    Some(c)
                }
                None => None,
            };
            let abbr = match &s.abbr {
                Some(a) => Some(check_abbr(a, s.abbr_mode)?),
                None => None,
            };
            Ok((
                chord,
                abbr,
                Rule {
                    name: name.clone(),
                    action,
                    method: s.method,
                    only_in,
                    except_in,
                },
            ))
        })();

        match built {
            Ok((chord, abbr, rule)) => {
                let rule = Arc::new(rule);
                view.active = true;
                rules.push(Some(rule.clone()));
                if let Some(chord) = chord {
                    view.hotkey = Some(chord.to_string());
                    let list = by_chord.entry(chord).or_default();
                    // Два общих правила на одном сочетании — сработает первое.
                    // Правила «только в …» друг другу не мешают, если приложения разные.
                    if let Some(other) = list.iter().find(|o| overlap(o, &rule)) {
                        view.active = false;
                        view.problem = Some(format!(
                            "{chord} уже занято правилом «{}» — сработает то, что выше в файле",
                            other.name
                        ));
                    }
                    list.push(rule.clone());
                }
                if let Some((chars, word)) = abbr {
                    abbr_views.push(views.len());
                    abbrs.push(Abbr { chars, word, rule });
                }
            }
            Err(e) => {
                view.problem = Some(e);
                rules.push(None);
            }
        }
        views.push(view);
    }

    // Конфликты сокращений: одинаковые — сработает верхнее; «сразу»-сокращение
    // внутри более длинного — длинное не успеет набраться.
    for j in 0..abbrs.len() {
        for i in 0..abbrs.len() {
            if i == j || !overlap(&abbrs[i].rule, &abbrs[j].rule) {
                continue;
            }
            let (a, b) = (&abbrs[i], &abbrs[j]);
            let text = |x: &Abbr| x.chars.iter().collect::<String>();
            let problem = if a.chars == b.chars && i < j {
                Some(format!(
                    "сокращение «{}» уже есть у «{}» — сработает верхнее",
                    text(b),
                    a.rule.name
                ))
            } else if !a.word
                && b.chars.len() > a.chars.len()
                && contains(&b.chars[..b.chars.len() - 1], &a.chars)
            {
                Some(format!(
                    "не успеет сработать: пока набираете «{}», раньше сработает «{}» («{}»)",
                    text(b),
                    text(a),
                    a.rule.name
                ))
            } else {
                None
            };
            if let Some(p) = problem {
                let v = &mut views[abbr_views[j]];
                if v.problem.is_none() {
                    v.problem = Some(p);
                }
                v.active = false;
            }
        }
    }

    Compiled {
        by_chord,
        abbrs,
        search,
        rules,
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
            hwnd: 0,
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
    #[test]
    fn abbreviations_match() {
        let c = compile(
            &parse(
                r#"
apps:
  tg: { exe: Telegram.exe }
snippets:
  - { name: адрес, abbr: ";адр", text: "ул. Ленина, 1" }
  - { name: спасибо, abbr: "спс", text: "Спасибо!" }
  - { name: спасибо тг, abbr: "спс", only_in: tg, text: "спс)" }
  - { name: принудительно слово, abbr: ";п", abbr_mode: word, text: "x" }
"#,
            )
            .unwrap(),
        );
        let t = |s: &str| s.chars().collect::<Vec<char>>();
        let a = || app("chrome.exe");
        // «сразу»: последний символ ещё не напечатан — стираем на один меньше
        let h = c.match_abbr(&t("пишу ;ад"), 'р', a).unwrap();
        assert_eq!(
            (h.rule.name.as_str(), h.erase, h.suffix),
            ("адрес", 3, None)
        );
        // «целым словом» — только после разделителя
        assert!(c.match_abbr(&t("спс"), 'а', a).is_none());
        let h = c.match_abbr(&t("ок спс"), ' ', a).unwrap();
        assert_eq!(
            (h.rule.name.as_str(), h.erase, h.suffix),
            ("спасибо", 3, Some(' '))
        );
        assert!(c.match_abbr(&t("мспс"), ' ', a).is_none());
        assert_eq!(c.match_abbr(&t("спс"), '!', a).unwrap().erase, 3);
        // правило для приложения важнее общего
        let h = c
            .match_abbr(&t("спс"), ' ', || app("telegram.exe"))
            .unwrap();
        assert_eq!(h.rule.name, "спасибо тг");
        // режим можно задать явно
        assert!(c.match_abbr(&t(";"), 'п', a).is_none());
        assert!(c.match_abbr(&t(";п"), '\r', a).is_some());
    }

    #[test]
    fn abbreviation_problems() {
        let c = compile(
            &parse(
                r#"
snippets:
  - { name: коротк, abbr: ";а", text: x }
  - { name: длинн, abbr: ";аб", text: y }
  - { name: дубль, abbr: ";а", text: z }
  - { name: пробел, abbr: "а б", text: z }
  - { name: один, abbr: "я", text: z }
  - { name: без всего, text: z }
  - { name: сочетание и сокращение, hotkey: Alt+1, abbr: ";ок", text: z }
"#,
            )
            .unwrap(),
        );
        let active: Vec<&str> = c
            .views
            .iter()
            .filter(|v| v.active)
            .map(|v| v.name.as_str())
            .collect();
        assert_eq!(active, ["коротк", "сочетание и сокращение"]);
        assert!(c.views[1].problem.as_deref().unwrap().contains("не успеет"));
        assert!(c.views[2].problem.as_deref().unwrap().contains("уже есть"));
        assert!(c.by_chord.contains_key(&Chord::parse("Alt+1").unwrap()));
    }
    #[test]
    fn search_hotkey_and_rule_index() {
        let c = compile(
            &parse(
                r#"
apps:
  tg: { exe: Telegram.exe }
settings: { search_hotkey: Ctrl+Alt+Space }
snippets:
  - { name: занято поиском, hotkey: Ctrl+Alt+Space, text: x }
  - { name: только тг, abbr: ";т", only_in: tg, text: y }
  - { name: везде, abbr: ";в", text: z, group: Почта }
"#,
            )
            .unwrap(),
        );
        assert_eq!(c.search, Some(Chord::parse("Ctrl+Alt+Space").unwrap()));
        assert!(c.rules[0].is_none());
        assert!(c.views[0].problem.as_deref().unwrap().contains("поиском"));
        let tg = c.rules[1].as_ref().unwrap();
        assert!(tg.applies_to(&app("telegram.exe")));
        assert!(!tg.applies_to(&app("chrome.exe")));
        assert!(c.rules[2].as_ref().unwrap().applies_to(&app("chrome.exe")));
        assert_eq!(c.views[2].group.as_deref(), Some("Почта"));
        assert_eq!(c.views[2].detail, "z");
    }

    #[test]
    fn one_rule_for_both_systems() {
        let c = compile(
            &parse(
                r#"
snippets:
  - { name: тг, hotkey: Ctrl+Alt+1, only_in: Telegram.exe, text: x }
"#,
            )
            .unwrap(),
        );
        let hk = Chord::parse("Ctrl+Alt+1").unwrap();
        assert!(c.pick(&hk, &app("telegram.exe")).is_some());
        assert!(c.pick(&hk, &app("telegram.app")).is_some());
        assert!(c.pick(&hk, &app("chrome.app")).is_none());
    }
}
