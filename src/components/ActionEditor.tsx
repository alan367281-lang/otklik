import { useEffect, useRef, useState, type JSX } from 'react'
import { api, errText } from '../lib/api'
import { actionKind, newStep, stepKind, type ActionKind, type Snippet, type Step, type StepKind } from '../lib/config'
import { Segmented } from './Segmented'

interface Props {
  snippet: Snippet
  set: (patch: Partial<Snippet>) => void
}

const EMPTY: Partial<Snippet> = { text: null, keys: null, open: null, args: null, steps: null }

// Что делает заготовка: вставляет текст, нажимает клавиши, открывает
// программу или ссылку — или всё это цепочкой.
export function ActionEditor({ snippet: s, set }: Props): JSX.Element {
  const kind = actionKind(s)

  const changeKind = (k: ActionKind): void => {
    if (k === kind) return
    if (k === 'text') set({ ...EMPTY, text: '' })
    if (k === 'keys') set({ ...EMPTY, keys: '' })
    if (k === 'open') set({ ...EMPTY, open: '' })
    if (k === 'steps') set({ ...EMPTY, steps: [newStep('open'), newStep('wait'), newStep('text')] })
  }

  return (
    <>
      <div className="field">
        <div className="field-label"><span>Действие</span></div>
        <Segmented<ActionKind>
          value={kind}
          onChange={changeKind}
          options={[
            { value: 'text', label: 'Вставить текст' },
            { value: 'keys', label: 'Нажать клавиши' },
            { value: 'open', label: 'Открыть' },
            { value: 'steps', label: 'Цепочка' },
          ]}
        />
      </div>
      {kind === 'text' && <TextField value={s.text ?? ''} onChange={(v) => set({ text: v })} rows={8} />}
      {kind === 'keys' && <KeysField value={s.keys ?? ''} onChange={(v) => set({ keys: v })} />}
      {kind === 'open' && (
        <OpenFields target={s.open ?? ''} args={s.args ?? ''} onChange={(open, args) => set({ open, args: args || null })} />
      )}
      {kind === 'steps' && <StepsField steps={s.steps ?? []} onChange={(steps) => set({ steps })} />}
    </>
  )
}

/** Вставить фрагмент в textarea/input на место курсора. */
function insertAt(el: HTMLTextAreaElement | HTMLInputElement | null, value: string, piece: string, onChange: (v: string) => void): void {
  const start = el?.selectionStart ?? value.length
  const end = el?.selectionEnd ?? value.length
  onChange(value.slice(0, start) + piece + value.slice(end))
  requestAnimationFrame(() => {
    if (!el) return
    el.focus()
    el.setSelectionRange(start + piece.length, start + piece.length)
  })
}

const TEXT_TOKENS: { label: string; token: string; hint: string }[] = [
  { label: 'Дата', token: '{date}', hint: '30.09.2026' },
  { label: 'Дата словами', token: '{date:%d %B %Y}', hint: '30 сентября 2026' },
  { label: 'Завтра', token: '{date+1d}', hint: 'сдвиг: +1d, -1w, +1m, +1y' },
  { label: 'Время', token: '{time}', hint: '14:05' },
  { label: 'Буфер обмена', token: '{clipboard}', hint: 'то, что сейчас скопировано' },
  { label: 'Курсор', token: '{cursor}', hint: 'где оставить курсор после вставки' },
]

function TextField(p: { value: string; onChange: (v: string) => void; rows: number }): JSX.Element {
  const ref = useRef<HTMLTextAreaElement>(null)
  const [preview, setPreview] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)
  const hasTokens = /\{(date|дата|time|время|clipboard|буфер|cursor|курсор)/i.test(p.value)

  useEffect(() => {
    if (!hasTokens) {
      setPreview(null)
      setError(null)
      return
    }
    const t = window.setTimeout(() => {
      api
        .previewText(p.value)
        .then((v) => {
          setPreview(v)
          setError(null)
        })
        .catch((e: unknown) => setError(errText(e)))
    }, 250)
    return () => window.clearTimeout(t)
  }, [p.value, hasTokens])

  return (
    <div className="field">
      <div className="field-label">
        <span>Текст</span>
        <span className="muted">{p.value.length} симв.</span>
      </div>
      <textarea ref={ref} className="text-area" value={p.value} onChange={(e) => p.onChange(e.target.value)} rows={p.rows} spellCheck={false} />
      <div className="token-row">
        <span className="muted">Вставить:</span>
        {TEXT_TOKENS.map((t) => (
          <button key={t.token} className="token" title={`${t.token} — ${t.hint}`} onClick={() => insertAt(ref.current, p.value, t.token, p.onChange)}>
            {t.label}
          </button>
        ))}
      </div>
      {error && <div className="field-problem">{error}</div>}
      {preview !== null && !error && (
        <div className="preview-box">
          <div className="label">Сейчас вставится:</div>
          <div className="preview-text">{preview}</div>
        </div>
      )}
    </div>
  )
}

