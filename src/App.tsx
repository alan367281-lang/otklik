import { useCallback, useEffect, useState, type JSX } from 'react'
import { api, type LogEntry, type Status } from './lib/api'

export function App(): JSX.Element {
  const [status, setStatus] = useState<Status | null>(null)
  const [log, setLog] = useState<LogEntry[]>([])

  const refresh = useCallback((): void => {
    void api.status().then(setStatus)
    void api.log().then(setLog)
  }, [])

  useEffect(() => {
    refresh()
    const offStatus = api.on('status', refresh)
    const offLog = api.on<LogEntry>('log', (e) => setLog((l) => [e, ...l].slice(0, 200)))
    return () => {
      offStatus()
      offLog()
    }
  }, [refresh])

  if (!status) return <div className="page" />

  return (
    <div className="page">
      <header className="head">
        <div className="brand">
          <span className="logo">K</span>
          <h1>Key Manager</h1>
        </div>
        <button
          className={status.paused ? 'state paused' : 'state on'}
          onClick={() => void api.setPaused(!status.paused)}
          title={status.pauseHotkey ? `Переключить: ${status.pauseHotkey}` : undefined}
        >
          <span className="dot" />
          {status.paused ? 'Пауза — нажмите, чтобы продолжить' : `Работает · правил: ${status.activeCount}`}
        </button>
      </header>

      <section className="card">
        <div className="row">
          <div className="grow">
            <div className="label">Файл настроек</div>
            <div className="path">{status.configPath}</div>
          </div>
          <button onClick={() => void api.openConfig()}>Открыть</button>
          <button onClick={() => void api.openConfigFolder()}>Папка</button>
          <button onClick={() => void api.reload()}>Перечитать</button>
        </div>
        <div className="hint">
          Сохраните файл — изменения применятся сами.
          {status.loadedAt && <> Загружено в {status.loadedAt}.</>}
        </div>
        {status.error && <div className="alert err">{status.error}</div>}
        {status.warnings.map((w) => (
          <div key={w} className="alert warn">{w}</div>
        ))}
      </section>

      <section className="card">
        <h2>Правила</h2>
        {status.rules.length === 0 && <div className="hint">В файле пока нет правил.</div>}
        <table className="rules">
          <tbody>
            {status.rules.map((r, i) => (
              <tr key={i} className={r.active ? '' : 'off'}>
                <td className="kbd-cell">{r.hotkey ? <Keys chord={r.hotkey} /> : <span className="hint">—</span>}</td>
                <td>
                  <div className="rname">{r.name}</div>
                  <div className="preview">{r.preview}</div>
                  {r.problem && <div className="problem">{r.problem}</div>}
                </td>
                <td className="scope">{r.scope}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </section>

      <section className="card">
        <h2>Журнал</h2>
        {log.length === 0 && <div className="hint">Нажмите Alt+1 в любом текстовом поле — здесь появится запись.</div>}
        <ul className="log">
          {log.map((e, i) => (
            <li key={i} className={e.kind}>
              <span className="time">{e.time}</span>
              <span>{e.text}</span>
            </li>
          ))}
        </ul>
      </section>
    </div>
  )
}

function Keys({ chord }: { chord: string }): JSX.Element {
  return (
    <span className="keys">
      {chord.split('+').map((k, i) => (
        <kbd key={i}>{k}</kbd>
      ))}
    </span>
  )
}
