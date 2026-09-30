//! Файл настроек: модель, чтение с понятными ошибками, стартовый файл.

pub mod model;

pub use model::*;

use std::fs;
use std::path::Path;

/// Стартовый файл. Пишем текстом, а не через serde, чтобы сохранить
/// комментарии-подсказки: человек открывает его руками уже на этапе 1.
pub const DEFAULT_CONFIG: &str = r#"# Key Manager — настройки.
# Файл перечитывается сам сразу после сохранения. Если в нём ошибка,
# программа продолжит работать на прошлой версии и покажет, где ошибка.
#
# Сочетания: Ctrl, Alt, Shift, Win + клавиша
#   (A–Z, 0–9, F1–F24, Space, Enter, Tab, Pause, Insert, стрелки, Num0–Num9…)
# method — как вставлять текст:
#   auto  — короткий печатью, длинный или многострочный через буфер (по умолчанию)
#   paste — всегда через буфер обмена
#   type  — всегда печатью (там, где вставка из буфера запрещена)
# only_in / except_in — где работает правило: имя из раздела apps
#   или сразу имя программы, например Telegram.exe

version: 1

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
    text: "Это тестовый текст от Key Manager."

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

/// Прочитать и разобрать файл. Ошибка — готовый текст для человека
/// (serde_norway сам добавляет номер строки и столбца).
pub fn load(path: &Path) -> Result<Config, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("не читается файл: {e}"))?;
    parse(&text)
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
    fs::write(path, DEFAULT_CONFIG).map_err(|e| format!("не записать файл настроек: {e}"))?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_parses() {
        let c = parse(DEFAULT_CONFIG).unwrap();
        assert_eq!(c.snippets.len(), 2);
        assert_eq!(c.snippets[0].hotkey.as_deref(), Some("Alt+1"));
        assert!(c.snippets[1].text.as_deref().unwrap().contains('\n'));
        assert_eq!(c.apps["terminal"].exe.to_vec(), ["WindowsTerminal.exe"]);
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
    fn error_mentions_line() {
        let e = parse("snippets:\n  - name: a\n    hotky: Alt+1\n").unwrap_err();
        assert!(e.contains("line 3"), "{e}");
    }
}
