import { useMemo, useRef, useState, type JSX } from 'react'
import { api, errText, type RuleView } from '../lib/api'
import { actionKind, list, newSnippet, type Config, type Method, type OneOrMany, type Snippet } from '../lib/config'
import { ActionEditor } from '../components/ActionEditor'
import { AppPicker } from '../components/AppPicker'
import { HotkeyInput } from '../components/HotkeyInput'
import { Keys } from '../components/Keys'
import { Segmented } from '../components/Segmented'

interface Props {
  config: Config
  views: RuleView[]
  update: (fn: (c: Config) => void) => void
}

const ALL = '\u0000все'
const NONE = '\u0000без группы'

export function SnippetsPage({ config, views, update }: Props): JSX.Element {
  const [group, setGroup] = useState<string>(ALL)
  const [query, setQuery] = useState('')
  const [selected, setSelected] = useState<number>(config.snippets.length > 0 ? 0 : -1)

  const groups = useMemo(() => {
    const g = new Set<string>()
    for (const s of config.snippets) if (s.group) g.add(s.group)
    return [...g].sort((a, b) => a.localeCompare(b, 'ru'))
  }, [config.snippets])

  const visible = useMemo(() => {
    const q = query.trim().toLowerCase()
    return config.snippets
      .map((s, i) => ({ s, i }))
      .filter(({ s }) => group === ALL || (group === NONE ? !s.group : s.group === group))
      .filter(({ s }) => !q || [s.name, s.text, s.hotkey].some((f) => f?.toLowerCase().includes(q)))
  }, [config.snippets, group, query])

  const add = (): void => {
    const g = group === ALL || group === NONE ? null : group
    update((c) => {
      c.snippets.push(newSnippet(g))
    })
    setSelected(config.snippets.length)
    setQuery('')
  }

  const current = selected >= 0 && selected < config.snippets.length ? config.snippets[selected] : null
  const noGroupCount = config.snippets.filter((s) => !s.group).length

  return (
    <div className="snip-layout">
      <nav className="groups">
        <GroupButton label="Все заготовки" count={config.snippets.length} on={group === ALL} onClick={() => setGroup(ALL)} />
        {groups.map((g) => (
          <GroupButton key={g} label={g} count={config.snippets.filter((s) => s.group === g).length} on={group === g} onClick={() => setGroup(g)} />
        ))}
        {groups.length > 0 && noGroupCount > 0 && (
          <GroupButton label="Без группы" count={noGroupCount} on={group === NONE} onClick={() => setGroup(NONE)} />
        )}
      </nav>

      <div className="snip-list">
        <div className="list-tools">
          <input className="search" value={query} onChange={(e) => setQuery(e.target.value)} placeholder="Поиск" />
          <button className="primary" onClick={add}>+ Новая</button>
        </div>
        <div className="list-scroll">
          {visible.length === 0 && <div className="empty">Ничего не найдено</div>}
          {visible.map(({ s, i }) => {
            const v = views[i]
            return (
              <button key={i} className={'snip-item' + (i === selected ? ' sel' : '') + (s.enabled ? '' : ' off')} onClick={() => setSelected(i)}>
                <div className="snip-top">
                  <span className="snip-name">{s.name || 'Без названия'}</span>
                  {v?.problem && <span className="warn-dot" title={v.problem} />}
                </div>
                <div className="snip-meta">
                  {s.hotkey ? <Keys chord={s.hotkey} /> : <span className="muted">без сочетания</span>}
                  {v && v.scope !== 'везде' && <span className="muted scope-tag">{v.scope}</span>}
                </div>
              </button>
            )
          })}
        </div>
      </div>

      <div className="editor">
        {current ? (
          <Editor
            key={selected}
            index={selected}
            snippet={current}
            view={views[selected]}
            config={config}
            groups={groups}
            update={update}
            onSelect={setSelected}
          />
        ) : (
          <div className="empty big">
            Выберите заготовку слева или создайте новую.
            <button className="primary" onClick={add}>+ Новая заготовка</button>
          </div>
        )}
      </div>
    </div>
  )
}

function GroupButton(p: { label: string; count: number; on: boolean; onClick: () => void }): JSX.Element {
  return (
    <button className={'group' + (p.on ? ' on' : '')} onClick={p.onClick}>
      <span>{p.label}</span>
      <span className="count">{p.count}</span>
    </button>
  )
}

type Scope = 'all' | 'only' | 'except'

interface EditorProps {
  index: number
  snippet: Snippet
  view: RuleView | undefined
  config: Config
  groups: string[]
  update: (fn: (c: Config) => void) => void
  onSelect: (i: number) => void
}

