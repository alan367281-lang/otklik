import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import './styles/theme.css'
import './styles/search.css'
// Не «./Search»: в Windows имена файлов без учёта регистра, и Search.tsx
// совпал бы с этим файлом (search.tsx).
import { Search } from './SearchApp'

createRoot(document.getElementById('root') as HTMLElement).render(
  <StrictMode>
    <Search />
  </StrictMode>,
)
