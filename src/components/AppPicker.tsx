import { useEffect, useRef, useState, type JSX } from 'react'
import { api, type RunningApp } from '../lib/api'

interface Props {
  /** Имена из раздела «Приложения». */
  known: string[]
  exclude: string[]
  onPick: (name: string) => void
  label?: string
}

// Выбор приложения: сначала свои именованные (telegram, terminal),
// ниже — запущенные сейчас программы по имени exe.
export function AppPicker({ known, exclude, onPick, label = '+ Добавить' }: Props): JSX.Element {
  const [open, setOpen] = useState(false)
  const [running, setRunning] = useState<RunningApp[]>([])
  const [custom, setCustom] = useState('')
  const box = useRef<HTMLDivElement>(null)
  const pop = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (!open) return
    // Список может оказаться ниже края окна — прокручиваем его в поле зрения.
    void api.listApps().then((r) => {
      setRunning(r)
      requestAnimationFrame(() => pop.current?.scrollIntoView({ block: 'nearest' }))
    })
    const close = (e: MouseEvent): void => {
      if (box.current && !box.current.contains(e.target as Node)) setOpen(false)
    }
    document.addEventListener('mousedown', close)
    return () => document.removeEventListener('mousedown', close)
  }, [open])

  const pick = (name: string): void => {
    onPick(name)
    setOpen(false)
    setCustom('')
  }
  const knownLeft = known.filter((k) => !exclude.includes(k))
  const runningLeft = running.filter((r) => !exclude.some((e) => e.toLowerCase() === r.exe))

  return (
    <div className="picker" ref={box}>
      <button className="chip-add" onClick={() => setOpen(!open)}>{label}</button>
      {open && (
        <div className="picker-pop" ref={pop}>
          {knownLeft.length > 0 && <div className="picker-head">Мои приложения</div>}
          {knownLeft.map((k) => (
            <button key={k} className="picker-item" onClick={() => pick(k)}>{k}</button>
          ))}
          <div className="picker-head">Запущены сейчас</div>
          {runningLeft.length === 0 && <div className="picker-empty">Нет открытых окон</div>}
          {runningLeft.map((r) => (
            <button key={r.exe} className="picker-item" onClick={() => pick(r.exe)}>
              <span>{r.exe}</span>
              <span className="muted">{r.title}</span>
            </button>
          ))}
          <form
            className="picker-custom"
            onSubmit={(e) => {
              e.preventDefault()
              if (custom.trim()) pick(custom.trim())
            }}
          >
            <input value={custom} onChange={(e) => setCustom(e.target.value)} placeholder="или впишите: Excel.exe" />
          </form>
        </div>
      )}
    </div>
  )
}
