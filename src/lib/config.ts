// Модель config.yaml на стороне окна. Поля — snake_case, как в файле:
// так черновик уходит в ядро без переименований.

export type OneOrMany = string | string[] | null
export type Method = 'auto' | 'paste' | 'type'
export type AbbrMode = 'auto' | 'instant' | 'word'

export interface Settings {
  pause_hotkey: string
  search_hotkey: string
  paste_threshold: number
  restore_clipboard_ms: number
}

export interface AppDef {
  exe: OneOrMany
  title_contains: string | null
  paste_keys: string | null
}

/** Шаг цепочки: заполнено ровно одно из text, keys, open, wait. */
export interface Step {
  text?: string | null
  keys?: string | null
  open?: string | null
  args?: string | null
  wait?: number | null
}

export type StepKind = 'text' | 'keys' | 'open' | 'wait'

export function stepKind(s: Step): StepKind {
  if (s.wait !== undefined && s.wait !== null) return 'wait'
  if (s.keys !== undefined && s.keys !== null) return 'keys'
  if (s.open !== undefined && s.open !== null) return 'open'
  return 'text'
}

export function newStep(kind: StepKind): Step {
  if (kind === 'wait') return { wait: 500 }
  if (kind === 'keys') return { keys: '' }
  if (kind === 'open') return { open: '' }
  return { text: '' }
}

export type ActionKind = 'text' | 'keys' | 'open' | 'steps'

export function actionKind(s: Snippet): ActionKind {
  if (s.steps !== null) return 'steps'
  if (s.keys !== null) return 'keys'
  if (s.open !== null) return 'open'
  return 'text'
}

export interface Snippet {
  name: string
  enabled: boolean
  group: string | null
  hotkey: string | null
  abbr: string | null
  abbr_mode: AbbrMode
  text: string | null
  keys: string | null
  open: string | null
  args: string | null
  steps: Step[] | null
  method: Method
  only_in: OneOrMany
  except_in: OneOrMany
}

export interface Config {
  version: number
  settings: Settings
  apps: Record<string, AppDef>
  snippets: Snippet[]
}

export function list(v: OneOrMany): string[] {
  if (v === null || v === undefined) return []
  return Array.isArray(v) ? v : [v]
}

export function newSnippet(group: string | null): Snippet {
  return {
    name: 'Новая заготовка',
    enabled: true,
    group,
    hotkey: null,
    abbr: null,
    abbr_mode: 'auto',
    text: '',
    keys: null,
    open: null,
    args: null,
    steps: null,
    method: 'auto',
    only_in: null,
    except_in: null,
  }
}

const BLANK: Snippet = { ...newSnippet(null), name: '', text: null }

// Ядро не пишет в ответ поля со значениями по умолчанию (как и в файл) —
// дополняем их, чтобы интерфейсу не проверять каждое поле на отсутствие.
export function withDefaults(c: Config): Config {
  return {
    version: c.version ?? 1,
    settings: c.settings,
    apps: Object.fromEntries(
      Object.entries(c.apps ?? {}).map(([k, a]) => [
        k,
        { exe: a.exe ?? null, title_contains: a.title_contains ?? null, paste_keys: a.paste_keys ?? null },
      ]),
    ),
    snippets: (c.snippets ?? []).map((s) => ({ ...BLANK, ...s })),
  }
}

export function sameConfig(a: Config | null, b: Config | null): boolean {
  return JSON.stringify(a) === JSON.stringify(b)
}
