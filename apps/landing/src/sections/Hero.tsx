import { useRef, type PointerEvent as ReactPointerEvent } from 'react'
import { useTranslation } from 'react-i18next'
import { Window, Swatches, RangeField, CheckField } from '@mirai-shin/ui'
import type { Lang } from '@mirai-shin/i18n'
import brand from '@brand'
import { PALETTES } from '../wallpaper/palettes'

export type WallControls = { palette: number; speed: number; brightness: number; paused: boolean }

type Props = {
  controls: WallControls
  onChange: (c: WallControls) => void
  glState: 'ok' | 'noGl' | 'reduced'
  sealInk: 'light' | 'dark'
}

/** Inicio: el escritorio con dos ventanas. La de la app cambia el fondo de toda la página. */
export function Hero({ controls, onChange, glState, sealInk }: Props) {
  const { t, i18n } = useTranslation()
  const lang = (i18n.resolvedLanguage ?? 'es') as Lang
  const heroRef = useRef<HTMLElement>(null)
  const drag = useDrag(heroRef)
  const set = (patch: Partial<WallControls>) => onChange({ ...controls, ...patch })

  return (
    <section className="hero" id="inicio" ref={heroRef} aria-labelledby="h-hero">
      <div className={`seal ink-${sealInk}`}>
        <span className="kanji" lang="ja">{brand.kanji}</span>
        <span className="motto">{brand.motto[lang]}</span>
      </div>

      <Window title={t('hero.welcome')} className="w-read" barProps={drag()}>
        <h1 id="h-hero">{t('hero.title')}</h1>
        <p>{t('hero.text')}</p>
        <p className="status">{t('hero.status')}</p>
        <div className="actions">
          <a className="ms-btn" href={brand.repo} target="_blank" rel="noopener">{t('oss.github')}</a>
          <a className="ms-link" href="#etapa">{t('hero.stageLink')}</a>
        </div>
      </Window>

      <Window title={brand.name} className="w-app" barProps={drag()}>
        <p className="hint">{t('app.hint')}</p>
        <div className="ms-field">
          <span>{t('app.wallpaper')}</span>
          <Swatches
            label={t('app.wallpaper')}
            value={controls.palette}
            onChange={palette => set({ palette })}
            options={PALETTES.map((p, i) => ({ value: i, label: t(`palettes.${p.key}`) }))}
          />
        </div>
        <RangeField label={t('app.speed')} min={0} max={3} step={0.05} value={controls.speed} onChange={speed => set({ speed })} />
        <RangeField label={t('app.bright')} min={0.3} max={1.3} step={0.01} value={controls.brightness} onChange={brightness => set({ brightness })} />
        <CheckField label={t('app.pause')} checked={controls.paused} onChange={paused => set({ paused })} />
        {glState !== 'ok' && <p className="hint">{t(`app.${glState}`)}</p>}
      </Window>
    </section>
  )
}

/** Ventanas arrastrables por su barra, dentro del inicio y solo en pantallas anchas. */
function useDrag(areaRef: React.RefObject<HTMLElement | null>) {
  // El estado vive en un ref: un re-render a mitad del arrastre no lo corta.
  const st = useRef({ sx: 0, sy: 0, ox: 0, oy: 0, dragging: false })
  const win = (e: ReactPointerEvent<HTMLDivElement>) => e.currentTarget.parentElement as HTMLElement
  const end = (e: ReactPointerEvent<HTMLDivElement>) => { st.current.dragging = false; win(e).style.transition = '' }
  const handlers = {
    onPointerDown: (e: ReactPointerEvent<HTMLDivElement>) => {
      if (!matchMedia('(min-width: 900px)').matches) return
      const w = win(e)
      st.current = { sx: e.clientX, sy: e.clientY, ox: +(w.dataset.dx ?? 0), oy: +(w.dataset.dy ?? 0), dragging: true }
      e.currentTarget.setPointerCapture(e.pointerId)
      w.style.transition = 'none'
      areaRef.current?.querySelectorAll<HTMLElement>('.ms-win').forEach(x => { x.style.zIndex = '2' })
      w.style.zIndex = '4'
    },
    onPointerMove: (e: ReactPointerEvent<HTMLDivElement>) => {
      const area = areaRef.current, d = st.current
      if (!d.dragging || !area) return
      const w = win(e)
      const f = area.getBoundingClientRect(), r = w.getBoundingClientRect()
      const baseL = r.left - f.left - +(w.dataset.dx ?? 0), baseT = r.top - f.top - +(w.dataset.dy ?? 0)
      const dx = Math.max(-baseL, Math.min(f.width - r.width - baseL, d.ox + e.clientX - d.sx))
      const dy = Math.max(-baseT, Math.min(f.height - 40 - baseT, d.oy + e.clientY - d.sy))
      w.dataset.dx = String(dx); w.dataset.dy = String(dy)
      w.style.transform = `translate(${dx}px, ${dy}px)`
    },
    onPointerUp: end,
    onPointerCancel: end,
  }
  return () => handlers
}
