import { VERT, FRAG } from './shader'
import { PALETTES, hexToRgb, type Rgb } from './palettes'

// Tope de píxeles: en 2K o 4K el fondo se escala en vez de calcularse a resolución completa.
const MAX_PX = 1920 * 1080
const REVEAL_MS = 1600

/**
 * Un lienzo con un shader. Se pausa solo con la pestaña oculta, fuera de pantalla,
 * con «reducir movimiento» o en pausa manual: la página practica lo que promete la app.
 */
export class LiveWallpaper {
  readonly ok: boolean
  palette = 0
  speed = 1
  brightness = 1
  paused = false
  fps = 0
  onFrame: ((w: LiveWallpaper) => void) | null = null

  private gl: WebGL2RenderingContext | null = null
  private program: WebGLProgram | null = null
  private u: Record<string, WebGLUniformLocation | null> = {}
  private cols: Rgb[] = []
  private t = 7
  private reveal = 1
  private born = 0
  private last = 0
  private raf = 0
  private inView = true
  private observers: { disconnect(): void }[] = []
  private readonly reduce: boolean

  constructor(private canvas: HTMLCanvasElement, opts: { palette?: number; reveal?: boolean } = {}) {
    this.palette = opts.palette ?? 0
    this.reduce = matchMedia('(prefers-reduced-motion: reduce)').matches
    if (opts.reveal && !this.reduce) this.reveal = 0
    this.ok = this.init()
    if (!this.ok) { canvas.hidden = true; return }
    this.refreshColors()
    const ro = new ResizeObserver(() => this.resize())
    ro.observe(canvas)
    const io = new IntersectionObserver(es => { this.inView = es[0].isIntersecting; this.update() })
    io.observe(canvas)
    this.observers.push(ro, io)
    document.addEventListener('visibilitychange', this.update)
    this.resize()
    this.update()
  }

  get reducedMotion() { return this.reduce }

  private init() {
    const gl = this.canvas.getContext('webgl2', { antialias: false, powerPreference: 'low-power' })
    if (!gl) return false
    const compile = (type: number, src: string) => {
      const s = gl.createShader(type)!
      gl.shaderSource(s, src)
      gl.compileShader(s)
      if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(s) ?? 'shader')
      return s
    }
    try {
      const p = gl.createProgram()!
      gl.attachShader(p, compile(gl.VERTEX_SHADER, VERT))
      gl.attachShader(p, compile(gl.FRAGMENT_SHADER, FRAG))
      gl.linkProgram(p)
      if (!gl.getProgramParameter(p, gl.LINK_STATUS)) throw new Error(gl.getProgramInfoLog(p) ?? 'link')
      this.program = p
    } catch (err) {
      console.error(err)
      return false
    }
    gl.bindVertexArray(gl.createVertexArray())
    for (const k of ['r', 't', 'b', 'e', 'c0', 'c1', 'c2', 'c3']) this.u[k] = gl.getUniformLocation(this.program, k)
    this.gl = gl
    return true
  }

  refreshColors() {
    if (!this.ok) return
    this.cols = PALETTES[this.palette].colors().map(hexToRgb)
    this.draw()
  }

  resize() {
    let s = Math.min(devicePixelRatio || 1, 1.5)
    const cw = this.canvas.clientWidth, ch = this.canvas.clientHeight
    if (cw * ch * s * s > MAX_PX) s = Math.sqrt(MAX_PX / (cw * ch))
    const w = Math.max(1, Math.round(cw * s)), h = Math.max(1, Math.round(ch * s))
    if (this.canvas.width !== w || this.canvas.height !== h) { this.canvas.width = w; this.canvas.height = h }
    this.draw()
  }

  draw() {
    const gl = this.gl, u = this.u
    if (!gl || !this.program || !this.cols.length) return
    gl.viewport(0, 0, this.canvas.width, this.canvas.height)
    gl.useProgram(this.program)
    gl.uniform2f(u.r, this.canvas.width, this.canvas.height)
    gl.uniform1f(u.t, this.t)
    gl.uniform1f(u.b, this.brightness)
    gl.uniform1f(u.e, this.reveal)
    ;(['c0', 'c1', 'c2', 'c3'] as const).forEach((k, i) => gl.uniform3fv(u[k], this.cols[i]))
    gl.drawArrays(gl.TRIANGLES, 0, 3)
  }

  private shouldRun() {
    return this.ok && this.inView && !document.hidden && !this.paused && !this.reduce && this.speed > 0
  }

  /** Arranca o detiene el bucle según el estado actual. */
  update = () => {
    if (this.shouldRun()) {
      if (!this.raf) { this.last = 0; this.raf = requestAnimationFrame(this.loop) }
    } else {
      cancelAnimationFrame(this.raf)
      this.raf = 0
      this.fps = 0
      this.reveal = 1
      this.draw()
      this.onFrame?.(this)
    }
  }

  private loop = (now: number) => {
    if (!this.shouldRun()) { this.raf = 0; return }
    if (!this.born) this.born = now
    if (this.last) {
      const dt = Math.min((now - this.last) / 1000, .1)
      this.t += dt * this.speed
      const inst = 1 / Math.max(dt, 1e-3)
      this.fps = this.fps ? this.fps * .92 + inst * .08 : inst
    }
    if (this.reveal < 1) this.reveal = Math.min(1, (now - this.born) / REVEAL_MS)
    this.last = now
    this.draw()
    this.onFrame?.(this)
    this.raf = requestAnimationFrame(this.loop)
  }

  destroy() {
    cancelAnimationFrame(this.raf)
    this.raf = 0
    this.observers.forEach(o => o.disconnect())
    document.removeEventListener('visibilitychange', this.update)
  }
}
