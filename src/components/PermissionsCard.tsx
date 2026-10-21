import { useEffect, useState, type JSX } from 'react'
import { api } from '../lib/api'

// macOS: перехват клавиш и эмуляция нажатий работают только с разрешением
// «Универсальный доступ». Без него программа молчит — показываем это прямо
// и даём кнопку в нужный раздел Настроек. Статус перепроверяем сами:
// человек выдаёт разрешение в Настройках, а не здесь.
export function PermissionsCard(): JSX.Element {
  const [trusted, setTrusted] = useState<boolean | null>(null)

  useEffect(() => {
    const check = (): void => void api.platformInfo().then((p) => setTrusted(p.trusted))
    check()
    const t = window.setInterval(check, 3000)
    return () => window.clearInterval(t)
  }, [])

  return (
    <section className="card">
      <h2>Разрешения macOS</h2>
      <div className="hint">
        Чтобы ловить сочетания и сокращения и печатать в другие программы, macOS требует разрешение
        «Универсальный доступ». Без него Отклик ничего не делает.
      </div>
      <div className="set-row">
        <div>
          <div className="set-title">Универсальный доступ</div>
          <div className="hint">
            {trusted === null
              ? 'Проверяю…'
              : trusted
                ? 'Разрешено — всё работает.'
                : 'Не разрешено. Откройте Настройки, включите Отклик в списке — заработает сразу, без перезапуска.'}
          </div>
        </div>
        {trusted === false && <button className="primary" onClick={() => void api.openPermissions()}>Открыть Настройки</button>}
      </div>
    </section>
  )
}
