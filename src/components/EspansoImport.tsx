import { useState, type JSX } from 'react'
import { api, errText, type Imported } from '../lib/api'
import { withDefaults, type Config } from '../lib/config'

interface Props {
  config: Config
  update: (fn: (c: Config) => void) => void
}

// Перенос заготовок из Espanso: сначала показываем, что нашлось и что не
// переносится, добавляем — по кнопке, в черновик (сохранить — как обычно).
export function EspansoImport({ config, update }: Props): JSX.Element {
  const [found, setFound] = useState<Imported | null>(null)
  const [err, setErr] = useState<string | null>(null)
  const [added, setAdded] = useState<number | null>(null)

  const scan = (): void => {
    setErr(null)
    setAdded(null)
    api
      .importEspanso()
      .then(setFound)
      .catch((e: unknown) => {
        setFound(null)
        setErr(errText(e))
      })
  }

  // Сокращения, которые уже есть, второй раз не добавляем.
  const have = new Set(config.snippets.map((s) => s.abbr).filter(Boolean))
  const fresh = found
    ? withDefaults({ ...config, snippets: found.snippets }).snippets.filter((s) => !have.has(s.abbr))
    : []

  const add = (): void => {
    update((c) => {
      c.snippets.push(...fresh)
    })
    setAdded(fresh.length)
    setFound(null)
  }

  return (
    <section className="card">
      <h2>Импорт из Espanso</h2>
      <div className="hint">
        Переносит текст и сокращения, «целым словом», курсор $|$, переменные даты и буфера обмена. Формы, регулярные
        выражения, скрипты и картинки не переносятся — список пропущенного покажем.
      </div>
      <div className="row gap-top">
        <button onClick={scan}>Найти заготовки Espanso</button>
        {added !== null && <span className="muted">Добавлено: {added}. Не забудьте сохранить (Ctrl+S).</span>}
      </div>
      {err && <div className="field-problem">{err}</div>}
      {found && (
        <div className="import-box">
          <div>
            В {found.files} файл(ах) найдено заготовок: <b>{found.snippets.length}</b>
            {found.snippets.length !== fresh.length && <>, из них уже есть у вас: {found.snippets.length - fresh.length}</>}.
          </div>
          {found.skipped.length > 0 && (
            <details>
              <summary>Не переносится: {found.skipped.length}</summary>
              <ul className="skip-list">
                {found.skipped.map((s, i) => (
                  <li key={i}>{s}</li>
                ))}
              </ul>
            </details>
          )}
          <div className="row gap-top">
            <button className="primary" disabled={fresh.length === 0} onClick={add}>
              Добавить {fresh.length} в группу «Espanso»
            </button>
            <button onClick={() => setFound(null)}>Отмена</button>
          </div>
        </div>
      )}
    </section>
  )
}
