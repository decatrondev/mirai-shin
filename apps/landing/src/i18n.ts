import i18n from 'i18next'
import { initReactI18next } from 'react-i18next'
import { LANGS, admin, landing, type Lang } from '@mirai-shin/i18n'
import brand from '@brand'

function initial(): Lang {
  try {
    const saved = localStorage.getItem('lang')
    if (saved && saved in LANGS) return saved as Lang
  } catch { /* el almacenamiento puede no estar disponible */ }
  return navigator.language.startsWith('en') ? 'en' : 'es'
}

i18n.use(initReactI18next).init({
  // `translation` = la landing; `admin` = el panel.
  resources: {
    es: { translation: landing.es, admin: admin.es },
    en: { translation: landing.en, admin: admin.en },
  },
  lng: initial(),
  fallbackLng: 'es',
  interpolation: {
    escapeValue: false,
    // El nombre está disponible en todos los textos como {{brand}}.
    defaultVariables: { brand: brand.name },
  },
})

export function setLang(l: Lang) {
  i18n.changeLanguage(l)
  try { localStorage.setItem('lang', l) } catch { /* ignorar */ }
}

export default i18n
