import { useEffect, useState, type JSX } from 'react'
import { api, errText, type UpdateInfo } from '../lib/api'

// Полоса «доступна новая версия». Обновление ставится только по кнопке —
// человек решает сам, когда программе перезапуститься.
export function UpdateBanner(): JSX.Element | null {
  const [update, setUpdate] = useState<UpdateInfo | null>(null)
  const [notes, setNotes] = useState(false)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    void api.updateStatus().then(setUpdate)
    return api.on<UpdateInfo>('update:available', setUpdate)
  }, [])

  if (!update) return null

  const install = (): void => {
    setBusy(true)
    setError(null)
    // При успехе программа перезапустится сама — ответа можно не дождаться.
    api.updateInstall().catch((e: unknown) => {
      setBusy(false)
      setError(errText(e))
    })
  }

  return (
    <div className="banner update">
      <div className="grow">
        <b>Доступна версия {update.version}</b> <span className="muted-inv">(у вас {update.current})</span>
        {notes && update.notes && <div className="update-notes">{update.notes}</div>}
        {error && <div className="err-text">{error}</div>}
      </div>
      {update.notes && <button onClick={() => setNotes(!notes)}>{notes ? 'Скрыть' : 'Что нового'}</button>}
      <button className="primary" onClick={install} disabled={busy}>
        {busy ? 'Скачиваю и ставлю…' : 'Обновить'}
      </button>
    </div>
  )
}
