import landingEs from '../locales/es/landing.json'
import landingEn from '../locales/en/landing.json'
import adminEs from '../locales/es/admin.json'
import adminEn from '../locales/en/admin.json'

// Hoy salen es y en; sumar un idioma es una carpeta más en locales/ y una línea aquí.
export const LANGS = { es: 'Español', en: 'English' } as const
export type Lang = keyof typeof LANGS

export const landing = { es: landingEs, en: landingEn } satisfies Record<Lang, unknown>
export const admin = { es: adminEs, en: adminEn } satisfies Record<Lang, unknown>
