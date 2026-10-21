import { useState, type JSX } from 'react'
import { list, type AppDef, type Config } from '../lib/config'
import { AppPicker } from '../components/AppPicker'
import { isMac } from '../lib/platform'

interface Props {
  config: Config
  update: (fn: (c: Config) => void) => void
}

// Именованные приложения: одно имя может означать несколько программ
// (все терминалы) или часть программы (Gmail — это chrome.exe + заголовок).
export function AppsPage({ config, update }: Props): JSX.Element {
  const names = Object.keys(config.apps)

  const addFromExe = (exe: string): void => {
    let name = exe.toLowerCase().replace(/\.(exe|app)$/, '').replace(/[^a-zа-я0-9_-]+/gi, '')
    if (!name) name = 'app'
    let n = name
    for (let i = 2; config.apps[n]; i++) n = `${name}${i}`
    // Имя без хвоста дополняем по системе: Telegram → Telegram.exe / Telegram.app.
    const full = /\.(exe|app)$/i.test(exe) ? exe : `${exe}${isMac ? '.app' : '.exe'}`
    update((c) => {
      c.apps[n] = { exe: full, title_contains: null, paste_keys: null }
    })
  }

  return (
    <div className="page-pad">
      <div className="page-intro">
        <h2>Приложения</h2>
        <p className="hint">
          Короткие имена для правил «Только в…» и «Везде, кроме…». В правиле можно указать и просто exe-файл —
          имя здесь нужно, когда программ несколько (все терминалы), когда важен заголовок окна (Gmail в Chrome) или
          когда в программе вставлять надо по-особому.
        </p>
        <AppPicker known={[]} exclude={[]} onPick={addFromExe} label="+ Добавить приложение" />
      </div>
      {names.length === 0 && <div className="empty">Пока ни одного.</div>}
      {names.map((name) => (
        <AppCard key={name} name={name} def={config.apps[name]} config={config} update={update} />
      ))}
    </div>
  )
}

function AppCard(p: { name: string; def: AppDef; config: Config; update: Props['update'] }): JSX.Element {
  const { name, def, config, update } = p
  const [draftName, setDraftName] = useState(name)
  const used = config.snippets.filter((s) => [...list(s.only_in), ...list(s.except_in)].includes(name)).length

  const set = (patch: Partial<AppDef>): void =>
    update((c) => {
      Object.assign(c.apps[name], patch)
    })

  // Переименование обновляет и ссылки в правилах, иначе они «потеряют» приложение.
  const rename = (): void => {
    const next = draftName.trim()
    if (!next || next === name || config.apps[next]) {
      setDraftName(name)
      return
    }
    update((c) => {
      const entries = Object.entries(c.apps).map(([k, v]) => [k === name ? next : k, v] as const)
      c.apps = Object.fromEntries(entries)
      const fix = (v: string[]): string[] | null => {
        const r = v.map((x) => (x === name ? next : x))
        return r.length ? r : null
      }
      for (const s of c.snippets) {
        s.only_in = fix(list(s.only_in))
        s.except_in = fix(list(s.except_in))
      }
    })
  }

  const remove = (): void =>
    update((c) => {
      delete c.apps[name]
    })

  return (
    <div className="card app-card">
      <div className="row">
        <input className="app-name" value={draftName} onChange={(e) => setDraftName(e.target.value)} onBlur={rename} onKeyDown={(e) => e.key === 'Enter' && (e.target as HTMLInputElement).blur()} />
        <span className="muted">{used ? `используется в правилах: ${used}` : 'не используется'}</span>
        <span className="spacer" />
        <button className="danger" onClick={remove} title={used ? 'Правила с этим именем перестанут работать' : undefined}>Удалить</button>
      </div>
      <div className="app-grid">
        <label>Программы через запятую</label>
        <input
          value={list(def.exe).join(', ')}
          onChange={(e) => {
            const v = e.target.value.split(',').map((x) => x.trim())
            set({ exe: v.filter(Boolean).length ? v : null })
          }}
          placeholder={isMac ? 'Telegram.app' : 'Telegram.exe'}
        />
        <label>Заголовок окна содержит</label>
        <input value={def.title_contains ?? ''} onChange={(e) => set({ title_contains: e.target.value || null })} placeholder="необязательно, например Gmail" />
        <label>Чем вставлять</label>
        <input value={def.paste_keys ?? ''} onChange={(e) => set({ paste_keys: e.target.value || null })} placeholder={isMac ? 'Cmd+V (как обычно)' : 'Ctrl+V (как обычно); терминалам — Ctrl+Shift+V'} />
      </div>
    </div>
  )
}
