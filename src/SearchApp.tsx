import { useCallback, useEffect, useMemo, useRef, useState, type JSX } from 'react'
import { api, errText, type SearchItem } from './lib/api'
import { score } from './lib/fuzzy'
import { Keys } from './components/Keys'

// Всплывающий поиск: живёт скрытым, ядро показывает его по сочетанию и
// присылает «search:open». Enter — выполнить в окне, откуда вызвали,
// Esc — закрыть и вернуть туда фокус.
export function Search(): JSX.Element {
  const [items, setItems] = useState<SearchItem[]>([])
  const [query, setQuery] = useState('')
  const [sel, setSel] = useState(0)
  const [error, setError] = useState<string | null>(null)
  const input = useRef<HTMLInputElement>(null)
  const list = useRef<HTMLDivElement>(null)

  const open = useCallback((): void => {
    setQuery('')
    setSel(0)
    setError(null)
    void api.searchList().then(setItems)
    // Окно только что показали — фокус в поле, чтобы сразу печатать.
    requestAnimationFrame(() => input.current?.focus())
  }, [])

  useEffect(() => {
    open()
    const off = api.on('search:open', open)
    // Щелчок в другое окно — просто закрыться: человек сам ушёл туда.
    const blur = (): void => void api.searchHide(false)
    window.addEventListener('blur', blur)
    return () => {
      off()
      window.removeEventListener('blur', blur)
    }
  }, [open])

  const found = useMemo(
    () =>
      items
        .map((it, i) => ({ it, s: score(query, it), i }))
        .filter((x) => x.s > 0)
        .sort((a, b) => b.s - a.s || a.i - b.i)
        .slice(0, 50)
        .map((x) => x.it),
    [items, query],
  )

  useEffect(() => {
    setSel(0)
  }, [query])

  useEffect(() => {
    list.current?.querySelector('.s-item.sel')?.scrollIntoView({ block: 'nearest' })
  }, [sel])

  const run = (it: SearchItem | undefined): void => {
    if (!it) return
    api.searchRun(it.index, it.name).catch((e: unknown) => setError(errText(e)))
  }

  const onKey = (e: React.KeyboardEvent): void => {
    if (e.key === 'ArrowDown') {
      e.preventDefault()
      setSel((s) => Math.min(s + 1, found.length - 1))
    } else if (e.key === 'ArrowUp') {
      e.preventDefault()
      setSel((s) => Math.max(s - 1, 0))
    } else if (e.key === 'Enter') {
      e.preventDefault()
      run(found[sel])
    } else if (e.key === 'Escape') {
      e.preventDefault()
      void api.searchHide(true)
    }
  }

  const current = found[sel]

  return (
    <div className="s-shell" onKeyDown={onKey}>
      <input
        ref={input}
        className="s-input"
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        placeholder="Найти заготовку…"
        spellCheck={false}
      />
      <div className="s-list" ref={list}>
        {items.length === 0 && <div className="s-empty">Для этого окна заготовок нет</div>}
        {items.length > 0 && found.length === 0 && <div className="s-empty">Ничего не найдено</div>}
        {found.map((it, i) => (
          <button
            key={it.index}
            className={'s-item' + (i === sel ? ' sel' : '')}
            onMouseMove={() => setSel(i)}
            onClick={() => run(it)}
          >
            <div className="s-top">
              <span className="s-name">{it.name}</span>
              {it.group && <span className="s-group">{it.group}</span>}
              <span className="spacer" />
              {it.abbr && <span className="abbr-tag">{it.abbr}</span>}
              {it.hotkey && <Keys chord={it.hotkey} />}
            </div>
            <div className="s-preview">{it.preview}</div>
          </button>
        ))}
      </div>
      {current && <div className="s-detail">{current.detail}</div>}
      <div className="s-foot">
        {error ? <span className="s-error">{error}</span> : <span>↑↓ выбрать · Enter — выполнить · Esc — закрыть</span>}
      </div>
    </div>
  )
}