function Editor({ index, snippet: s, view, config, groups, update, onSelect }: EditorProps): JSX.Element {
  const [testing, setTesting] = useState(false)
  const [testText, setTestText] = useState('')
  const [testError, setTestError] = useState<string | null>(null)
  const testRef = useRef<HTMLTextAreaElement>(null)

  const set = (patch: Partial<Snippet>): void =>
    update((c) => {
      Object.assign(c.snippets[index], patch)
    })

  const only = list(s.only_in)
  const except = list(s.except_in)
  const fromFile: Scope = only.length > 0 ? 'only' : except.length > 0 ? 'except' : 'all'
  // Режим, выбранный кнопкой: «Только в…» с пустым списком в файле
  // выглядит как «везде», но человек как раз собирается добавить приложение.
  const [picked, setPicked] = useState<Scope>(fromFile)
  const scope: Scope = fromFile !== 'all' ? fromFile : picked
  const scopeList = scope === 'except' ? except : only
  const setScopeList = (v: string[]): void => {
    const val: OneOrMany = v.length === 0 ? null : v
    set(scope === 'except' ? { except_in: val } : { only_in: val })
  }
  // Переключение «только в / кроме» переносит уже выбранные приложения.
  const changeScope = (next: Scope): void => {
    setPicked(next)
    const cur = [...only, ...except]
    if (next === 'all') set({ only_in: null, except_in: null })
    if (next === 'only') set({ only_in: cur.length ? cur : null, except_in: null })
    if (next === 'except') set({ except_in: cur.length ? cur : null, only_in: null })
  }

  const move = (d: -1 | 1): void => {
    const j = index + d
    if (j < 0 || j >= config.snippets.length) return
    update((c) => {
      const [x] = c.snippets.splice(index, 1)
      c.snippets.splice(j, 0, x)
    })
    onSelect(j)
  }
  const duplicate = (): void => {
    update((c) => {
      c.snippets.splice(index + 1, 0, { ...structuredClone(s), name: s.name + ' (копия)', hotkey: null })
    })
    onSelect(index + 1)
  }
  const remove = (): void => {
    update((c) => {
      c.snippets.splice(index, 1)
    })
    onSelect(Math.min(index, config.snippets.length - 2))
  }
  const test = (): void => {
    setTesting(true)
    setTestError(null)
    setTestText('')
    // Даём полю получить фокус, потом ядро «печатает» в него, как в любое окно.
    window.setTimeout(() => {
      testRef.current?.focus()
      window.setTimeout(() => {
        api.testSnippet(s).catch((e: unknown) => setTestError(errText(e)))
      }, 60)
    }, 30)
  }

  const kind = actionKind(s)

  return (
    <div className="editor-inner">
      <div className="ed-head">
        <input className="title-input" value={s.name} onChange={(e) => set({ name: e.target.value })} placeholder="Название" />
        <label className="toggle" title="Выключенная заготовка остаётся в списке, но не срабатывает">
          <input type="checkbox" checked={s.enabled} onChange={(e) => set({ enabled: e.target.checked })} />
          <span>Включена</span>
        </label>
      </div>

      {view?.problem && <div className="alert warn">{view.problem}</div>}

      <Field label="Сочетание клавиш">
        <HotkeyInput value={s.hotkey} onChange={(v) => set({ hotkey: v })} />
      </Field>

      <ActionEditor snippet={s} set={set} />

      {(kind === 'text' || (kind === 'steps' && (s.steps ?? []).some((st) => typeof st.text === 'string'))) && (
        <Field label="Как вставлять текст">
          <Segmented<Method>
            value={s.method}
            onChange={(m) => set({ method: m })}
            options={[
              { value: 'auto', label: 'Сам решит' },
              { value: 'paste', label: 'Через буфер' },
              { value: 'type', label: 'Печатью' },
            ]}
          />
          <div className="hint">
            {s.method === 'auto' && `Короткий текст печатается, длинный (больше ${config.settings.paste_threshold} симв.) или с переносами — через буфер обмена.`}
            {s.method === 'paste' && 'Быстро для любого объёма; буфер обмена потом возвращается как был.'}
            {s.method === 'type' && 'Посимвольно — для мест, где вставка из буфера запрещена. Длинный текст печатается заметно дольше.'}
          </div>
        </Field>
      )}

      <Field label="Где работает">
        <Segmented<Scope>
          value={scope}
          onChange={changeScope}
          options={[
            { value: 'all', label: 'Везде' },
            { value: 'only', label: 'Только в…' },
            { value: 'except', label: 'Везде, кроме…' },
          ]}
        />
        {scope !== 'all' && (
          <div className="chips">
            {scopeList.map((a) => (
              <span key={a} className="chip">
                {a}
                <button onClick={() => setScopeList(scopeList.filter((x) => x !== a))} title="Убрать">×</button>
              </span>
            ))}
            <AppPicker known={Object.keys(config.apps)} exclude={scopeList} onPick={(n) => setScopeList([...scopeList, n])} />
          </div>
        )}
        {scope !== 'all' && scopeList.length === 0 && <div className="hint">Добавьте хотя бы одно приложение.</div>}
      </Field>

      <Field label="Группа">
        <input list="km-groups" value={s.group ?? ''} onChange={(e) => set({ group: e.target.value || null })} placeholder="без группы" />
        <datalist id="km-groups">
          {groups.map((g) => (
            <option key={g} value={g} />
          ))}
        </datalist>
      </Field>

      <div className="ed-actions">
        <button onClick={test} title={kind === 'open' ? 'Откроет указанное — как по сочетанию' : undefined}>▶ Проверить</button>
        <span className="spacer" />
        <button onClick={() => move(-1)} disabled={index === 0} title="Выше: при одинаковом сочетании срабатывает верхняя">↑</button>
        <button onClick={() => move(1)} disabled={index === config.snippets.length - 1} title="Ниже">↓</button>
        <button onClick={duplicate}>Дублировать</button>
        <button className="danger" onClick={remove}>Удалить</button>
      </div>

      {testing && (
        <div className="test-box">
          <div className="label">Поле проверки — сюда вставится текст так же, как в любую программу</div>
          <textarea ref={testRef} value={testText} onChange={(e) => setTestText(e.target.value)} rows={4} />
          {testError && <div className="field-problem">{testError}</div>}
        </div>
      )}
    </div>
  )
}

function Field(p: { label: string; hint?: string; children: React.ReactNode }): JSX.Element {
  return (
    <div className="field">
      <div className="field-label">
        <span>{p.label}</span>
        {p.hint && <span className="muted">{p.hint}</span>}
      </div>
      {p.children}
    </div>
  )
}
