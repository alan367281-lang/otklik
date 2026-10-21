//! Файл настроек: модель, чтение с понятными ошибками, стартовый файл.

pub mod model;

pub use model::*;

use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

/// Шапка файла: подсказки по формату. Её же ставим при записи из окна —
/// serde комментарии не сохраняет, а подсказки должны оставаться в файле.
pub const HEADER: &str = r#"# Отклик — настройки.
# Файл перечитывается сам сразу после сохранения. Если в нём ошибка,
# программа продолжит работать на прошлой версии и покажет, где ошибка.
# Файл можно править и руками, и в окне программы (при сохранении из окна
# ваши собственные комментарии не сохраняются — только эта шапка).
#
# Сочетания: Ctrl, Alt, Shift, Win + клавиша
#   (A–Z, 0–9, F1–F24, Space, Enter, Tab, Pause, Insert, стрелки, Num0–Num9…)
# method — как вставлять текст:
#   auto  — короткий печатью, длинный или многострочный через буфер (по умолчанию)
#   paste — всегда через буфер обмена
#   type  — всегда печатью (там, где вставка из буфера запрещена)
# only_in / except_in — где работает правило: имя из раздела apps
#   или сразу имя программы, например Telegram.exe

"#;

/// Стартовый файл. Пишем текстом, а не через serde, чтобы оставить
/// закомментированный пример правила.
#[cfg(not(target_os = "macos"))]
pub const DEFAULT_BODY: &str = r#"version: 1

settings:
  pause_hotkey: Ctrl+Alt+Pause
  search_hotkey: Ctrl+Alt+Space
  paste_threshold: 100
  restore_clipboard_ms: 300

apps:
  notepad: { exe: Notepad.exe }
  terminal: { exe: WindowsTerminal.exe, paste_keys: Ctrl+Shift+V }

snippets:
  - name: Тест
    hotkey: Alt+1
    text: "Это тестовый текст от Отклика."

  - name: Тест длинный (через буфер)
    hotkey: Ctrl+Alt+2
    text: |
      Первая строка длинного тестового текста.
      Вторая строка — проверяем многострочную вставку через буфер обмена,
      кириллицу и то, что старый буфер вернулся на место.

  # Пример правила только для одного приложения: в Блокноте Alt+1
  # вставит другой текст, а везде остальное — «Тест» выше.
  # - name: Тест для Блокнота
  #   hotkey: Alt+1
  #   only_in: notepad
  #   text: "Это Блокнот!"
"#;

/// Стартовый файл для Mac. Сочетания с Ctrl+Option: Option+цифра на Mac
/// печатает спецсимволы («¡», «™»), а клавиши Pause нет.
#[cfg(target_os = "macos")]
pub const DEFAULT_BODY: &str = r#"version: 1

settings:
  pause_hotkey: Ctrl+Option+P
  search_hotkey: Ctrl+Option+Space
  paste_threshold: 100
  restore_clipboard_ms: 300

apps:
  notes: { exe: Notes.app }

snippets:
  - name: Тест
    hotkey: Ctrl+Option+1
    text: "Это тестовый текст от Отклика."

  - name: Тест длинный (через буфер)
    hotkey: Ctrl+Option+2
    text: |
      Первая строка длинного тестового текста.
      Вторая строка — проверяем многострочную вставку через буфер обмена,
      кириллицу и то, что старый буфер вернулся на место.

  # Пример правила только для одного приложения: в Заметках Ctrl+Option+1
  # вставит другой текст, а везде остальное — «Тест» выше.
  # - name: Тест для Заметок
  #   hotkey: Ctrl+Option+1
  #   only_in: notes
  #   text: "Это Заметки!"
"#;

/// Прочитать и разобрать файл. Ошибка — готовый текст для человека
/// (serde_norway сам добавляет номер строки и столбца).
pub fn load(path: &Path) -> Result<Config, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("не читается файл: {e}"))?;
    parse(&text)
}

pub fn default_config() -> String {
    format!("{HEADER}{DEFAULT_BODY}")
}

pub fn parse(text: &str) -> Result<Config, String> {
    if text.trim().is_empty() {
        return Ok(Config::default());
    }
    serde_norway::from_str(text).map_err(|e| format!("ошибка в файле: {e}"))
}

/// Создать стартовый файл, если его ещё нет. true — файл только что создан
/// (первый запуск: стоит показать окно).
pub fn ensure_exists(path: &Path) -> Result<bool, String> {
    if path.exists() {
        return Ok(false);
    }
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("не создать папку настроек: {e}"))?;
    }
    fs::write(path, default_config()).map_err(|e| format!("не записать файл настроек: {e}"))?;
    Ok(true)
}

/// Время изменения файла в миллисекундах — «версия» файла. По ней окно
/// понимает, не поменяли ли файл руками, пока оно было открыто.
pub fn stamp(path: &Path) -> Option<u64> {
    let m = fs::metadata(path).and_then(|m| m.modified()).ok()?;
    Some(m.duration_since(UNIX_EPOCH).ok()?.as_millis() as u64)
}

pub fn to_yaml(cfg: &Config) -> Result<String, String> {
    let mut cfg = cfg.clone();
    cfg.normalize();
    let body =
        serde_norway::to_string(&cfg).map_err(|e| format!("не удалось записать настройки: {e}"))?;
    Ok(format!("{HEADER}{}", airy(&body)))
}

