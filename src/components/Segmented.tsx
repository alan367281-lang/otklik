import type { JSX } from 'react'

interface Props<T extends string> {
  value: T
  options: { value: T; label: string }[]
  onChange: (v: T) => void
}

export function Segmented<T extends string>({ value, options, onChange }: Props<T>): JSX.Element {
  return (
    <div className="seg">
      {options.map((o) => (
        <button key={o.value} className={o.value === value ? 'on' : ''} onClick={() => onChange(o.value)}>
          {o.label}
        </button>
      ))}
    </div>
  )
}
