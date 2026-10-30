import { useId } from 'react'

type Props = {
  label: string
  value: number
  min: number
  max: number
  step: number
  onChange: (value: number) => void
}

/** Control deslizante con su etiqueta, como velocidad o brillo. */
export function RangeField({ label, value, min, max, step, onChange }: Props) {
  const id = useId()
  return (
    <label className="ms-field" htmlFor={id}>
      <span>{label}</span>
      <input id={id} type="range" min={min} max={max} step={step} value={value} onChange={e => onChange(+e.target.value)} />
    </label>
  )
}
