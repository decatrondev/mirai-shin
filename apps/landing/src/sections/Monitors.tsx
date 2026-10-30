import { useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Window, Swatches } from '@mirai-shin/ui'
import { LiveWallpaper } from '../wallpaper/live'
import { PALETTES } from '../wallpaper/palettes'
import type { Theme } from '../theme'
import { useFocusable } from '../focus'

// Los nombres son los que usa el sistema para cada salida de video.
const SCREENS = [
  { name: 'HDMI-1', kind: 'portrait', palette: 2 },
  { name: 'DP-1', kind: 'primary', palette: 0 },
  { name: 'DP-2', kind: '', palette: 1 },
] as const

/** Tres pantallas, cada una con su propio fondo vivo. */
export function Monitors({ theme }: { theme: Theme }) {
  const { t } = useTranslation()
  const [selected, setSelected] = useState(1)
  const [palettes, setPalettes] = useState<number[]>(SCREENS.map(s => s.palette))
  const canvases = useRef<(HTMLCanvasElement | null)[]>([])
  const walls = useRef<LiveWallpaper[]>([])
  const focus = useFocusable('monitors')

  useEffect(() => {
    walls.current = canvases.current.map((c, i) => new LiveWallpaper(c!, { palette: SCREENS[i].palette }))
    return () => walls.current.forEach(w => w.destroy())
  }, [])

  useEffect(() => {
    walls.current.forEach((w, i) => { w.palette = palettes[i]; w.refreshColors() })
  }, [palettes, theme])

  const label = (s: (typeof SCREENS)[number]) => (s.kind ? t(`monitors.${s.kind}`, { name: s.name }) : s.name)

  return (
    <section className="tiled" id="monitores" aria-labelledby="h-mons">
      <Window as="article" title={t('monitors.bar')} className={`ms-tile mons${focus.focused ? ' is-focus' : ''}`} onPointerDown={focus.onPointerDown}>
        <div>
          <h2 id="h-mons">{t('monitors.title')}</h2>
          <p>{t('monitors.text')}</p>
        </div>
        <div className="desk">
          {SCREENS.map((s, i) => (
            <button key={s.name} type="button" className={`mon m${i}`} aria-pressed={selected === i} onClick={() => setSelected(i)}>
              <span className="scr"><canvas ref={el => { canvases.current[i] = el }} aria-hidden="true" /></span>
              <span className="neck" />
              <span className="base" />
              <span className="tag">{label(s)}</span>
            </button>
          ))}
        </div>
        <div className="mon-panel">
          <span>{t('monitors.wallpaperFor')} <b>{SCREENS[selected].name}</b></span>
          <Swatches
            label={t('monitors.wallpaperFor')}
            value={palettes[selected]}
            onChange={p => setPalettes(prev => prev.map((v, i) => (i === selected ? p : v)))}
            options={PALETTES.map((p, i) => ({ value: i, label: t(`palettes.${p.key}`) }))}
          />
        </div>
      </Window>
    </section>
  )
}
