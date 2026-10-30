import { useEffect, useRef } from 'react'
import { LiveWallpaper } from '../wallpaper/live'
import type { Theme } from '../theme'

/** El mismo fondo de la landing, pero en pausa: un solo cuadro, sin gastar mientras se lee. */
export function Desktop({ theme }: { theme: Theme }) {
  const ref = useRef<HTMLCanvasElement>(null)
  const wall = useRef<LiveWallpaper | null>(null)

  useEffect(() => {
    const w = new LiveWallpaper(ref.current!)
    w.paused = true
    w.update()
    wall.current = w
    return () => w.destroy()
  }, [])

  useEffect(() => { wall.current?.refreshColors() }, [theme])

  return <canvas ref={ref} className="wall" aria-hidden="true" />
}
