import { useEffect, useState, type JSX } from 'react'
import { api, errText } from '../lib/api'

// Версия программы и проверка обновлений вручную. Сама программа
// проверяет при запуске и раз в 6 часов — кнопка для нетерпеливых.
export function UpdatesCard(): JSX.Element {
  const [version, setVersion] = useState('')
  const [state, setState] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)

  useEffect(() => {
    void api.appVersion().then(setVersion)
  }, [])

  const check = (): void => {
    setBusy(true)
    setState(null)
    api
      .updateCheck()
      .then((u) => setState(u ? `Доступна версия ${u.version} — кнопка «Обновить» вверху окна.` : 'У вас последняя версия.'))
      .catch((e: unknown) => setState(errText(e)))
      .finally(() => setBusy(false))
  }

  return (
    <section className="card">
      <h2>Обновления</h2>
      <div className="set-row">
        <div>
          <div className="set-title">Версия {version}</div>
          <div className="hint">
            Отклик сам проверяет обновления при запуске и раз в 6 часов и спрашивает, прежде чем ставить. Каждое
            обновление подписано — чужой файл программа не поставит.
          </div>
          {state && <div className="hint">{state}</div>}
        </div>
        <button onClick={check} disabled={busy}>{busy ? 'Проверяю…' : 'Проверить обновления'}</button>
      </div>
    </section>
  )
}
