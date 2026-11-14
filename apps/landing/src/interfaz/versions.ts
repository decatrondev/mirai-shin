import type { Theme } from '../theme'

/* Las 5 versiones de la app que compara la encuesta /interfaz. Tienen que coincidir con
   services/web/src/survey.rs (el servidor rechaza lo demás) y con la vista previa (/direcciones/app/#a … #e). */
export const VERSIONS = ['a', 'b', 'c', 'd', 'e'] as const
export type V = typeof VERSIONS[number]
export const MAX_CHANGE = 500, MAX_MIX = 300

/** Captura de la versión (en `public/versiones-app/`, una por tema). */
export const shot = (v: string, theme: Theme) => `/versiones-app/${v}-${theme}.webp`
/** La versión en vivo, en la vista previa. */
export const live = (v: string) => `/direcciones/app/#${v}`
