import { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { LANGS, type Lang } from '@mirai-shin/i18n'
import type { Theme } from '../theme'

const SPACES = [
  { href: '#inicio', key: 'home' },
  { href: '#por-que', key: 'why' },
  { href: '#monitores', key: 'monitors' },
  { href: '#etapa', key: 'stage' },
] as const

type Props = {
  desktop: boolean
  onToggleDesktop: () => void
  gap: number
  onGap: (v: number) => void
  subscribeFps: (fn: (fps: number) => void) => () => void
  theme: Theme
  onToggleTheme: () => void
  onToggleLang: () => void
}

/** Barra de tareas: mostrar escritorio, secciones como escritorios numerados y la bandeja. */
export function Taskbar({ desktop, onToggleDesktop, gap, onGap, subscribeFps, theme, onToggleTheme, onToggleLang }: Props) {
  const { t, i18n } = useTranslation()
  const other: Lang = i18n.resolvedLanguage === 'en' ? 'es' : 'en'
  const current = useCurrentSpace()
  const [fps, setFps] = useState(0)
  useEffect(() => subscribeFps(setFps), [subscribeFps])
  const clock = useClock(i18n.resolvedLanguage ?? 'es')
  const themeLabel = t(theme === 'dark' ? 'taskbar.toLight' : 'taskbar.toDark')

  return (
    <nav className="taskbar" aria-label={t('taskbar.label')}>
      <button className="ms-chip" type="button" aria-pressed={desktop} onClick={onToggleDesktop}>
        <span className="long">{t(desktop ? 'taskbar.hideDesk' : 'taskbar.showDesk')}</span>
        <span className="short">{t(desktop ? 'taskbar.hideDeskShort' : 'taskbar.showDeskShort')}</span>
      </button>
      <ul className="ws">
        {SPACES.map((s, i) => (
          <li key={s.key}>
            <a href={s.href} aria-current={current === i}><b>{i + 1}</b><span>{t(`taskbar.${s.key}`)}</span></a>
          </li>
        ))}
      </ul>
      <div className="tray">
        <label className="gap">
          <span>{t('taskbar.gap')}</span>
          <input type="range" min={4} max={56} step={1} value={gap} onChange={e => onGap(+e.target.value)} />
        </label>
        <span className="fps" title={fps ? undefined : t('taskbar.paused')}><b>{fps}</b> <span className="unit">{t('taskbar.fps')}</span></span>
        <span className="clock">{clock}</span>
        <button className="ms-chip" type="button" lang={other} aria-label={LANGS[other]} onClick={onToggleLang}>
          <span className="long">{LANGS[other]}</span>
          <span className="short">{other.toUpperCase()}</span>
        </button>
        <button className="ms-chip" type="button" aria-label={themeLabel} title={themeLabel} onClick={onToggleTheme}>
          <span className="long">{themeLabel}</span>
          <svg className="short" aria-hidden="true" width="16" height="16" viewBox="0 0 16 16">
            <circle cx="8" cy="8" r="6.5" fill="none" stroke="currentColor" strokeWidth="1.5" />
            <path d="M8 1.5a6.5 6.5 0 0 1 0 13z" fill="currentColor" />
          </svg>
        </button>
      </div>
    </nav>
  )
}

/** Escritorio activo según la sección visible; al final de la página, el último. */
function useCurrentSpace() {
  const [current, setCurrent] = useState(0)
  useEffect(() => {
    const mark = () => {
      const y = innerHeight * .4
      let cur = 0
      SPACES.forEach((s, i) => {
        const el = document.querySelector(s.href)
        if (el && el.getBoundingClientRect().top <= y) cur = i
      })
      if (innerHeight + scrollY >= document.documentElement.scrollHeight - 4) cur = SPACES.length - 1
      setCurrent(cur)
    }
    mark()
    addEventListener('scroll', mark, { passive: true })
    return () => removeEventListener('scroll', mark)
  }, [])
  return current
}

function useClock(lang: string) {
  const fmt = () => new Date().toLocaleTimeString(lang, { hour: '2-digit', minute: '2-digit' })
  const [now, setNow] = useState(fmt)
  useEffect(() => {
    setNow(fmt())
    const id = setInterval(() => setNow(fmt()), 15000)
    return () => clearInterval(id)
  }, [lang])
  return now
}