/// Пустые строки между разделами и между заготовками — как у человека.
/// Строки текста заготовок всегда с отступом, поэтому их это не задевает.
fn airy(body: &str) -> String {
    let mut out = String::with_capacity(body.len() + 256);
    let mut prev = "";
    for line in body.lines() {
        let top_key = !line.starts_with([' ', '-', '#']) && !out.is_empty();
        let next_item = line.starts_with("- ") && prev.starts_with([' ', '-']);
        if top_key || next_item {
            out.push('\n');
        }
        out.push_str(line);
        out.push('\n');
        prev = line;
    }
    out
}

const BACKUPS_KEEP: usize = 20;

fn backups_dir(path: &Path) -> PathBuf {
    path.parent().unwrap_or(Path::new(".")).join("backups")
}

/// Копия текущего файла в backups/ перед записью; храним последние 20.
fn backup(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }
    let dir = backups_dir(path);
    fs::create_dir_all(&dir).map_err(|e| format!("не создать папку копий: {e}"))?;
    let stamp = chrono::Local::now()
        .format("config-%Y%m%d-%H%M%S%.3f")
        .to_string();
    // Два сохранения в одну миллисекунду (на быстром Mac — запросто) дали бы
    // одно имя, и вторая копия затёрла бы первую. Занято — добавляем номер.
    let mut target = dir.join(format!("{stamp}.yaml"));
    for n in 1.. {
        if !target.exists() {
            break;
        }
        target = dir.join(format!("{stamp}-{n}.yaml"));
    }
    fs::copy(path, target).map_err(|e| format!("не сделать резервную копию: {e}"))?;
    let mut old: Vec<PathBuf> = fs::read_dir(&dir)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with("config-"))
        })
        .collect();
    old.sort();
    let extra = old.len().saturating_sub(BACKUPS_KEEP);
    for p in &old[..extra] {
        let _ = fs::remove_file(p);
    }
    Ok(())
}

/// Записать настройки: резервная копия, запись во временный файл рядом
/// и замена — чтобы при сбое посреди записи не остался полуфайл.
pub fn save(path: &Path, cfg: &Config) -> Result<(), String> {
    let text = to_yaml(cfg)?;
    // Проверяем, что записанное прочитается обратно тем же самым.
    let back = parse(&text)?;
    let mut expect = cfg.clone();
    expect.normalize();
    if back != expect {
        return Err("внутренняя ошибка: записанный файл читается иначе".into());
    }
    backup(path)?;
    let tmp = path.with_extension("yaml.tmp");
    fs::write(&tmp, text).map_err(|e| format!("не записать файл настроек: {e}"))?;
    fs::rename(&tmp, path).map_err(|e| format!("не заменить файл настроек: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_parses() {
        let c = parse(&default_config()).unwrap();
        assert_eq!(c.snippets.len(), 2);
        // Стартовые правила рабочие на той системе, где собрано.
        let compiled = crate::rules::compile(&c);
        assert!(
            compiled.views.iter().all(|v| v.active),
            "{:?}",
            compiled.views
        );
        assert!(c.snippets[1].text.as_deref().unwrap().contains('\n'));
        #[cfg(not(target_os = "macos"))]
        assert_eq!(c.apps["terminal"].exe.to_vec(), ["WindowsTerminal.exe"]);
        #[cfg(target_os = "macos")]
        assert_eq!(c.apps["notes"].exe.to_vec(), ["Notes.app"]);
    }

    #[test]
    fn short_file_gets_defaults() {
        let c = parse("snippets:\n  - name: a\n    hotkey: Alt+2\n    text: hi\n").unwrap();
        assert_eq!(c.settings, Settings::default());
        assert!(c.snippets[0].enabled);
        assert_eq!(c.snippets[0].method, Method::Auto);
    }

    #[test]
    fn empty_file_is_empty_config() {
        assert_eq!(parse("  \n").unwrap(), Config::default());
    }

    #[test]
    fn save_roundtrip_is_short_and_stable() {
        let mut c = parse(&default_config()).unwrap();
        c.snippets[0].only_in = OneOrMany::Many(vec!["notepad".into()]);
        c.snippets[0].group = Some("  ".into());
        let y = to_yaml(&c).unwrap();
        let body = y.strip_prefix(HEADER).unwrap();
        assert!(body.contains("only_in: notepad"), "{y}");
        for absent in ["enabled", "method", "group"] {
            assert!(!body.contains(absent), "{absent}: {y}");
        }
        assert!(body.contains("\n\nsnippets:\n- name: Тест\n"), "{y}");
        c.snippets.push(Snippet {
            name: "третий".into(),
            hotkey: Some("Alt+3".into()),
            text: Some("x".into()),
            ..Default::default()
        });
        let y = to_yaml(&c).unwrap();
        assert!(y.contains("место.\n\n- name: третий"), "{y}");
        let back = parse(&y).unwrap();
        assert_eq!(back.snippets[1].text, c.snippets[1].text);
        assert_eq!(to_yaml(&back).unwrap(), y);
    }

    #[test]
    fn save_keeps_backups() {
        let dir = std::env::temp_dir().join(format!("km-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let path = dir.join("config.yaml");
        assert!(ensure_exists(&path).unwrap());
        let mut c = load(&path).unwrap();
        for i in 0..3 {
            c.snippets[0].name = format!("n{i}");
            save(&path, &c).unwrap();
        }
        assert_eq!(load(&path).unwrap().snippets[0].name, "n2");
        assert_eq!(fs::read_dir(dir.join("backups")).unwrap().count(), 3);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn error_mentions_line() {
        let e = parse("snippets:\n  - name: a\n    hotky: Alt+1\n").unwrap_err();
        assert!(e.contains("line 3"), "{e}");
    }
}
