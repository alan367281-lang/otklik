//! Шаблоны: подстановки в тексте и мини-язык клавиш. Чистый модуль —
//! разбор при загрузке настроек (ошибки видны сразу), разворачивание в
//! момент срабатывания (дата, буфер обмена берутся «сейчас»).
//!
//! Текст:   «Добрый день, {cursor}! Сегодня {date}.»
//! Клавиши: «Иванов{Tab}Иван{Tab 2}{wait 200}{Ctrl+Enter}»
//! `{{` и `}}` — буквальные фигурные скобки.

use crate::chord::Chord;
use chrono::format::{Item, StrftimeItems};
use chrono::{Datelike, Duration, Months, NaiveDateTime};

#[derive(Debug, Clone, PartialEq)]
pub enum Var {
    Date { offset: Offset, fmt: String },
    Time { fmt: String },
    Clipboard,
}

/// Сдвиг даты: `{date+1d}`, `{date-2w}`, `{date+1m}`, `{date+1y}`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Offset {
    pub days: i64,
    pub months: i32,
}

/// Кусок текстового шаблона.
#[derive(Debug, Clone, PartialEq)]
pub enum TextPiece {
    Lit(String),
    Var(Var),
    Cursor,
}

/// Шаг мини-языка клавиш.
#[derive(Debug, Clone, PartialEq)]
pub enum KeyOp {
    Type(String),
    Var(Var),
    Press(Chord, u32),
    Wait(u64),
}

const DATE_FMT: &str = "%d.%m.%Y";
const TIME_FMT: &str = "%H:%M";

fn parse_offset(s: &str) -> Result<Offset, String> {
    if s.is_empty() {
        return Ok(Offset::default());
    }
    let bad = || format!("не понимаю сдвиг даты «{s}» — пример: +1d, -2w, +1m");
    let (sign, rest) = match s.chars().next() {
        Some('+') => (1i64, &s[1..]),
        Some('-') => (-1i64, &s[1..]),
        _ => return Err(bad()),
    };
    let unit_at = rest.find(|c: char| !c.is_ascii_digit()).ok_or_else(bad)?;
    let n: i64 = rest[..unit_at].parse().map_err(|_| bad())?;
    let n = n * sign;
    match &rest[unit_at..] {
        "d" | "д" => Ok(Offset { days: n, months: 0 }),
        "w" | "н" => Ok(Offset {
            days: n * 7,
            months: 0,
        }),
        "m" | "м" => Ok(Offset {
            days: 0,
            months: n as i32,
        }),
        "y" | "г" => Ok(Offset {
            days: 0,
            months: (n * 12) as i32,
        }),
        _ => Err(bad()),
    }
}

fn check_fmt(fmt: &str) -> Result<(), String> {
    let prepared = russian_names(fmt, 1, 1);
    if StrftimeItems::new(&prepared).any(|i| matches!(i, Item::Error)) {
        return Err(format!("не понимаю формат «{fmt}» — пример: %d.%m.%Y"));
    }
    Ok(())
}

/// Подстановка — то, что внутри `{…}`. None — это не подстановка.
pub fn parse_var(inner: &str) -> Option<Result<Var, String>> {
    let (head, fmt) = match inner.split_once(':') {
        Some((h, f)) => (h.trim(), Some(f.to_string())),
        None => (inner.trim(), None),
    };
    let lower = head.to_lowercase();
    let with_fmt = |default: &str| -> Result<String, String> {
        let f = fmt.clone().unwrap_or_else(|| default.to_string());
        check_fmt(&f)?;
        Ok(f)
    };
    for word in ["date", "дата"] {
        if let Some(rest) = lower.strip_prefix(word) {
            return Some(parse_offset(rest).and_then(|offset| {
                Ok(Var::Date {
                    offset,
                    fmt: with_fmt(DATE_FMT)?,
                })
            }));
        }
    }
    match lower.as_str() {
        "time" | "время" => Some(with_fmt(TIME_FMT).map(|fmt| Var::Time { fmt })),
        "clipboard" | "буфер" if fmt.is_none() => Some(Ok(Var::Clipboard)),
        _ => None,
    }
}

