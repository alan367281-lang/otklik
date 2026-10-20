import { useCallback, useEffect, useRef, useState, type JSX } from 'react'
import { api, errText, type Check, type LogEntry, type Status } from './lib/api'
import { sameConfig, withDefaults, type Config } from './lib/config'
import { SnippetsPage } from './pages/Snippets'
import { AppsPage } from './pages/Apps'
import { GeneralPage } from './pages/General'
import { JournalPage } from './pages/Journal'

type Tab = 'snippets' | 'apps' | 'general' | 'journal'

const TABS: { id: Tab; label: string }[] = [
  { id: 'snippets', label: 'Заготовки' },
  { id: 'apps', label: 'Приложения' },
  { id: 'general', label: 'Общие' },
  { id: 'journal', label: 'Журнал' },
]

export function App(): JSX.Element {
  const [tab, setTab] = useState<Tab>('snippets')
  const [status, setStatus] = useState<Status | null>(null)
  const [log, setLog] = useState<LogEntry[]>([])

  // saved — как в файле, draft — что правит человек. Сохраняем кнопкой.
  const [saved, setSaved] = useState<Config | null>(null)
  const [draft, setDraft] = useState<Config | null>(null)
  const [stamp, setStamp] = useState<number | null>(null)
  const [loadError, setLoadError] = useState<string | null>(null)
  const [saveError, setSaveError] = useState<string | null>(null)
  const [externalChange, setExternalChange] = useState(false)
  const [check, setCheck] = useState<Check>({ rules: [], warnings: [] })
  const [fatal, setFatal] = useState<string | null>(null)

  const dirty = !sameConfig(saved, draft)
  const dirtyRef = useRef(dirty)
  dirtyRef.current = dirty
  const stampRef = useRef(stamp)
  stampRef.current = stamp

  const loadConfig = useCallback((): void => {
    api.getConfig().catch((e: unknown) => { setFatal(`Не удалось прочитать настройки: ${errText(e)}`); return null }).then((doc) => {
      if (!doc) return
      setStamp(doc.stamp)
      setLoadError(doc.error)
      setExternalChange(false)
      setSaveError(null)
      if (doc.config) {
        const c = withDefaults(doc.config)
        setSaved(c)
        setDraft(c)
      }
    })
  }, [])

  const refreshStatus = useCallback((): void => {
    api
      .status()
      .then(setStatus)
      .catch((e: unknown) => setFatal(`Окно не может связаться с ядром: ${errText(e)}`))
  }, [])

  useEffect(() => {
    refreshStatus()
    loadConfig()
    void api.log().then(setLog)
    // Ядро перечитало файл (правка руками или наше сохранение).
    const offStatus = api.on('status', () => {
      refreshStatus()
      if (!dirtyRef.current) {
        loadConfig()
        return
      }
      void api.getConfig().then((doc) => {
        if (doc.stamp !== stampRef.current) setExternalChange(true)
      })
    })
    const offLog = api.on<LogEntry>('log', (e) => setLog((l) => [e, ...l].slice(0, 200)))
    return () => {
      offStatus()
      offLog()
    }
  }, [refreshStatus, loadConfig])

  // Проверка черновика в ядре — те же правила, что при загрузке файла.
  useEffect(() => {
    if (!draft) return
    const t = window.setTimeout(() => {
      void api.checkConfig(draft).then(setCheck)
    }, 200)
    return () => window.clearTimeout(t)
  }, [draft])

  const update = useCallback((fn: (c: Config) => void): void => {
    setDraft((d) => {
      if (!d) return d
      const next = structuredClone(d)
      fn(next)
      return next
    })
  }, [])

  const save = useCallback((): void => {
    if (!draft || !dirty) return
    setSaveError(null)
    api
      .saveConfig(draft, stamp)
      .then((s) => {
        setStamp(s)
        setSaved(draft)
        setExternalChange(false)
      })
      .catch((e: unknown) => setSaveError(errText(e)))
  }, [draft, dirty, stamp])

  useEffect(() => {
    const onKey = (e: KeyboardEvent): void => {
      if (e.ctrlKey && !e.altKey && (e.key === 's' || e.key === 'ы' || e.code === 'KeyS')) {
        e.preventDefault()
        save()
      }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [save])

  if (fatal) return <div className="shell"><div className="banner err">{fatal}</div></div>
  if (!status) return <div className="shell"><div className="empty big">Загрузка…</div></div>

  return (
    <div className="shell">
      <header className="topbar">
        <div className="brand">
          <span className="logo">О</span>
          <span className="brand-name">Отклик</span>
        </div>
        <nav className="tabs">
          {TABS.map((t) => (
            <button key={t.id} className={'tab' + (t.id === tab ? ' on' : '')} onClick={() => setTab(t.id)}>
              {t.label}
            </button>
          ))}
        </nav>
        <button
          className={status.paused ? 'state paused' : 'state on'}
          onClick={() => void api.setPaused(!status.paused)}
          title={status.pauseHotkey ? `Переключить: ${status.pauseHotkey}` : undefined}
        >
          <span className="dot" />
          {status.paused ? 'Пауза' : `Работает · ${status.activeCount}`}
        </button>
      </header>

      {(loadError || status.error) && (
        <div className="banner err">
          <div className="grow">
            <b>В файле настроек ошибка.</b> Программа работает на последней правильной версии.
            <div className="mono">{loadError ?? status.error}</div>
          </div>
          <button onClick={() => void api.openConfig()}>Открыть файл</button>
          <button onClick={loadConfig}>Перечитать</button>
        </div>
      )}
      {externalChange && (
        <div className="banner warn">
          <div className="grow">Файл настроек изменили снаружи, пока здесь были несохранённые правки.</div>
          <button onClick={loadConfig}>Загрузить из файла (мои правки пропадут)</button>
        </div>
      )}
      {check.warnings.map((w) => (
        <div key={w} className="banner warn">{w}</div>
      ))}

      <main className="content">
        {draft ? (
          <>
            {tab === 'snippets' && <SnippetsPage config={draft} views={check.rules} update={update} />}
            {tab === 'apps' && <AppsPage config={draft} update={update} />}
            {tab === 'general' && <GeneralPage config={draft} status={status} update={update} />}
            {tab === 'journal' && <JournalPage log={log} />}
          </>
        ) : tab === 'journal' ? (
          <JournalPage log={log} />
        ) : (
          <div className="empty big">Исправьте файл настроек — после сохранения он откроется здесь.</div>
        )}
      </main>

      {(dirty || saveError) && (
        <footer className="savebar">
          <span className="grow">{saveError ? <span className="err-text">{saveError}</span> : 'Есть несохранённые изменения'}</span>
          <button onClick={() => { setDraft(saved); setSaveError(null) }}>Отменить</button>
          <button className="primary" onClick={save} disabled={!dirty}>Сохранить <span className="muted-inv">Ctrl+S</span></button>
        </footer>
      )}
    </div>
  )
}
