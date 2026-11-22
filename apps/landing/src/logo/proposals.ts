/* Las propuestas de logo y las paletas de la ronda 2 (/marca). Tienen que coincidir con
   services/web/src/survey.rs (PROPOSALS y LOGO_PALETTES: el servidor rechaza lo demás). */
export const PROPOSALS = ['sello', 'enso', 'mecha', 'kitsune'] as const
export type Proposal = typeof PROPOSALS[number]

export const LOGO_PALETTES = ['actual', 'hora_azul', 'sello'] as const
export type LogoPalette = typeof LOGO_PALETTES[number]

/** Colores de cada paleta: fondo, tinta del kanji, acento y color sobre el acento.
 *  Son los mismos de la ronda 1 (marca/Samples.tsx). */
export interface PaletteColors { bg: string; ink: string; acc: string; onAcc: string }
export const PALETTE_COLORS: Record<LogoPalette, PaletteColors> = {
  actual: { bg: '#121419', ink: '#eceef2', acc: '#4fd1b5', onAcc: '#0c2a24' },
  hora_azul: { bg: '#1a1e27', ink: '#eceef2', acc: '#8f7ff2', onAcc: '#14121f' },
  sello: { bg: '#ece8e1', ink: '#1b1b1f', acc: '#c8352e', onAcc: '#ece8e1' },
}

export const MAX_COMMENT = 1000

/** La lámina completa con todas las propuestas, paletas y tamaños. */
export const SHEET = '/direcciones/logo/'