/// Разбить строку на литералы и `{…}`. `{{`/`}}` — буквальные скобки.
/// Возвращает (кусок, это_токен).
fn scan(s: &str) -> Result<Vec<(String, bool)>, String> {
    let mut out: Vec<(String, bool)> = Vec::new();
    let mut lit = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' if chars.peek() == Some(&'{') => {
                chars.next();
                lit.push('{');
            }
            '}' if chars.peek() == Some(&'}') => {
                chars.next();
                lit.push('}');
            }
            '{' => {
                let mut inner = String::new();
                let mut closed = false;
                for c in chars.by_ref() {
                    if c == '}' {
                        closed = true;
                        break;
                    }
                    inner.push(c);
                }
                if !closed {
                    return Err(format!("не закрыта скобка: «{{{inner}»"));
                }
                if !lit.is_empty() {
                    out.push((std::mem::take(&mut lit), false));
                }
                out.push((inner, true));
            }
            _ => lit.push(c),
        }
    }
    if !lit.is_empty() {
        out.push((lit, false));
    }
    Ok(out)
}

/// Текстовый шаблон. Незнакомое `{…}` остаётся как есть: в тексте бывают
/// обычные фигурные скобки (код, JSON), и ломать их незачем.
pub fn parse_text(s: &str) -> Result<Vec<TextPiece>, String> {
    let parts = match scan(s) {
        Ok(p) => p,
        // Незакрытая скобка в тексте — тоже просто текст.
        Err(_) => return Ok(vec![TextPiece::Lit(s.to_string())]),
    };
    let mut out = Vec::new();
    let mut cursors = 0;
    for (part, is_token) in parts {
        if !is_token {
            push_lit(&mut out, &part);
            continue;
        }
        let lower = part.trim().to_lowercase();
        if lower == "cursor" || lower == "курсор" {
            cursors += 1;
            out.push(TextPiece::Cursor);
        } else if let Some(v) = parse_var(&part) {
            out.push(TextPiece::Var(v.map_err(|e| format!("{{{part}}}: {e}"))?));
        } else {
            push_lit(&mut out, &format!("{{{part}}}"));
        }
    }
    if cursors > 1 {
        return Err("{cursor} можно поставить только один раз".into());
    }
    Ok(out)
}

fn push_lit(out: &mut Vec<TextPiece>, s: &str) {
    if let Some(TextPiece::Lit(prev)) = out.last_mut() {
        prev.push_str(s);
    } else {
        out.push(TextPiece::Lit(s.to_string()));
    }
}

/// Цепочка клавиш. Здесь всё в `{…}` должно быть понятно — иначе ошибка:
/// опечатка в имени клавиши не должна молча напечататься текстом.
pub fn parse_keys(s: &str) -> Result<Vec<KeyOp>, String> {
    let mut out = Vec::new();
    for (part, is_token) in scan(s)? {
        if !is_token {
            out.push(KeyOp::Type(part));
            continue;
        }
        let t = part.trim();
        let lower = t.to_lowercase();
        if let Some(ms) = lower
            .strip_prefix("wait")
            .or_else(|| lower.strip_prefix("пауза"))
        {
            let ms: u64 = ms
                .trim()
                .parse()
                .map_err(|_| format!("{{{t}}}: пауза пишется так: {{wait 200}} (миллисекунды)"))?;
            if ms > 60_000 {
                return Err(format!("{{{t}}}: пауза больше минуты — наверное, опечатка"));
            }
            out.push(KeyOp::Wait(ms));
            continue;
        }
        if let Some(v) = parse_var(t) {
            out.push(KeyOp::Var(v.map_err(|e| format!("{{{t}}}: {e}"))?));
            continue;
        }
        // «Tab 3» — нажать трижды.
        let (spec, times) = match t.rsplit_once(' ') {
            Some((a, n)) if n.chars().all(|c| c.is_ascii_digit()) && !n.is_empty() => {
                (a.trim(), n.parse::<u32>().unwrap_or(1))
            }
            _ => (t, 1),
        };
        if times == 0 || times > 100 {
            return Err(format!("{{{t}}}: повторять можно от 1 до 100 раз"));
        }
        let chord = Chord::parse_any(spec).map_err(|e| format!("{{{t}}}: {e}"))?;
        out.push(KeyOp::Press(chord, times));
    }
    Ok(out)
}

