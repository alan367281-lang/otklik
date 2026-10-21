//! Сочетание клавиш: разбор строки «Ctrl+Alt+1», обратная запись и
//! проверка, что сочетание вообще можно занимать. Чистый модуль без WinAPI —
//! коды клавиш здесь просто числа Windows (VK_*), чтобы всё проверялось тестами.

use std::fmt;

pub const CTRL: u8 = 1;
pub const ALT: u8 = 2;
pub const SHIFT: u8 = 4;
pub const WIN: u8 = 8;

/// Модификаторы без различия левый/правый: для человека «Ctrl» один.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Chord {
    pub mods: u8,
    pub vk: u16,
}

pub const VK_PAUSE: u16 = 0x13;

/// Как называть Alt и Win там, где живёт программа. В файле настроек
/// понимаются оба варианта: «Option» и «Alt», «Cmd» и «Win» — одно и то же,
/// так один config.yaml годится и для Windows, и для Mac.
#[cfg(target_os = "macos")]
pub const ALT_NAME: &str = "Option";
#[cfg(target_os = "macos")]
pub const WIN_NAME: &str = "Cmd";
#[cfg(not(target_os = "macos"))]
pub const ALT_NAME: &str = "Alt";
#[cfg(not(target_os = "macos"))]
pub const WIN_NAME: &str = "Win";
/// Ctrl+Pause Windows присылает не как Pause, а как Break (VK_CANCEL).
/// Хук переводит его обратно, иначе «Ctrl+Alt+Pause» никогда не совпадёт.
pub const VK_CANCEL: u16 = 0x03;

/// Имена клавиш ↔ коды. Первое имя для кода — каноническое (им и пишем).
const KEYS: &[(&str, u16)] = &[
    ("Space", 0x20),
    ("Enter", 0x0D),
    ("Tab", 0x09),
    ("Esc", 0x1B),
    ("Escape", 0x1B),
    ("Backspace", 0x08),
    ("Pause", VK_PAUSE),
    ("Break", VK_PAUSE),
    ("Insert", 0x2D),
    ("Ins", 0x2D),
    ("Delete", 0x2E),
    ("Del", 0x2E),
    ("Home", 0x24),
    ("End", 0x23),
    ("PgUp", 0x21),
    ("PageUp", 0x21),
    ("PgDn", 0x22),
    ("PageDown", 0x22),
    ("Left", 0x25),
    ("Up", 0x26),
    ("Right", 0x27),
    ("Down", 0x28),
    ("PrintScreen", 0x2C),
    ("`", 0xC0),
    ("-", 0xBD),
    ("=", 0xBB),
    ("[", 0xDB),
    ("]", 0xDD),
    (";", 0xBA),
    ("'", 0xDE),
    (",", 0xBC),
    (".", 0xBE),
    ("/", 0xBF),
    ("\\", 0xDC),
    ("Num*", 0x6A),
    ("Num+", 0x6B),
    ("Num-", 0x6D),
    ("Num.", 0x6E),
    ("Num/", 0x6F),
];

pub fn key_vk(name: &str) -> Option<u16> {
    let n = name.trim();
    if n.len() == 1 {
        let c = n.chars().next()?.to_ascii_uppercase();
        if c.is_ascii_uppercase() || c.is_ascii_digit() {
            return Some(c as u16);
        }
    }
    if let Some(rest) = n.strip_prefix(['F', 'f']) {
        if let Ok(k) = rest.parse::<u16>() {
            if (1..=24).contains(&k) {
                return Some(0x70 + k - 1);
            }
        }
    }
    if let Some(rest) = n.strip_prefix("Num").or_else(|| n.strip_prefix("num")) {
        if let Ok(k) = rest.parse::<u16>() {
            if k <= 9 {
                return Some(0x60 + k);
            }
        }
    }
    KEYS.iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(n))
        .map(|(_, vk)| *vk)
}

pub fn key_name(vk: u16) -> String {
    match vk {
        0x30..=0x39 | 0x41..=0x5A => (vk as u8 as char).to_string(),
        0x70..=0x87 => format!("F{}", vk - 0x70 + 1),
        0x60..=0x69 => format!("Num{}", vk - 0x60),
        _ => KEYS
            .iter()
            .find(|(_, v)| *v == vk)
            .map(|(k, _)| k.to_string())
            .unwrap_or_else(|| format!("VK{vk:#04X}")),
    }
}

fn modifier(name: &str) -> Option<u8> {
    match name.trim().to_ascii_lowercase().as_str() {
        "ctrl" | "control" => Some(CTRL),
        "alt" | "option" | "opt" => Some(ALT),
        "shift" => Some(SHIFT),
        "win" | "meta" | "super" | "cmd" | "command" => Some(WIN),
        _ => None,
    }
}

impl Chord {
    pub fn new(mods: u8, vk: u16) -> Self {
        Chord { mods, vk }
    }

    /// «Ctrl+Alt+1» → Chord. Регистр и пробелы вокруг «+» не важны.
    /// Для сочетаний, которые мы займём, — с проверкой `check()`.
    pub fn parse(s: &str) -> Result<Chord, String> {
        let chord = Self::parse_any(s)?;
        chord.check()?;
        Ok(chord)
    }

