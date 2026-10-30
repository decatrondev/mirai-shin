type Props = {
  options: { value: number; label: string }[]
  value: number
  onChange: (value: number) => void
  label?: string
}

/** Grupo de botones para elegir una opción, por ejemplo el fondo. */
export function Swatches({ options, value, onChange, label }: Props) {
  return (
    <span className="ms-swatches" role="group" aria-label={label}>
      {options.map(o => (
        <button key={o.value} type="button" aria-pressed={o.value === value} onClick={() => onChange(o.value)}>
          {o.label}
        </button>
      ))}
    </span>
  )
}
