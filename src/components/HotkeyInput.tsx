import { useEffect, useRef, useState, type JSX } from 'react'
import { api } from '../lib/api'
import { Keys } from './Keys'

interface Props {
  value: string | null
  onChange: (v: string | null) => void
  /** Можно ли оставить поле пустым (Backspace очищает). */
  clearable?: boolean
}

// Имена клавиш по физическому положению (e.code), а не по символу: так
// сочетание одинаково записывается в русской и английской раскладке.
// Имена — те же, что понимает ядро в config.yaml.
const CODE_NAMES: Record<string, string> = {
  Space: 'Space', Enter: 'Enter', NumpadEnter: 'Enter', Tab: 'Tab', Escape: 'Esc', Backspace: 'Backspace',
  Pause: 'Pause', Insert: 'Insert', Delete: 'Delete', Home: 'Home', End: 'End', PageUp: 'PgUp', PageDown: 'PgDn',
  ArrowLeft: 'Left', ArrowUp: 'Up', ArrowRight: 'Right', ArrowDown: 'Down', PrintScreen: 'PrintScreen',
  Backquote: '`', Minus: '-', Equal: '=', BracketLeft: '[', BracketRight: ']', Semicolon: ';', Quote: "'",
  Comma: ',', Period: '.', Slash: '/', Backslash: '\\',
  NumpadMultiply: 'Num*', NumpadAdd: 'Num+', NumpadSubtract: 'Num-', NumpadDecimal: 'Num.', NumpadDivide: 'Num/',
}

function keyName(code: string): string | null {
  if (/^Key[A-Z]$/.test(code)) return code.slice(3)
  if (/^Digit[0-9]$/.test(code)) return code.slice(5)
  if (/^Numpad[0-9]$/.test(code)) return 'Num' + code.slice(6)
  if (/^F([1-9]|1[0-9]|2[0-4])$/.test(code)) return code
  return CODE_NAMES[code] ?? null
}

const MODIFIER_KEYS = new Set(['Control', 'Alt', 'Shift', 'Meta', 'AltGraph'])

function modsOf(e: KeyboardEvent): string[] {
  const m: string[] = []
  if (e.ctrlKey) m.push('Ctrl')
  if (e.altKey) m.push('Alt')
  if (e.shiftKey) m.push('Shift')
  if (e.metaKey) m.push('Win')
  return m
}

// Сочетание записывает само окно. Перехват клавиатуры ядра здесь не
// поможет: пока окно Отклика в фокусе, Windows его не вызывает.
// Проверяет записанное ядро — по тем же правилам, что и файл настроек.
export function HotkeyInput({ value, onChange, clearable = true }: Props): JSX.Element {
  const [recording, setRecording] = useState(false)
  const [held, setHeld] = useState<string[]>([])
  const [problem, setProblem] = useState<string | null>(null)
  const onChangeRef = useRef(onChange)
  onChangeRef.current = onChange

  useEffect(() => {
    if (!recording) return
    const finish = (): void => {
      setRecording(false)
      setHeld([])
    }
    const onKeyDown = (e: KeyboardEvent): void => {
      // Всё нажатое во время записи — наше: ни браузер, ни окно не должны
      // на это реагировать (Ctrl+R, Alt для меню и т. п.).
      e.preventDefault()
      e.stopPropagation()
      if (MODIFIER_KEYS.has(e.key)) {
        setHeld(modsOf(e))
        return
      }
      const mods = modsOf(e)
      if (mods.length === 0 && e.code === 'Escape') return finish()
      if (mods.length === 0 && (e.code === 'Backspace' || e.code === 'Delete') && clearable) {
        setProblem(null)
        onChangeRef.current(null)
        return finish()
      }
      const name = keyName(e.code)
      if (!name) {
        setProblem(`Эту клавишу пока нельзя использовать (${e.code || e.key})`)
        return finish()
      }
      const chord = [...mods, name].join('+')
      finish()
      void api.checkChord(chord).then((why) => {
        if (why) setProblem(`${chord}: ${why}`)
        else {
          setProblem(null)
          onChangeRef.current(chord)
        }
      })
    }
    const onKeyUp = (e: KeyboardEvent): void => {
      e.preventDefault()
      setHeld(modsOf(e))
    }
    window.addEventListener('keydown', onKeyDown, true)
    window.addEventListener('keyup', onKeyUp, true)
    window.addEventListener('blur', finish)
    return () => {
      window.removeEventListener('keydown', onKeyDown, true)
      window.removeEventListener('keyup', onKeyUp, true)
      window.removeEventListener('blur', finish)
    }
  }, [recording, clearable])

  const start = (): void => {
    setProblem(null)
    setHeld([])
    setRecording(true)
  }

  return (
    <div className="hotkey">
      <button className={recording ? 'hotkey-field rec' : 'hotkey-field'} onClick={recording ? undefined : start}>
        {recording ? (
          held.length > 0 ? (
            <span className="rec-hint">{held.join('+')}+…</span>
          ) : (
            <span className="rec-hint">Нажмите сочетание… Esc — отмена{clearable ? ', Backspace — очистить' : ''}</span>
          )
        ) : value ? (
          <Keys chord={value} />
        ) : (
          <span className="muted">не задано — нажмите, чтобы записать</span>
        )}
      </button>
      {problem && <div className="field-problem">{problem}</div>}
    </div>
  )
}