const KEY_TOKENS = ['{Enter}', '{Tab}', '{Esc}', '{Ctrl+A}', '{Ctrl+C}', '{Ctrl+V}', '{Tab 2}', '{wait 200}']

function KeysField(p: { value: string; onChange: (v: string) => void; compact?: boolean }): JSX.Element {
  const ref = useRef<HTMLTextAreaElement>(null)
  return (
    <div className="field">
      {!p.compact && <div className="field-label"><span>Клавиши</span></div>}
      <textarea ref={ref} className="mono-area" value={p.value} onChange={(e) => p.onChange(e.target.value)} rows={p.compact ? 2 : 3} spellCheck={false} placeholder="Иванов{Tab}Иван{Tab}{wait 200}{Enter}" />
      <div className="token-row">
        {KEY_TOKENS.map((t) => (
          <button key={t} className="token mono" onClick={() => insertAt(ref.current, p.value, t, p.onChange)}>{t}</button>
        ))}
      </div>
      {!p.compact && (
        <div className="hint">
          Обычный текст печатается как есть. В фигурных скобках — клавиши и сочетания: {'{Enter}'}, {'{Ctrl+Shift+T}'},
          {' {Tab 3}'} — нажать трижды, {'{wait 200}'} — пауза в миллисекундах, {'{date}'} и другие подстановки.
        </div>
      )}
    </div>
  )
}

function OpenFields(p: { target: string; args: string; onChange: (target: string, args: string) => void; compact?: boolean }): JSX.Element {
  return (
    <div className="field">
      {!p.compact && <div className="field-label"><span>Что открыть</span></div>}
      <input value={p.target} onChange={(e) => p.onChange(e.target.value, p.args)} placeholder="https://tracker.yandex.ru  или  C:\Program Files\…\app.exe  или  %USERPROFILE%\Documents" />
      <input className="args-input" value={p.args} onChange={(e) => p.onChange(p.target, e.target.value)} placeholder="Аргументы программы (необязательно)" />
      {!p.compact && <div className="hint">Сайт, папка, файл или программа — как в окне «Выполнить» (Win+R). Можно писать %USERPROFILE% и другие переменные.</div>}
    </div>
  )
}

const STEP_LABELS: Record<StepKind, string> = { text: 'Текст', keys: 'Клавиши', open: 'Открыть', wait: 'Пауза' }

function StepsField(p: { steps: Step[]; onChange: (s: Step[]) => void }): JSX.Element {
  const upd = (i: number, st: Step): void => p.onChange(p.steps.map((x, j) => (j === i ? st : x)))
  const move = (i: number, d: -1 | 1): void => {
    const j = i + d
    if (j < 0 || j >= p.steps.length) return
    const next = [...p.steps]
    ;[next[i], next[j]] = [next[j], next[i]]
    p.onChange(next)
  }
  return (
    <div className="field">
      <div className="field-label"><span>Шаги — выполняются по порядку</span></div>
      <div className="steps">
        {p.steps.map((st, i) => {
          const k = stepKind(st)
          return (
            <div key={i} className="step">
              <div className="step-head">
                <span className="step-num">{i + 1}</span>
                <select value={k} onChange={(e) => upd(i, newStep(e.target.value as StepKind))}>
                  {(Object.keys(STEP_LABELS) as StepKind[]).map((x) => (
                    <option key={x} value={x}>{STEP_LABELS[x]}</option>
                  ))}
                </select>
                <span className="spacer" />
                <button onClick={() => move(i, -1)} disabled={i === 0}>↑</button>
                <button onClick={() => move(i, 1)} disabled={i === p.steps.length - 1}>↓</button>
                <button className="danger" onClick={() => p.onChange(p.steps.filter((_, j) => j !== i))}>×</button>
              </div>
              {k === 'text' && (
                <textarea className="text-area small" value={st.text ?? ''} onChange={(e) => upd(i, { text: e.target.value })} rows={3} placeholder="Текст, можно с {date}, {clipboard}…" />
              )}
              {k === 'keys' && <KeysField compact value={st.keys ?? ''} onChange={(v) => upd(i, { keys: v })} />}
              {k === 'open' && <OpenFields compact target={st.open ?? ''} args={st.args ?? ''} onChange={(o, a) => upd(i, { open: o, args: a || null })} />}
              {k === 'wait' && (
                <div className="num">
                  <input type="number" min={10} max={60000} step={100} value={st.wait ?? 500} onChange={(e) => upd(i, { wait: Math.max(10, Math.min(60000, Math.round(Number(e.target.value) || 0))) })} />
                  <span className="muted">мс — дать программе открыться или окну появиться</span>
                </div>
              )}
            </div>
          )
        })}
      </div>
      <div className="token-row">
        <span className="muted">Добавить шаг:</span>
        {(Object.keys(STEP_LABELS) as StepKind[]).map((x) => (
          <button key={x} className="token" onClick={() => p.onChange([...p.steps, newStep(x)])}>+ {STEP_LABELS[x]}</button>
        ))}
      </div>
    </div>
  )
}