    /// Разбор без проверки «можно ли занимать»: для сочетаний, которые мы
    /// сами нажимаем (Shift+Insert для вставки — законно, хоть и «один Shift»).
    pub fn parse_any(s: &str) -> Result<Chord, String> {
        let s = s.trim();
        if s.is_empty() {
            return Err("пустое сочетание".into());
        }
        let parts: Vec<&str> = s.split('+').map(str::trim).collect();
        let (key, mod_parts) = parts
            .split_last()
            .expect("split всегда даёт хотя бы один кусок");
        let mut mods = 0u8;
        for p in mod_parts {
            match modifier(p) {
                Some(m) => mods |= m,
                None => {
                    return Err(format!(
                        "«{p}» — не модификатор (Ctrl, {ALT_NAME}, Shift, {WIN_NAME})"
                    ))
                }
            }
        }
        if modifier(key).is_some() {
            return Err("в конце должна быть обычная клавиша, а не модификатор".into());
        }
        let vk = key_vk(key).ok_or_else(|| format!("не знаю клавишу «{key}»"))?;
        Ok(Chord { mods, vk })
    }

    /// Можно ли вообще занимать это сочетание.
    pub fn check(&self) -> Result<(), String> {
        let is_fkey = (0x70..=0x87).contains(&self.vk) || self.vk == VK_PAUSE;
        if self.mods == 0 && !is_fkey {
            return Err(format!(
                "нужен хотя бы один модификатор (Ctrl, {ALT_NAME} или {WIN_NAME})"
            ));
        }
        // Shift+буква/цифра — это просто заглавная буква или символ:
        // занять такое сочетание значит сломать обычный набор текста.
        if self.mods == SHIFT && !is_fkey {
            return Err("одного Shift мало — так печатаются заглавные буквы".into());
        }
        #[cfg(target_os = "macos")]
        const RESERVED: &[(u8, u16, &str)] = &[
            (WIN, 0x51, "Cmd+Q закрывает программы"),
            (WIN, 0x09, "Cmd+Tab переключает программы"),
            (WIN, 0x20, "Cmd+Space открывает Spotlight"),
            (CTRL | WIN, 0x51, "Ctrl+Cmd+Q блокирует экран"),
            (
                WIN | ALT,
                0x1B,
                "Cmd+Option+Esc — принудительное завершение",
            ),
        ];
        #[cfg(not(target_os = "macos"))]
        const RESERVED: &[(u8, u16, &str)] = &[
            (WIN, 0x4C, "Win+L блокирует компьютер"),
            (CTRL | ALT, 0x2E, "Ctrl+Alt+Del занят системой"),
            (ALT, 0x09, "Alt+Tab занят системой"),
            (ALT, 0x73, "Alt+F4 закрывает окна"),
            (CTRL, 0x1B, "Ctrl+Esc открывает «Пуск»"),
            (
                CTRL | SHIFT,
                0x1B,
                "Ctrl+Shift+Esc открывает диспетчер задач",
            ),
        ];
        if let Some((_, _, why)) = RESERVED
            .iter()
            .find(|(m, vk, _)| *m == self.mods && *vk == self.vk)
        {
            return Err((*why).into());
        }
        Ok(())
    }

    pub fn has(&self, m: u8) -> bool {
        self.mods & m != 0
    }
}

impl fmt::Display for Chord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (m, name) in [
            (CTRL, "Ctrl"),
            (ALT, ALT_NAME),
            (SHIFT, "Shift"),
            (WIN, WIN_NAME),
        ] {
            if self.has(m) {
                write!(f, "{name}+")?;
            }
        }
        write!(f, "{}", key_name(self.vk))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_prints_back() {
        for s in [
            format!("{ALT_NAME}+1"),
            format!("Ctrl+{ALT_NAME}+2"),
            format!("Ctrl+{ALT_NAME}+Pause"),
            "Ctrl+Shift+Space".to_string(),
            format!("{WIN_NAME}+F13"),
            "F9".to_string(),
        ] {
            assert_eq!(Chord::parse(&s).unwrap().to_string(), s);
        }
    }

    #[test]
    fn case_and_spaces_do_not_matter() {
        assert_eq!(
            Chord::parse(" ctrl + ALT + a ").unwrap(),
            Chord::new(CTRL | ALT, 0x41)
        );
    }

    #[test]
    fn modifier_order_is_normalised() {
        assert_eq!(
            Chord::parse("Alt+Ctrl+X").unwrap().to_string(),
            format!("Ctrl+{ALT_NAME}+X")
        );
    }

    #[test]
    fn mac_and_windows_names_are_the_same() {
        assert_eq!(Chord::parse("Cmd+Option+K"), Chord::parse("Win+Alt+K"));
        assert_eq!(Chord::parse("command+opt+k").unwrap().mods, WIN | ALT);
    }

    #[cfg(windows)]
    #[test]
    fn rejects_bad_chords() {
        for s in [
            "", "1", "Shift+A", "Ctrl+Alt", "Hyper+A", "Ctrl+Щ", "Win+L", "Alt+F4", "Ctrl+Esc",
        ] {
            assert!(Chord::parse(s).is_err(), "{s} должно быть отвергнуто");
        }
    }

    #[test]
    fn key_names_roundtrip() {
        for vk in [0x20u16, 0x30, 0x5A, 0x70, 0x87, 0x60, 0xC0, VK_PAUSE, 0x25] {
            assert_eq!(key_vk(&key_name(vk)), Some(vk), "vk {vk:#x}");
        }
    }
}
