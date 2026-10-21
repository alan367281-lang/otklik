// Единственное место, где интерфейс разговаривает с ядром. Вне Tauri
// (npm run dev:web в браузере) команды возвращают заглушки, чтобы вёрстку
// можно было смотреть без сборки Rust.
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import type { Config, Snippet } from './config'

export interface AdminStatus {
  elevated: boolean
  task: boolean
  exe: string
}

export interface UpdateInfo {
  version: string
  current: string
  notes: string | null
}

export interface PlatformInfo {
  os: 'windows' | 'macos'
  trusted: boolean
}

export interface Imported {
  snippets: Snippet[]
  skipped: string[]
  files: number
  dir: string
}

export interface RuleView {
  name: string
  hotkey: string | null
  abbr: string | null
  group: string | null
  detail: string
  scope: string
  preview: string
  active: boolean
  problem: string | null
}

export interface Status {
  paused: boolean
  configPath: string
  loadedAt: string | null
  error: string | null
  warnings: string[]
  pauseHotkey: string | null
  activeCount: number
  rules: RuleView[]
}

export interface LogEntry {
  time: string
  kind: 'ok' | 'err' | 'info'
  text: string
}

export interface ConfigDoc {
  config: Config | null
  error: string | null
  stamp: number | null
}

export interface Check {
  rules: RuleView[]
  warnings: string[]
}

export interface RunningApp {
  exe: string
  title: string
}

export interface SearchItem {
  index: number
  name: string
  hotkey: string | null
  abbr: string | null
  group: string | null
  preview: string
  detail: string
}

const native = '__TAURI_INTERNALS__' in window

// Демо-данные для режима разработки в браузере (npm run dev:web) —
// по ним смотрят вёрстку и снимают экраны для README.
function demo(p: Partial<Snippet> & { name: string }): Snippet {
  return {
    enabled: true, group: null, hotkey: null, abbr: null, abbr_mode: 'auto', text: null, keys: null,
    open: null, args: null, steps: null, method: 'auto', only_in: null, except_in: null, ...p,
  }
}

const demoConfig: Config = {
  version: 1,
  settings: { pause_hotkey: 'Ctrl+Alt+Pause', search_hotkey: 'Ctrl+Alt+Space', paste_threshold: 100, restore_clipboard_ms: 300 },
  apps: {
    telegram: { exe: 'Telegram.exe', title_contains: null, paste_keys: null },
    terminal: { exe: 'WindowsTerminal.exe', title_contains: null, paste_keys: 'Ctrl+Shift+V' },
  },
  snippets: [
    demo({ name: 'Подпись', group: 'Почта', hotkey: 'Ctrl+Alt+1', abbr: ';подп', text: 'С уважением,\nИван\n{date:%d %B %Y}' }),
    demo({ name: 'Ответ на заявку', group: 'Почта', abbr: ';отв', text: 'Добрый день, {cursor}!\n\nСпасибо за заявку — посмотрю и вернусь с ответом до конца дня.' }),
    demo({ name: 'Адрес офиса', group: 'Работа', abbr: ';адр', text: 'ул. Примерная, 1, офис 5' }),
    demo({ name: 'Спасибо (Telegram)', group: 'Чаты', abbr: 'спс', only_in: 'telegram', text: 'Спасибо! 🙏' }),
    demo({ name: 'Ссылка на встречу', group: 'Работа', hotkey: 'Ctrl+Alt+M', text: 'Ссылка на встречу: {clipboard}' }),
    demo({ name: 'Заполнить форму', group: 'Работа', hotkey: 'Ctrl+Alt+F', keys: 'Иванов{Tab}Иван{Tab}{wait 200}{Enter}' }),
    demo({ name: 'Утро', hotkey: 'Ctrl+Alt+U', steps: [{ open: 'https://tracker.yandex.ru' }, { wait: 1500 }, { keys: '{Ctrl+L}' }] }),
  ],
}

// Имитация подстановок для демо-режима: в браузере ядра нет.
function demoPreview(text: string): string {
  return text
    .replace(/\{(date|дата):[^}]*%B[^}]*\}/gi, '21 октября 2026')
    .replace(/\{(date|дата)[^}]*\}/gi, '21.10.2026')
    .replace(/\{(time|время)[^}]*\}/gi, '14:05')
    .replace(/\{(clipboard|буфер)\}/gi, 'https://telemost.yandex.ru/j/123')
    .replace(/\{(cursor|курсор)\}/gi, '‸')
}

