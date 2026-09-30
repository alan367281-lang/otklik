// Единственное место, где интерфейс разговаривает с ядром. Вне Tauri
// (npm run dev:web в браузере) команды возвращают заглушки, чтобы вёрстку
// можно было смотреть без сборки Rust.
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import type { Config, Snippet } from './config'

export interface RuleView {
  name: string
  hotkey: string | null
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

const native = '__TAURI_INTERNALS__' in window

const demoConfig: Config = {
  version: 1,
  settings: { pause_hotkey: 'Ctrl+Alt+Pause', search_hotkey: 'Ctrl+Alt+Space', paste_threshold: 100, restore_clipboard_ms: 300 },
  apps: { notepad: { exe: 'Notepad.exe', title_contains: null, paste_keys: null } },
  snippets: [
    { name: 'Тест', enabled: true, group: null, hotkey: 'Alt+1', abbr: null, text: 'Это тестовый текст от Key Manager.', keys: null, open: null, method: 'auto', only_in: null, except_in: null },
    { name: 'Подпись', enabled: true, group: 'Почта', hotkey: 'Ctrl+Alt+2', abbr: null, text: 'С уважением,\nИван', keys: null, open: null, method: 'auto', only_in: null, except_in: null },
  ],
}

const demoStatus: Status = {
  paused: false,
  configPath: 'C:\\Users\\…\\app.keymanager.desktop\\config.yaml',
  loadedAt: '12:00:00',
  error: null,
  warnings: [],
  pauseHotkey: 'Ctrl+Alt+Pause',
  activeCount: 2,
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
    call('check_config', { config }, { rules: config.snippets.map(() => ({ name: '', hotkey: null, scope: '', preview: '', active: true, problem: null })), warnings: [] }),
  listApps: (): Promise<RunningApp[]> =>
    call('list_apps', undefined, [{ exe: 'telegram.exe', title: 'Telegram' }, { exe: 'chrome.exe', title: 'Google Chrome' }]),
  /** null — сочетание можно занимать, иначе — почему нельзя. */
  checkChord: (chord: string): Promise<string | null> => call('check_chord', { chord }, null),
  testSnippet: (snippet: Snippet): Promise<void> => call('test_snippet', { snippet }),
  getAutostart: (): Promise<boolean> => call('get_autostart', undefined, false),
  setAutostart: (enabled: boolean): Promise<void> => call('set_autostart', { enabled }),

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