const MONTHS_GEN: [&str; 12] = [
    "января",
    "февраля",
    "марта",
    "апреля",
    "мая",
    "июня",
    "июля",
    "августа",
    "сентября",
    "октября",
    "ноября",
    "декабря",
];
const MONTHS_NOM: [&str; 12] = [
    "январь",
    "февраль",
    "март",
    "апрель",
    "май",
    "июнь",
    "июль",
    "август",
    "сентябрь",
    "октябрь",
    "ноябрь",
    "декабрь",
];
const WEEKDAYS: [&str; 7] = [
    "понедельник",
    "вторник",
    "среда",
    "четверг",
    "пятница",
    "суббота",
    "воскресенье",
];

/// Русские названия вместо английских: %B — «сентября» (для «30 сентября»),
/// %Ми — «сентябрь», %A — «среда». Подставляем до chrono.
fn russian_names(fmt: &str, month: u32, weekday: u32) -> String {
    let m = (month.clamp(1, 12) - 1) as usize;
    fmt.replace("%Ми", MONTHS_NOM[m])
        .replace("%B", MONTHS_GEN[m])
        .replace("%A", WEEKDAYS[(weekday % 7) as usize])
}

pub fn expand_var(v: &Var, now: NaiveDateTime, clipboard: &mut dyn FnMut() -> String) -> String {
    match v {
        Var::Date { offset, fmt } => {
            let mut d = now + Duration::days(offset.days);
            if offset.months > 0 {
                d = d
                    .checked_add_months(Months::new(offset.months as u32))
                    .unwrap_or(d);
            } else if offset.months < 0 {
                d = d
                    .checked_sub_months(Months::new(offset.months.unsigned_abs()))
                    .unwrap_or(d);
            }
            let f = russian_names(fmt, d.month(), d.weekday().num_days_from_monday());
            d.format(&f).to_string()
        }
        Var::Time { fmt } => {
            let f = russian_names(fmt, now.month(), now.weekday().num_days_from_monday());
            now.format(&f).to_string()
        }
        Var::Clipboard => clipboard(),
    }
}

/// Развернуть текст. Второе значение — сколько раз нажать «←» после
/// вставки, чтобы курсор встал на место {cursor} (перенос строки — один шаг).
pub fn expand_text(
    pieces: &[TextPiece],
    now: NaiveDateTime,
    clipboard: &mut dyn FnMut() -> String,
) -> (String, usize) {
    let mut before = String::new();
    let mut after = String::new();
    let mut seen_cursor = false;
    for p in pieces {
        let target = if seen_cursor { &mut after } else { &mut before };
        match p {
            TextPiece::Lit(s) => target.push_str(s),
            TextPiece::Var(v) => target.push_str(&expand_var(v, now, clipboard)),
            TextPiece::Cursor => seen_cursor = true,
        }
    }
    let back = after.replace("\r\n", "\n").chars().count();
    before.push_str(&after);
    (before, back)
}

