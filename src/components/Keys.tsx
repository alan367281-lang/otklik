import type { JSX } from 'react'

export function Keys({ chord }: { chord: string }): JSX.Element {
  return (
    <span className="keys">
      {chord.split('+').map((k, i) => (
        <kbd key={i}>{k}</kbd>
      ))}
    </span>
  )
}
