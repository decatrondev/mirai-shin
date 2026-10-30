// Tres fondos de muestra. «Tinta» sale de los tokens del tema, así cambia con oscuro y claro.
export const PALETTES = [
  { key: 'ink', colors: () => ['--w0', '--w1', '--w2', '--w3'].map(readVar) },
  { key: 'ember', colors: () => ['#140c0a', '#5a1f14', '#d06a2c', '#f4cf8a'] },
  { key: 'frost', colors: () => ['#0b1220', '#2c4a6e', '#9cc4e0', '#f2f7fb'] },
] as const

export type Rgb = [number, number, number]

function readVar(name: string) {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim()
}

export function hexToRgb(hex: string): Rgb {
  const n = parseInt(hex.replace('#', ''), 16)
  return [(n >> 16 & 255) / 255, (n >> 8 & 255) / 255, (n & 255) / 255]
}

/** Luminancia media de los tres primeros colores: decide si el sello va en tinta clara u oscura. */
export function paletteLuminance(colors: string[]) {
  const rgb = colors.slice(0, 3).map(hexToRgb)
  return rgb.reduce((a, c) => a + .2126 * c[0] + .7152 * c[1] + .0722 * c[2], 0) / rgb.length
}
