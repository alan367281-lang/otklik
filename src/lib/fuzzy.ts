// Нечёткий поиск для всплывающего окна: чем «естественнее» совпадение,
// тем выше. Название важнее текста, начало слова — важнее середины.

export interface Searchable {
  name: string
  abbr: string | null
  group: string | null
  detail: string
}

function words(s: string): string[] {
  return s.toLowerCase().split(/[\s\-_.,:;!?()«»"'/\\]+/).filter(Boolean)
}

/** Все ли буквы запроса идут в строке по порядку («тст» → «тест»). */
function subsequence(q: string, s: string): boolean {
  let i = 0
  for (const c of s) if (c === q[i]) i++
  return i === q.length
}

/** 0 — не подходит; больше — лучше. */
export function score(query: string, it: Searchable): number {
  const q = query.trim().toLowerCase()
  if (!q) return 1
  const name = it.name.toLowerCase()
  const abbr = (it.abbr ?? '').toLowerCase()
  const detail = it.detail.toLowerCase()
  if (abbr && abbr === q) return 1000
  if (name === q) return 950
  if (name.startsWith(q)) return 900
  if (words(name).some((w) => w.startsWith(q))) return 800
  if (name.includes(q)) return 700
  if (abbr && abbr.includes(q)) return 650
  // Несколько слов: каждое где-то есть — в названии или тексте.
  const parts = q.split(/\s+/).filter(Boolean)
  if (parts.length > 1 && parts.every((p) => name.includes(p) || detail.includes(p))) return 600
  if ((it.group ?? '').toLowerCase().includes(q)) return 500
  if (detail.includes(q)) return 400
  if (q.length >= 2 && subsequence(q, name)) return 300
  return 0
}
