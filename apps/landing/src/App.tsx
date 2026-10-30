import { useCallback, useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import type { Lang } from '@mirai-shin/i18n'
import { setLang } from './i18n'
import { applyTheme, currentTheme, type Theme } from './theme'
import { LiveWallpaper } from './wallpaper/live'
import { PALETTES, paletteLuminance } from './wallpaper/palettes'
import { Hero, type WallControls } from './sections/Hero'
import { Why } from './sections/Why'
import { Monitors } from './sections/Monitors'
import { Stage } from './sections/Stage'
import { Footer } from './sections/Footer'
import { Taskbar } from './sections/Taskbar'
import { FocusProvider } from './focus'

export function App() {
  const { t, i18n } = useTranslation()
  const lang = (i18n.resolvedLanguage ?? 'es') as Lang
  const [theme, setTheme] = useState<Theme>(currentTheme)
  const [controls, setControls] = useState<WallControls>({ palette: 0, speed: 1, brightness: 1, paused: false })
  const [glState, setGlState] = useState<'ok' | 'noGl' | 'reduced'>('ok')
  const fpsListeners = useRef(new Set<(fps: number) => void>())
  const [sealInk, setSealInk] = useState<'light' | 'dark'>('light')
  const [desktop, setDesktop] = useState(false)
  const [gap, setGap] = useState(22)
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const wallRef = useRef<LiveWallpaper | null>(null)

  // El fondo vivo de toda la página.
  useEffect(() => {
    const w = new LiveWallpaper(canvasRef.current!, { reveal: true })
    wallRef.current = w
    setGlState(!w.ok ? 'noGl' : w.reducedMotion ? 'reduced' : 'ok')
    // Solo la barra escucha los cuadros/s, a lo sumo dos veces por segundo: la página no se re-renderiza.
    let lastShown = 0
    w.onFrame = wall => {
      const now = performance.now()
      if (wall.fps === 0 || now - lastShown > 500) {
        lastShown = now
        fpsListeners.current.forEach(fn => fn(Math.round(wall.fps)))
      }
    }
    return () => { w.destroy(); wallRef.current = null }
  }, [])

  useEffect(() => {
    const w = wallRef.current
    if (!w) return
    w.palette = controls.palette
    w.speed = controls.speed
    w.brightness = controls.brightness
    w.paused = controls.paused
    w.refreshColors()
    w.update()
  }, [controls, theme])

  // El sello va en tinta clara u oscura según lo claro que sea el fondo elegido.
  useEffect(() => {
    setSealInk(paletteLuminance(PALETTES[controls.palette].colors()) > .5 ? 'dark' : 'light')
  }, [controls.palette, theme])

  useEffect(() => { document.documentElement.style.setProperty('--gap', `${gap}px`) }, [gap])

  useEffect(() => {
    document.documentElement.lang = lang
    document.querySelector('meta[name="description"]')?.setAttribute('content', t('meta.description'))
  }, [lang, t])

  const subscribeFps = useCallback((fn: (fps: number) => void) => {
    fpsListeners.current.add(fn)
    return () => { fpsListeners.current.delete(fn) }
  }, [])

  const toggleTheme = () => {
    const next: Theme = theme === 'dark' ? 'light' : 'dark'
    applyTheme(next)
    setTheme(next)
  }
  const toggleLang = () => setLang(lang === 'es' ? 'en' : 'es')

  return (
    <FocusProvider>
      <canvas ref={canvasRef} className="wall" aria-hidden="true" />
      <main className={desktop ? 'is-desktop' : undefined} inert={desktop}>
        <Hero controls={controls} onChange={setControls} glState={glState} sealInk={sealInk} />
        <Why />
        <Monitors theme={theme} />
        <Stage />
        <Footer theme={theme} onToggleTheme={toggleTheme} onToggleLang={toggleLang} />
      </main>
      <Taskbar
        desktop={desktop}
        onToggleDesktop={() => setDesktop(d => !d)}
        gap={gap}
        onGap={setGap}
        subscribeFps={subscribeFps}
        theme={theme}
        onToggleTheme={toggleTheme}
        onToggleLang={toggleLang}
      />
    </FocusProvider>
  )
}
