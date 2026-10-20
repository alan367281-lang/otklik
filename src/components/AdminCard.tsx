import { useCallback, useEffect, useState, type JSX } from 'react'
import { api, errText, type AdminStatus } from '../lib/api'

// Права администратора: без них Отклик не может печатать в окна,
// запущенные от администратора (так устроена защита Windows).
export function AdminCard(): JSX.Element {
  const [st, setSt] = useState<AdminStatus | null>(null)
  const [busy, setBusy] = useState(false)
  const [err, setErr] = useState<string | null>(null)

  const refresh = useCallback((): void => {
    void api.adminStatus().then(setSt)
  }, [])

  useEffect(refresh, [refresh])

  // Окно UAC и schtasks работают отдельно от нас — результат узнаём,
  // спросив ещё раз через пару секунд.
  const setTask = (on: boolean): void => {
    setErr(null)
    setBusy(true)
    api
      .adminSetTask(on)
      .then(() => window.setTimeout(() => { refresh(); setBusy(false) }, 2500))
      .catch((e: unknown) => {
        setErr(errText(e))
        setBusy(false)
      })
  }

  const restart = (): void => {
    setErr(null)
    api.adminRestart().catch((e: unknown) => setErr(errText(e)))
  }

  if (!st) return <section className="card"><h2>Права администратора</h2></section>

  return (
    <section className="card">
      <h2>Права администратора</h2>
      <div className="hint">
        Windows не даёт обычной программе печатать в окна, запущенные от имени администратора (диспетчер задач,
        часть установщиков и системных настроек). Если нужно, чтобы сочетания работали и там, — запускайте Отклик
        от администратора.
      </div>
      <div className="set-row">
        <div>
          <div className="set-title">Сейчас</div>
          <div className="hint">{st.elevated ? 'Работает с правами администратора' : 'Работает с обычными правами'}</div>
        </div>
        {!st.elevated && <button onClick={restart}>Перезапустить от администратора</button>}
      </div>
      <div className="set-row">
        <div>
          <div className="set-title">Запускать при входе от администратора</div>
          <div className="hint">
            Через Планировщик заданий — без окна подтверждения при каждом входе. Спросит подтверждение один раз,
            при включении. Обычный автозапуск при этом выключается.
          </div>
          {err && <div className="field-problem">{err}</div>}
        </div>
        <label className="toggle">
          <input type="checkbox" disabled={busy} checked={st.task} onChange={(e) => setTask(e.target.checked)} />
          <span>{busy ? 'Меняю…' : st.task ? 'Включено' : 'Выключено'}</span>
        </label>
      </div>
    </section>
  )
}
