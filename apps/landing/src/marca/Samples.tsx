/* Dibujos de ejemplo de la encuesta. Son propios (sin logos de otras marcas) y usan currentColor
   y var(--accent), así siguen el tema. El motivo común es una llama/mecha: el «shin» del nombre. */

type P = { size?: number }

const box = (size: number, body: React.ReactNode, label?: string) => (
  <svg width={size} height={size} viewBox="0 0 64 64" role={label ? 'img' : undefined} aria-label={label} aria-hidden={label ? undefined : true}>
    {body}
  </svg>
)

/* ---------- qué muestra el logo ---------- */
export function SymbolArt({ id, size = 56 }: { id: string } & P) {
  switch (id) {
    case 'kanji':
      return box(size, <text x="32" y="47" textAnchor="middle" fontSize="42" fontFamily="var(--kanji)" fontWeight="600" fill="currentColor">芯</text>)
    case 'mecha':
      return box(size, <>
        <path d="M32 8c7 10 14 17 14 27a14 14 0 0 1-28 0c0-10 7-17 14-27Z" fill="var(--accent)" />
        <path d="M32 26c3 5 6 8 6 13a6 6 0 0 1-12 0c0-5 3-8 6-13Z" fill="var(--win, #1d2026)" />
        <rect x="31" y="46" width="2" height="10" rx="1" fill="currentColor" />
      </>)
    case 'monitor':
      return box(size, <>
        <rect x="8" y="12" width="48" height="30" rx="3" fill="none" stroke="currentColor" strokeWidth="3" />
        <path d="M12 38 26 26l8 7 6-5 12 10" fill="none" stroke="var(--accent)" strokeWidth="3" strokeLinejoin="round" />
        <path d="M28 42v8M20 52h24" stroke="currentColor" strokeWidth="3" strokeLinecap="round" />
      </>)
    case 'onda':
      return box(size, <>
        <path d="M6 30c6-10 12-10 18 0s12 10 18 0 12-10 16-4" fill="none" stroke="var(--accent)" strokeWidth="4" strokeLinecap="round" />
        <path d="M6 42c6-10 12-10 18 0s12 10 18 0 12-10 16-4" fill="none" stroke="currentColor" strokeWidth="3" strokeLinecap="round" opacity=".55" />
      </>)
    case 'abstracto':
      return box(size, <>
        <circle cx="26" cy="30" r="15" fill="var(--accent)" />
        <circle cx="38" cy="36" r="15" fill="none" stroke="currentColor" strokeWidth="3" />
      </>)
    case 'letras':
      return box(size, <text x="32" y="44" textAnchor="middle" fontSize="30" fontFamily="var(--head)" fontWeight="700" fill="currentColor" style={{ fontStretch: '75%' }}>MS</text>)
    default:
      return null
  }
}

/* ---------- tipo de logo ---------- */
export function LogoTypeArt({ id, brand }: { id: string; brand: string }) {
  const mark = (x: number, y: number, s: number) => (
    <path transform={`translate(${x} ${y}) scale(${s / 64})`} d="M32 8c7 10 14 17 14 27a14 14 0 0 1-28 0c0-10 7-17 14-27Z" fill="var(--accent)" />
  )
  const word = (x: number, anchor: 'start' | 'middle', size: number) => (
    <text x={x} y="45" textAnchor={anchor} fontSize={size} fontFamily="var(--head)" fontWeight="700" fill="currentColor" style={{ fontStretch: '78%' }}>{brand}</text>
  )
  return (
    <svg viewBox="0 0 160 64" width="100%" height="64" aria-hidden="true">
      {id === 'simbolo' && mark(52, 4, 56)}
      {id === 'simbolo_nombre' && <>{mark(6, 8, 48)}{word(52, 'start', 22)}</>}
      {id === 'nombre' && word(80, 'middle', 28)}
      {id === 'sello' && <>
        <rect x="56" y="8" width="48" height="48" rx="6" fill="#c8352e" />
        <text x="80" y="45" textAnchor="middle" fontSize="32" fontFamily="var(--kanji)" fontWeight="600" fill="#fff">芯</text>
      </>}
    </svg>
  )
}

