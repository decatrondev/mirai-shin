import { ENSO, FLAME, FLAME_CORE, FOX, FOX_EAR_L, FOX_EAR_R, FOX_EYE, FOX_MUZZLE, FOX_NOSE, KANJI_PARTS, KANJI_TRANSFORM, SEAL } from './marks'
import { PALETTE_COLORS, type LogoPalette, type Proposal } from './proposals'

/** El 芯 movido a (x, y) y escalado. `noDot` quita el punto central del 心 (lo reemplaza la llama). */
function Kanji({ fill, x, y, s, noDot }: { fill: string; x: number; y: number; s: number; noDot?: boolean }) {
  return (
    <g transform={`translate(${x} ${y}) scale(${s})`}>
      <g fill={fill} transform={KANJI_TRANSFORM}>
        {KANJI_PARTS.map((d, i) => (noDot && i === 4 ? null : <path key={i} d={d} />))}
      </g>
    </g>
  )
}

/** La llama en las coordenadas del kanji; `x`, `y`, `s` la mueven y escalan. */
function Flame({ fill, core, transform }: { fill: string; core: string; transform?: string }) {
  return <g transform={transform}><path fill={fill} d={FLAME} /><path fill={core} d={FLAME_CORE} /></g>
}

/** La llama sola, centrada en el lienzo y agrandada `s` veces (versión para tamaño chico). */
const centered = (s: number) => `translate(${500 - 435 * s} ${500 - 487 * s}) scale(${s})`

/** Una propuesta de logo en una paleta. `small` = versión simplificada para 32 px o menos (la llama en lugar del kanji). */
export function Mark({ kind, palette, size, small, title }: { kind: Proposal; palette: LogoPalette; size: number | string; small?: boolean; title?: string }) {
  const { bg, ink, acc, onAcc } = PALETTE_COLORS[palette]
  let body
  switch (kind) {
    case 'sello':
      body = <><path fill={acc} d={SEAL} />{small ? <Flame fill={onAcc} core={acc} transform={centered(2.3)} /> : <Kanji fill={onAcc} x={190} y={176} s={0.78} />}</>
      break
    case 'enso':
      body = <><path fill={acc} d={ENSO} />{small ? <Flame fill={ink} core={bg} transform={centered(1.7)} /> : <Kanji fill={ink} x={235} y={225} s={0.67} />}</>
      break
    case 'mecha':
      body = small
        ? <Flame fill={acc} core={bg} transform={centered(3.2)} />
        : <><Kanji fill={ink} x={60} y={40} s={1.1} noDot /><Flame fill={acc} core={bg} transform="translate(60 40) scale(1.1)" /></>
      break
    case 'kitsune':
      body = (
        <>
          <path fill={acc} d={FOX} />
          <path fill={bg} opacity={0.35} d={FOX_EAR_L} />
          <path fill={bg} opacity={0.35} d={FOX_EAR_R} />
          <path fill="#f5f2ec" d={FOX_MUZZLE} />
          <path fill="#1b1b1f" d={FOX_NOSE} />
          <path fill={bg} d={FOX_EYE} />
          <path fill={bg} d={FOX_EYE} transform="translate(1000 0) scale(-1 1)" />
          {!small && <Kanji fill={bg} x={412} y={360} s={0.22} />}
        </>
      )
      break
  }
  return (
    <svg width={size} height={size} viewBox="0 0 1000 1000" role={title ? 'img' : undefined} aria-label={title} aria-hidden={title ? undefined : true}>
      {body}
    </svg>
  )
}
