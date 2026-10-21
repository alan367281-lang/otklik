//! Импорт заготовок из Espanso (`%APPDATA%\espanso\match\*.yml`, на Mac —
//! `~/Library/Application Support/espanso/match`).
//! Переносим то, что умеем сами: текст, сокращения, «целым словом»,
//! курсор `$|$`, переменные буфера и даты. Формы, регулярки, скрипты,
//! картинки — пропускаем и говорим, что именно пропущено.

use crate::config::{AbbrMode, Snippet};
use serde::Serialize;
use serde_norway::Value;
use std::path::{Path, PathBuf};

#[derive(Serialize, Default)]
pub struct Imported {
    pub snippets: Vec<Snippet>,
    /// Что не перенесли и почему — человеку.
    pub skipped: Vec<String>,
    pub files: usize,
    pub dir: String,
}

fn str_of(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::to_string)
}

/// Переменные Espanso `{{имя}}` → наши подстановки. None — такую не умеем.
fn convert_vars(text: &str, vars: Option<&Value>) -> Result<String, String> {
    let mut out = text.replace("$|$", "{cursor}");
    let Some(list) = vars.and_then(Value::as_sequence) else {
        return if out.contains("{{") {
            Err("использует переменные, которые не описаны".into())
        } else {
            Ok(out)
        };
    };
    for var in list {
        let name = str_of(var, "name").unwrap_or_default();
        let kind = str_of(var, "type").unwrap_or_default();
        let ours = match kind.as_str() {
            "clipboard" => "{clipboard}".to_string(),
            // Espanso берёт формат даты из той же библиотеки chrono, что и мы.
            "date" => {
                let fmt = var
                    .get("params")
                    .and_then(|p| str_of(p, "format"))
                    .unwrap_or_else(|| "%d.%m.%Y".into());
                format!("{{date:{fmt}}}")
            }
            other => {
                return Err(format!(
                    "переменная «{name}» типа «{other}» у нас не поддерживается"
                ))
            }
        };
        out = out.replace(&format!("{{{{{name}}}}}"), &ours);
    }
    if out.contains("{{") {
        return Err("использует переменные, которые не удалось перенести".into());
    }
    Ok(out)
}

/// Разобрать один файл Espanso.
pub fn from_yaml(text: &str, file: &str) -> (Vec<Snippet>, Vec<String>) {
    let mut snippets = Vec::new();
    let mut skipped = Vec::new();
    let doc: Value = match serde_norway::from_str(text) {
        Ok(v) => v,
        Err(e) => {
            skipped.push(format!("{file}: не читается ({e})"));
            return (snippets, skipped);
        }
    };
    let Some(matches) = doc.get("matches").and_then(Value::as_sequence) else {
        return (snippets, skipped);
    };
    for m in matches {
        let mut triggers: Vec<String> = m
            .get("triggers")
            .and_then(Value::as_sequence)
            .map(|s| {
                s.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default();
        if let Some(t) = str_of(m, "trigger") {
            triggers.insert(0, t);
        }
        let label = triggers
            .first()
            .cloned()
            .unwrap_or_else(|| "без сокращения".into());
        let why = if m.get("regex").is_some() {
            Some("срабатывает по регулярному выражению")
        } else if m.get("form").is_some() {
            Some("это форма")
        } else if m.get("image_path").is_some() {
            Some("вставляет картинку")
        } else if triggers.is_empty() {
            Some("нет сокращения")
        } else {
            None
        };
        if let Some(why) = why {
            skipped.push(format!("{file}: «{label}» — {why}"));
            continue;
        }
        // markdown/html вставляем как обычный текст — лучше так, чем потерять.
        let Some(replace) = str_of(m, "replace")
            .or_else(|| str_of(m, "markdown"))
            .or_else(|| str_of(m, "html"))
        else {
            skipped.push(format!("{file}: «{label}» — нет текста замены"));
            continue;
        };
        let text = match convert_vars(&replace, m.get("vars")) {
            Ok(t) => t,
            Err(e) => {
                skipped.push(format!("{file}: «{label}» — {e}"));
                continue;
            }
        };
        let word = m.get("word").and_then(Value::as_bool).unwrap_or(false);
        for t in triggers {
            if t.chars().any(char::is_whitespace) || t.chars().count() < 2 {
                skipped.push(format!(
                    "{file}: «{t}» — с пробелом или короче двух символов"
                ));
                continue;
            }
            snippets.push(Snippet {
                name: t.clone(),
                group: Some("Espanso".into()),
                abbr: Some(t),
                abbr_mode: if word {
                    AbbrMode::Word
                } else {
                    AbbrMode::Instant
                },
                text: Some(text.clone()),
                ..Snippet::default()
            });
        }
    }
    (snippets, skipped)
}

fn yml_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        let name = e.file_name().to_string_lossy().to_string();
        // Файлы на «_» Espanso сам не загружает — их подключают вручную.
        if name.starts_with('_') {
            continue;
        }
        if p.is_dir() {
            yml_files(&p, out);
        } else if name.ends_with(".yml") || name.ends_with(".yaml") {
            out.push(p);
        }
    }
}

