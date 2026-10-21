import { useEffect, useState, type JSX } from 'react'
import { api, errText, type Status } from '../lib/api'
import type { Config } from '../lib/config'
import { HotkeyInput } from '../components/HotkeyInput'
import { AdminCard } from '../components/AdminCard'
import { PermissionsCard } from '../components/PermissionsCard'
import { isMac } from '../lib/platform'
import { EspansoImport } from '../components/EspansoImport'
import { UpdatesCard } from '../components/UpdatesCard'

interface Props {
  config: Config
  status: Status
  update: (fn: (c: Config) => void) => void
}

export function GeneralPage({ config, status, update }: Props): JSX.Element {
  const st = config.settings
  const [autostart, setAutostart] = useState<boolean | null>(null)
  const [autoErr, setAutoErr] = useState<string | null>(null)

  useEffect(() => {
    void api.getAutostart().then(setAutostart)
  }, [])

  const toggleAutostart = (on: boolean): void => {
    setAutoErr(null)
    api
      .setAutostart(on)
      .then(() => setAutostart(on))
      .catch((e: unknown) => setAutoErr(errText(e)))
  }

  const num = (v: string, min: number, max: number): number => Math.min(max, Math.max(min, Math.round(Number(v) || 0)))

  return (
    <div className="page-pad">
      <section className="card">
        <h2>Сочетания программы</h2>
        <div className="set-row">
          <div>
            <div className="set-title">Пауза</div>
            <div className="hint">Включает и выключает все сочетания. То же — из меню в трее.</div>
          </div>
          <HotkeyInput value={st.pause_hotkey || null} clearable={false} onChange={(v) => update((c) => void (c.settings.pause_hotkey = v ?? ''))} />
        </div>
        <div className="set-row">
          <div>
            <div className="set-title">Поиск заготовок</div>
            <div className="hint">Всплывающее окно поиска — появится на одном из следующих этапов.</div>
          </div>
          <HotkeyInput value={st.search_hotkey || null} clearable={false} onChange={(v) => update((c) => void (c.settings.search_hotkey = v ?? ''))} />
        </div>
      </section>

      <section className="card">
        <h2>Вставка текста</h2>
        <div className="set-row">
          <div>
            <div className="set-title">Через буфер — если длиннее</div>
            <div className="hint">Для правил «Сам решит»: текст длиннее этого или с переносами строк вставляется через буфер обмена.</div>
          </div>
          <div className="num">
            <input type="number" min={1} max={10000} value={st.paste_threshold} onChange={(e) => update((c) => void (c.settings.paste_threshold = num(e.target.value, 1, 10000)))} />
            <span className="muted">симв.</span>
          </div>
        </div>
        <div className="set-row">
          <div>
            <div className="set-title">Вернуть буфер через</div>
            <div className="hint">Сколько ждать, прежде чем вернуть в буфер обмена прежнее содержимое. Если в какой-то программе вставляется старый текст — увеличьте.</div>
          </div>
          <div className="num">
            <input type="number" min={50} max={5000} step={50} value={st.restore_clipboard_ms} onChange={(e) => update((c) => void (c.settings.restore_clipboard_ms = num(e.target.value, 50, 5000)))} />
            <span className="muted">мс</span>
          </div>
        </div>
      </section>

      <section className="card">
        <h2>Запуск</h2>
        <div className="set-row">
          <div>
            <div className="set-title">Запускать при входе в систему</div>
            <div className="hint">Применяется сразу, без кнопки «Сохранить».</div>
            {autoErr && <div className="field-problem">{autoErr}</div>}
          </div>
          <label className="toggle">
            <input type="checkbox" disabled={autostart === null} checked={autostart ?? false} onChange={(e) => toggleAutostart(e.target.checked)} />
            <span>{autostart ? 'Включено' : 'Выключено'}</span>
          </label>
        </div>
      </section>

      {isMac ? <PermissionsCard /> : <AdminCard />}

      <UpdatesCard />

      <EspansoImport config={config} update={update} />

      <section className="card">
        <h2>Файл настроек</h2>
        <div className="path">{status.configPath}</div>
        <div className="hint">Всё, что есть в этом окне, хранится в одном файле. Его можно править и руками — изменения подхватятся сами. Перед каждым сохранением из окна рядом, в папке backups, остаётся копия (последние 20).</div>
        <div className="row gap-top">
          <button onClick={() => void api.openConfig()}>Открыть файл</button>
          <button onClick={() => void api.openConfigFolder()}>Показать в папке</button>
        </div>
      </section>
    </div>
  )
}