/// Короткое описание для списка и журнала.
pub fn describe_keys(ops: &[KeyOp]) -> String {
    ops.iter()
        .map(|op| match op {
            KeyOp::Type(s) => format!("«{s}»"),
            KeyOp::Var(_) => "подстановка".into(),
            KeyOp::Press(c, 1) => c.to_string(),
            KeyOp::Press(c, n) => format!("{c}×{n}"),
            KeyOp::Wait(ms) => format!("пауза {ms} мс"),
        })
        .collect::<Vec<_>>()
        .join(" → ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn now() -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 9, 30)
            .unwrap()
            .and_hms_opt(14, 5, 0)
            .unwrap()
    }

    fn expand(s: &str) -> (String, usize) {
        expand_text(&parse_text(s).unwrap(), now(), &mut || {
            "из буфера".to_string()
        })
    }

    #[test]
    fn text_vars() {
        assert_eq!(
            expand("Сегодня {date}, {time}").0,
            "Сегодня 30.09.2026, 14:05"
        );
        assert_eq!(expand("{дата:%d %B %Y}").0, "30 сентября 2026");
        assert_eq!(expand("{date:%A}, {date:%Ми}").0, "среда, сентябрь");
        assert_eq!(expand("[{буфер}]").0, "[из буфера]");
    }

    #[test]
    fn date_offsets() {
        assert_eq!(expand("{date+1d}").0, "01.10.2026");
        assert_eq!(expand("{date-1w}").0, "23.09.2026");
        assert_eq!(expand("{дата+1м}").0, "30.10.2026");
        assert_eq!(expand("{date+5m}").0, "28.02.2027"); // 30 февраля нет — последний день месяца
        assert_eq!(expand("{date+1y:%Y}").0, "2027");
        assert!(parse_text("{date+1x}").is_err());
        assert!(parse_text("{date:%Q}").is_err());
    }

    #[test]
    fn cursor_position() {
        assert_eq!(
            expand("Добрый день, {cursor}!"),
            ("Добрый день, !".into(), 1)
        );
        assert_eq!(expand("a{курсор}\r\nbc"), ("a\r\nbc".into(), 3));
        assert_eq!(expand("без курсора").1, 0);
        assert!(parse_text("{cursor}{cursor}").is_err());
    }

    #[test]
    fn braces_in_plain_text_survive() {
        assert_eq!(expand(r#"{"a": 1} и {что-то}"#).0, r#"{"a": 1} и {что-то}"#);
        assert_eq!(expand("{{date}}").0, "{date}");
        assert_eq!(expand("незакрытая { скобка").0, "незакрытая { скобка");
    }

    #[test]
    fn keys_language() {
        let ops = parse_keys("Иванов{Tab}{Tab 2}{wait 150}{Ctrl+Enter}{date}").unwrap();
        assert_eq!(ops.len(), 6);
        assert_eq!(ops[0], KeyOp::Type("Иванов".into()));
        assert_eq!(ops[1], KeyOp::Press(Chord::parse_any("Tab").unwrap(), 1));
        assert_eq!(ops[2], KeyOp::Press(Chord::parse_any("Tab").unwrap(), 2));
        assert_eq!(ops[3], KeyOp::Wait(150));
        assert_eq!(
            ops[4],
            KeyOp::Press(Chord::parse_any("Ctrl+Enter").unwrap(), 1)
        );
        assert!(matches!(ops[5], KeyOp::Var(Var::Date { .. })));
        assert_eq!(
            describe_keys(&ops[..4]),
            "«Иванов» → Tab → Tab×2 → пауза 150 мс"
        );
    }

    #[test]
    fn keys_errors_are_explained() {
        for bad in [
            "{Tabb}",
            "{wait}",
            "{wait 99999}",
            "{Tab 0}",
            "{Ctrl+",
            "{Hyper+A}",
        ] {
            assert!(parse_keys(bad).is_err(), "{bad} должно быть ошибкой");
        }
        assert_eq!(
            parse_keys("{{x}}").unwrap(),
            vec![KeyOp::Type("{x}".into())]
        );
    }
}