#[tauri::command]
pub fn import_espanso() -> Result<Imported, String> {
    #[cfg(windows)]
    let base = std::env::var("APPDATA").map_err(|_| "не найдена папка APPDATA".to_string())?;
    #[cfg(not(windows))]
    let base = std::env::var("HOME")
        .map(|h| format!("{h}/Library/Application Support"))
        .map_err(|_| "не найдена домашняя папка".to_string())?;
    let dir = Path::new(&base).join("espanso").join("match");
    if !dir.is_dir() {
        return Err(format!("Espanso не найден: нет папки {}", dir.display()));
    }
    let mut files = Vec::new();
    yml_files(&dir, &mut files);
    let mut res = Imported {
        files: files.len(),
        dir: dir.display().to_string(),
        ..Default::default()
    };
    for f in &files {
        let name = f
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let text = std::fs::read_to_string(f).map_err(|e| format!("{name}: {e}"))?;
        let (s, k) = from_yaml(&text, &name);
        res.snippets.extend(s);
        res.skipped.extend(k);
    }
    Ok(res)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_what_we_can() {
        let (s, skipped) = from_yaml(
            r#"
matches:
  - trigger: ":sig"
    replace: "С уважением,\nИван"
  - triggers: [":hi", ":привет"]
    replace: "Привет, $|$!"
    word: true
  - trigger: ":now"
    replace: "Сегодня {{d}}, в буфере {{c}}"
    vars:
      - { name: d, type: date, params: { format: "%d.%m.%Y" } }
      - { name: c, type: clipboard }
  - trigger: ":sh"
    replace: "{{out}}"
    vars: [ { name: out, type: shell, params: { cmd: "echo 1" } } ]
  - regex: "(?P<x>\\d+)kg"
    replace: "{{x}} кг"
  - trigger: ":form"
    form: "Имя: [[name]]"
"#,
            "base.yml",
        );
        let names: Vec<&str> = s.iter().map(|x| x.name.as_str()).collect();
        assert_eq!(names, [":sig", ":hi", ":привет", ":now"]);
        assert_eq!(s[0].abbr_mode, AbbrMode::Instant);
        assert_eq!(s[1].abbr_mode, AbbrMode::Word);
        assert_eq!(s[1].text.as_deref(), Some("Привет, {cursor}!"));
        assert_eq!(
            s[3].text.as_deref(),
            Some("Сегодня {date:%d.%m.%Y}, в буфере {clipboard}")
        );
        assert_eq!(skipped.len(), 3, "{skipped:?}");
        assert!(skipped.iter().any(|x| x.contains("shell")));
        // Всё перенесённое — рабочие правила.
        let cfg = crate::config::Config {
            snippets: s,
            ..Default::default()
        };
        let c = crate::rules::compile(&cfg);
        assert!(c.views.iter().all(|v| v.active), "{:?}", c.views);
    }
}