/* ---------- pares de estilos: [A, B] ---------- */
const flame = 'M32 8c7 10 14 17 14 27a14 14 0 0 1-28 0c0-10 7-17 14-27Z'
export function StyleArt({ pair, side, size = 88 }: { pair: string; side: 'a' | 'b' } & P) {
  const a = side === 'a'
  switch (pair) {
    case 'geometrico_pincel':
      return a
        ? box(size, <><circle cx="32" cy="41" r="13" fill="var(--accent)" /><path d="M32 6 43 25H21Z" fill="var(--accent)" /><path d="M14 58h36" stroke="currentColor" strokeWidth="1" opacity=".35" /></>)
        : box(size, <g fill="none" stroke="var(--accent)" strokeLinecap="round" strokeLinejoin="round">
          <path d="M34 8c-6 9-15 17-15 28 0 9 6 15 13 15 8 0 13-6 13-14 0-8-6-12-8-19" strokeWidth="7" />
          <path d="M35 12c-3 6-9 11-10 19" strokeWidth="2.5" opacity=".55" />
          <path d="M30 51c5 1 10-1 12-5" strokeWidth="2" opacity=".5" />
        </g>)
    case 'fina_rellena':
      return a
        ? box(size, <path d={flame} fill="none" stroke="var(--accent)" strokeWidth="1.6" />)
        : box(size, <path d={flame} fill="var(--accent)" />)
    case 'plano_volumen':
      return a
        ? box(size, <path d={flame} fill="var(--accent)" />)
        : box(size, <>
          <defs>
            <radialGradient id="mk-vol" cx=".38" cy=".35" r=".75"><stop offset="0" stopColor="#fff" stopOpacity=".75" /><stop offset=".35" stopColor="var(--accent)" /><stop offset="1" stopColor="#0c2a24" /></radialGradient>
          </defs>
          <ellipse cx="32" cy="57" rx="12" ry="2.5" fill="currentColor" opacity=".2" />
          <path d={flame} fill="url(#mk-vol)" />
        </>)
    case 'redondo_anguloso':
      return a
        ? box(size, <rect x="14" y="14" width="36" height="36" rx="14" fill="var(--accent)" />)
        : box(size, <path d="M32 6 52 30 40 58H24L12 30Z" fill="var(--accent)" />)
    case 'simetrico_organico':
      return a
        ? box(size, <><path d={flame} fill="var(--accent)" /><path d="M32 6v52" stroke="currentColor" strokeWidth="1" strokeDasharray="2 3" opacity=".5" /></>)
        : box(size, <path d="M36 6c-4 9 12 14 10 28-1 11-9 17-17 16-9-1-13-9-10-18 2-6 7-7 9-13 2 4 2 7 1 10 5-5 8-13 7-23Z" fill="var(--accent)" />)
    default:
      return null
  }
}

/* ---------- tamaño chico ---------- */
export function SmallArt({ id, size }: { id: string; size: number }) {
  if (id === 'simple') return box(size, <path d={flame} fill="var(--accent)" />)
  if (id === 'medio') return box(size, <><path d={flame} fill="var(--accent)" /><path d="M32 28c3 5 6 8 6 12a6 6 0 0 1-12 0c0-4 3-7 6-12Z" fill="var(--win, #1d2026)" /></>)
  return box(size, <>
    <circle cx="32" cy="32" r="29" fill="none" stroke="currentColor" strokeWidth="2" />
    <path d={flame} fill="var(--accent)" />
    <path d="M32 28c3 5 6 8 6 12a6 6 0 0 1-12 0c0-4 3-7 6-12Z" fill="var(--win, #1d2026)" />
    <text x="32" y="60" textAnchor="middle" fontSize="7" fontFamily="var(--head)" fill="currentColor">MIRAI</text>
    <path d="M14 20h6M44 20h6M12 40h5M47 40h5" stroke="currentColor" strokeWidth="1.5" />
  </>)
}

/* Paletas: fondo, superficie, texto, acento */
export const PALETTES: Record<string, [string, string, string, string]> = {
  actual: ['#121419', '#1d2026', '#eceef2', '#4fd1b5'],
  hora_azul: ['#1a1e27', '#232836', '#eceef2', '#8f7ff2'],
  sello: ['#ece8e1', '#d9d3c7', '#1b1b1f', '#c8352e'],
  amanecer: ['#0f1a2b', '#1f3354', '#f5e9d7', '#f2a950'],
  mono: ['#0e0e10', '#2a2a2e', '#f4f4f5', '#ffffff'],
}

export function PaletteArt({ id }: { id: string }) {
  const [bg, surf, fg, acc] = PALETTES[id]
  return (
    <svg viewBox="0 0 160 64" width="100%" height="64" aria-hidden="true">
      <rect width="160" height="64" rx="6" fill={bg} />
      <rect x="10" y="10" width="62" height="44" rx="4" fill={surf} />
      <rect x="18" y="20" width="36" height="5" rx="2" fill={fg} />
      <rect x="18" y="31" width="26" height="4" rx="2" fill={fg} opacity=".55" />
      <rect x="18" y="41" width="22" height="7" rx="3" fill={acc} />
      <path transform="translate(96 6) scale(.81)" d={flame} fill={acc} />
    </svg>
  )
}
