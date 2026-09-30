import type { JSX } from 'react'
import type { LogEntry } from '../lib/api'

export function JournalPage({ log }: { log: LogEntry[] }): JSX.Element {
  return (
    <div className="page-pad">
      <section className="card">
        <h2>Журнал</h2>
        <div className="hint">Что сработало, куда вставлено и что пошло не так. Набранный вами текст сюда не записывается.</div>
        {log.length === 0 && <div className="empty">Пока пусто. Нажмите Alt+1 в любом текстовом поле.</div>}
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