const demoStatus: Status = {
  paused: false,
  configPath: 'C:\\Users\\…\\app.otklik.desktop\\config.yaml',
  loadedAt: '12:00:00',
  error: null,
  warnings: [],
  pauseHotkey: 'Ctrl+Alt+Pause',
  activeCount: 7,
  rules: [],
}

async function call<T>(cmd: string, args?: Record<string, unknown>, fallback?: T): Promise<T> {
  if (!native) return fallback as T
  return invoke<T>(cmd, args)
}

export const api = {
  status: (): Promise<Status> => call('get_status', undefined, demoStatus),
  log: (): Promise<LogEntry[]> => call('get_log', undefined, []),
  setPaused: (paused: boolean): Promise<void> => call('set_paused', { paused }),
  reload: (): Promise<void> => call('reload_config'),
  openConfig: (): Promise<void> => call('open_config'),
  openConfigFolder: (): Promise<void> => call('open_config_folder'),

  getConfig: (): Promise<ConfigDoc> => call('get_config', undefined, { config: demoConfig, error: null, stamp: 1 }),
  saveConfig: (config: Config, stamp: number | null): Promise<number | null> =>
    call('save_config', { config, stamp }, stamp),
  checkConfig: (config: Config): Promise<Check> =>
    call('check_config', { config }, { rules: config.snippets.map(() => ({ name: '', hotkey: null, abbr: null, group: null, detail: '', scope: '', preview: '', active: true, problem: null })), warnings: [] }),
  listApps: (): Promise<RunningApp[]> =>
    call('list_apps', undefined, [{ exe: 'telegram.exe', title: 'Telegram' }, { exe: 'chrome.exe', title: 'Google Chrome' }]),
  /** Что вставится прямо сейчас (‸ — место курсора). */
  previewText: (text: string): Promise<string> => call('preview_text', { text }, demoPreview(text)),
  /** null — сочетание можно занимать, иначе — почему нельзя. */
  checkChord: (chord: string): Promise<string | null> => call('check_chord', { chord }, null),
  testSnippet: (snippet: Snippet): Promise<void> => call('test_snippet', { snippet }),
  getAutostart: (): Promise<boolean> => call('get_autostart', undefined, false),
  setAutostart: (enabled: boolean): Promise<void> => call('set_autostart', { enabled }),

  searchList: (): Promise<SearchItem[]> =>
    call(
      'search_list',
      undefined,
      demoConfig.snippets.map((sn, index) => ({
        index,
        name: sn.name,
        hotkey: sn.hotkey,
        abbr: sn.abbr,
        group: sn.group,
        preview: (sn.text ?? sn.keys ?? 'цепочка шагов').replace(/\n/g, ' ⏎ ').slice(0, 80),
        detail: sn.text ?? sn.keys ?? 'открыть → пауза → клавиши',
      })),
    ),
  searchRun: (index: number, name: string): Promise<void> => call('search_run', { index, name }),
  searchHide: (restore: boolean): Promise<void> => call('search_hide', { restore }),
  appVersion: (): Promise<string> => call('app_version', undefined, '0.2.0'),
  updateStatus: (): Promise<UpdateInfo | null> => call('update_status', undefined, null),
  updateCheck: (): Promise<UpdateInfo | null> => call('update_check', undefined, null),
  updateInstall: (): Promise<void> => call('update_install'),
  platformInfo: (): Promise<PlatformInfo> => call('platform_info', undefined, { os: 'windows', trusted: true }),
  openPermissions: (): Promise<void> => call('open_permissions'),
  adminStatus: (): Promise<AdminStatus> => call('admin_status', undefined, { elevated: false, task: false, exe: '' }),
  adminSetTask: (enabled: boolean): Promise<void> => call('admin_set_task', { enabled }),
  adminRestart: (): Promise<void> => call('admin_restart'),
  importEspanso: (): Promise<Imported> => call('import_espanso', undefined, { snippets: [], skipped: [], files: 0, dir: '' }),

  /** Подписка на событие ядра. Возвращает функцию отписки. */
  on<T>(channel: string, fn: (payload: T) => void): () => void {
    if (!native) return () => {}
    let un: (() => void) | undefined
    let cancelled = false
    void listen<T>(channel, (e) => fn(e.payload)).then((u) => {
      if (cancelled) u()
      else un = u
    })
    return () => {
      cancelled = true
      un?.()
    }
  },
}

/** Текст ошибки из invoke: ядро возвращает строки, но на всякий случай. */
export function errText(e: unknown): string {
  return typeof e === 'string' ? e : e instanceof Error ? e.message : String(e)
}
