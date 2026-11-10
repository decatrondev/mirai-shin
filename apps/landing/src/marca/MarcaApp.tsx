import { useEffect, useRef, useState, type FormEvent, type ReactNode } from 'react'
import { useTranslation } from 'react-i18next'
import brand from '@brand'
import { applyTheme, currentTheme, type Theme } from '../theme'
import { setLang } from '../i18n'
import { LogoTypeArt, PaletteArt, SmallArt, StyleArt, SymbolArt } from './Samples'
import './marca.css'

/* Las listas tienen que coincidir con services/web/src/survey.rs (el servidor rechaza lo demás). */
const AXES = ['serio_jugueton', 'tecnico_artistico', 'simple_detallado', 'tranquilo_energico', 'japones_evidente_sutil'] as const
const WORDS = ['liviano', 'nocturno', 'vivo', 'tranquilo', 'preciso', 'calido', 'futurista', 'artesanal', 'libre', 'elegante', 'jugueton', 'tecnico'] as const
const SYMBOLS = ['kanji', 'mecha', 'monitor', 'onda', 'abstracto', 'letras'] as const
const LOGO_TYPES = ['simbolo', 'simbolo_nombre', 'nombre', 'sello'] as const
const STYLE_PAIRS = ['geometrico_pincel', 'fina_rellena', 'plano_volumen', 'redondo_anguloso', 'simetrico_organico'] as const
const PALETTE_IDS = ['actual', 'hora_azul', 'sello', 'amanecer', 'mono'] as const
const SMALL = ['simple', 'medio', 'detallado'] as const
const MAX_WORDS = 3, MAX_SYMBOLS = 2, MAX_AVOID = 500, MAX_COMMENT = 1000

const STEPS = ['intro', 'personality', 'words', 'symbols', 'logoType', 'styles', 'palette', 'small', 'free', 'review'] as const
type Step = typeof STEPS[number]
const TOTAL = STEPS.length - 1

interface Answers {
  personality: Record<string, number>
  words: string[]
  symbols: string[]
  logoType: string | null
  styles: Record<string, string>
  palette: string | null
  small: string | null
  avoid: string
  comment: string
}
interface Draft { step: number; name: string; answers: Answers; sent: { id: string; token: string } | null; done: boolean }

const KEY = 'marca-1'
const empty = (): Draft => ({
  step: 0, name: '', sent: null, done: false,
  answers: { personality: {}, words: [], symbols: [], logoType: null, styles: {}, palette: null, small: null, avoid: '', comment: '' },
})
function load(): Draft {
  try {
    const raw = localStorage.getItem(KEY)
    if (raw) { const d = JSON.parse(raw); return { ...empty(), ...d, answers: { ...empty().answers, ...d.answers } } }
  } catch { /* sin almacenamiento: se empieza de cero */ }
  return empty()
}
function save(d: Draft) { try { localStorage.setItem(KEY, JSON.stringify(d)) } catch { /* ignorar */ } }

async function send(d: Draft, lang: string, website: string): Promise<{ id: string; token: string } | null> {
  const body = JSON.stringify({ name: d.name, lang, answers: d.answers, website })
  const headers: Record<string, string> = { 'content-type': 'application/json' }
  if (d.sent) headers['x-edit-token'] = d.sent.token
  let res: Response
  try {
    res = await fetch(d.sent ? `/api/survey/marca/${d.sent.id}` : '/api/survey/marca', { method: d.sent ? 'PUT' : 'POST', headers, body })
  } catch { throw new Error('network') }
  if (!res.ok) {
    let code = 'generic'
    try { code = (await res.json()).error ?? code } catch { /* no es JSON */ }
    throw new Error(code)
  }
  return d.sent ? null : res.json()
}

const nameOk = (n: string) => { const l = n.trim().length; return l >= 2 && l <= 60 }
/** Si el paso ya tiene alguna respuesta (para marcarlo en la lista). */
function answered(s: Step, d: Draft): boolean {
  const a = d.answers
  switch (s) {
    case 'intro': return nameOk(d.name)
    case 'personality': return Object.keys(a.personality).length > 0
    case 'words': return a.words.length > 0
    case 'symbols': return a.symbols.length > 0
    case 'logoType': return !!a.logoType
    case 'styles': return Object.keys(a.styles).length > 0
    case 'palette': return !!a.palette
    case 'small': return !!a.small
    case 'free': return !!(a.avoid.trim() || a.comment.trim())
    case 'review': return !!d.sent
  }
}

const toggle = (list: string[], v: string, max: number) =>
  list.includes(v) ? list.filter(x => x !== v) : list.length < max ? [...list, v] : list

/** Encuesta pública de la marca: `/marca`. Una pantalla por grupo de preguntas; borrador en el navegador. */
export default function MarcaApp() {
  const { t, i18n } = useTranslation('marca')
  const [d, setD] = useState<Draft>(load)
  const [theme, setTheme] = useState<Theme>(currentTheme)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const [notice, setNotice] = useState('')
  const [website, setWebsite] = useState('')
  const head = useRef<HTMLHeadingElement>(null)
  const first = useRef(true)
  const step: Step = d.done ? 'review' : STEPS[d.step]
  const lang = i18n.resolvedLanguage === 'en' ? 'en' : 'es'

  useEffect(() => { save(d) }, [d])
  useEffect(() => { document.title = t('meta.title', { brand: brand.name }) }, [t, lang])
  // Al cambiar de paso, el foco va al título (lectores de pantalla y teclado) y se sube al inicio.
  useEffect(() => {
    if (first.current) { first.current = false; return }
    head.current?.focus({ preventScroll: true })
    window.scrollTo({ top: 0 })
  }, [d.step, d.done])

  const set = (patch: Partial<Answers>) => setD(x => ({ ...x, answers: { ...x.answers, ...patch } }))
  const go = (n: number) => { setError(''); setNotice(''); setD(x => ({ ...x, step: Math.max(0, Math.min(TOTAL, n)), done: false })) }
  const a = d.answers

  const submit = async () => {
    setBusy(true); setError(''); setNotice('')
    try {
      const created = await send(d, lang, website)
      setD(x => ({ ...x, sent: created ?? x.sent, done: true }))
      if (!created) setNotice(t('done.saved'))
    } catch (e) {
      const code = e instanceof Error ? e.message : 'generic'
      // El código del navegador ya no vale (respuesta borrada): la próxima vez se manda como nueva.
      if (code === 'not_found') setD(x => ({ ...x, sent: null }))
      setError(t(`errors.${code}`, t('errors.generic')))
    } finally { setBusy(false) }
  }

  const toggleTheme = () => { const next: Theme = theme === 'dark' ? 'light' : 'dark'; applyTheme(next); setTheme(next) }

  return (
    <div className="mk">
      <header className="mk-top">
        <div className="mk-top-in">
        <span className="mk-brand"><span className="mk-kanji" aria-hidden="true">芯</span>{brand.name}</span>
        <span className="mk-top-actions">
          <button type="button" className="mk-flat" lang={lang === 'es' ? 'en' : 'es'} onClick={() => setLang(lang === 'es' ? 'en' : 'es')}>{t('top.lang')}</button>
          <button type="button" className="mk-flat" onClick={toggleTheme}>{t(theme === 'dark' ? 'top.toLight' : 'top.toDark')}</button>
        </span>
        </div>
      </header>

      <div className="mk-layout">
      <aside className="mk-aside" aria-label={t('steps.label')}>
        <div className="mk-progress">
          <span>{d.step > 0 && !d.done ? t('top.step', { n: d.step, total: TOTAL }) : t('steps.label')}</span>
          <div className="mk-bar" role="progressbar" aria-valuemin={0} aria-valuemax={TOTAL} aria-valuenow={d.done ? TOTAL : d.step} aria-label={t('top.step', { n: d.done ? TOTAL : d.step, total: TOTAL })}>
            <i style={{ width: `${((d.done ? TOTAL : d.step) / TOTAL) * 100}%` }} />
          </div>
        </div>
        <ol className="mk-steps">
          {STEPS.map((s, i) => {
            const current = !d.done && d.step === i, ok = answered(s, d)
            return (
              <li key={s}>
                <button type="button" className="mk-stepbtn" aria-current={current ? 'step' : undefined} disabled={!nameOk(d.name) && i > 0}
                  onClick={() => go(i)}>
                  <span className={ok ? 'mk-tick is-ok' : 'mk-tick'} aria-hidden="true">{ok ? '✓' : i === 0 ? '·' : i}</span>
                  <span>{t(`steps.${s}`)}</span>
                  {ok && <span className="mk-sr">, {t('steps.answered')}</span>}
                </button>
              </li>
            )
          })}
        </ol>
      </aside>

      <main className="mk-main">

        {d.done ? (
          <Done name={d.name} notice={notice} headRef={head} onEdit={() => go(1)} />
        ) : step === 'intro' ? (
          <Intro d={d} headRef={head} onName={name => setD(x => ({ ...x, name }))} website={website} onWebsite={setWebsite}
            onStart={() => go(1)} />
        ) : (
          <section className="mk-step" aria-labelledby="mk-h">
            {step === 'personality' && (
              <Head id={step} headRef={head}>
                <div className="mk-axes">
                  {AXES.map(ax => {
                    const [l, r] = t(`personality.axes.${ax}`, { returnObjects: true }) as string[]
                    const pos = t('personality.pos', { returnObjects: true, a: l, b: r }) as string[]
                    const v = a.personality[ax]
                    return (
                      <fieldset key={ax} className="mk-axis">
                        <legend className="mk-sr">{l} / {r}</legend>
                        <span className="mk-axis-l" aria-hidden="true">{l}</span>
                        <span className="mk-dots">
                          {[-2, -1, 0, 1, 2].map((n, i) => (
                            <label key={n} className={`mk-dot s${Math.abs(n)}`} title={pos[i]}>
                              <input type="radio" name={ax} checked={v === n} onChange={() => set({ personality: { ...a.personality, [ax]: n } })} />
                              <span className="mk-sr">{pos[i]}</span>
                            </label>
                          ))}
                        </span>
                        <span className="mk-axis-r" aria-hidden="true">{r}</span>
                      </fieldset>
                    )
                  })}
                </div>
              </Head>
            )}

            {step === 'words' && (
              <Head id={step} headRef={head} extra={<p className="mk-count" aria-live="polite">{t('words.count', { n: a.words.length })}</p>}>
                <div className="mk-words">
                  {WORDS.map(w => {
                    const on = a.words.includes(w), full = !on && a.words.length >= MAX_WORDS
                    return (
                      <button key={w} type="button" className="mk-word" aria-pressed={on} disabled={full}
                        onClick={() => set({ words: toggle(a.words, w, MAX_WORDS) })}>{t(`words.items.${w}`)}</button>
                    )
                  })}
                </div>
              </Head>
            )}

            {step === 'symbols' && (
              <Head id={step} headRef={head}>
                <div className="mk-cards">
                  {SYMBOLS.map(s => {
                    const [name, desc] = t(`symbols.items.${s}`, { returnObjects: true }) as string[]
                    const on = a.symbols.includes(s), full = !on && a.symbols.length >= MAX_SYMBOLS
                    return (
                      <button key={s} type="button" className="mk-card" aria-pressed={on} disabled={full}
                        onClick={() => set({ symbols: toggle(a.symbols, s, MAX_SYMBOLS) })}>
                        <SymbolArt id={s} /><b>{name}</b><small>{desc}</small>
                      </button>
                    )
                  })}
                </div>
              </Head>
            )}

            {step === 'logoType' && (
              <Head id={step} headRef={head}>
                <div className="mk-cards mk-cards-2" role="radiogroup" aria-labelledby="mk-h">
                  {LOGO_TYPES.map(s => {
                    const [name, desc] = t(`logoType.items.${s}`, { returnObjects: true }) as string[]
                    return (
                      <button key={s} type="button" role="radio" className="mk-card" aria-checked={a.logoType === s}
                        onClick={() => set({ logoType: a.logoType === s ? null : s })}>
                        <LogoTypeArt id={s} brand={brand.name} /><b>{name}</b><small>{desc}</small>
                      </button>
                    )
                  })}
                </div>
              </Head>
            )}

            {step === 'styles' && (
              <Head id={step} headRef={head}>
                <div className="mk-pairs">
                  {STYLE_PAIRS.map(p => {
                    const [la, lb] = t(`styles.pairs.${p}`, { returnObjects: true }) as string[]
                    const v = a.styles[p]
                    const pick = (c: string) => set({ styles: { ...a.styles, [p]: c } })
                    return (
                      <div key={p} className="mk-pair" role="radiogroup" aria-label={`${la} / ${lb}`}>
                        <button type="button" role="radio" aria-checked={v === 'a'} className="mk-side" onClick={() => pick('a')}><StyleArt pair={p} side="a" /><span>{la}</span></button>
                        <button type="button" role="radio" aria-checked={v === 'igual'} className="mk-same" onClick={() => pick('igual')}>{t('styles.same')}</button>
                        <button type="button" role="radio" aria-checked={v === 'b'} className="mk-side" onClick={() => pick('b')}><StyleArt pair={p} side="b" /><span>{lb}</span></button>
                      </div>
                    )
                  })}
                </div>
              </Head>
            )}

            {step === 'palette' && (
              <Head id={step} headRef={head}>
                <div className="mk-cards mk-cards-2" role="radiogroup" aria-labelledby="mk-h">
                  {PALETTE_IDS.map(p => {
                    const [name, desc] = t(`palette.items.${p}`, { returnObjects: true }) as string[]
                    return (
                      <button key={p} type="button" role="radio" className="mk-card" aria-checked={a.palette === p}
                        onClick={() => set({ palette: a.palette === p ? null : p })}>
                        <PaletteArt id={p} /><b>{name}</b><small>{desc}</small>
                      </button>
                    )
                  })}
                </div>
              </Head>
            )}

            {step === 'small' && (
              <Head id={step} headRef={head}>
                <div className="mk-taskbar" aria-label={t('small.taskbar')}>
                  <span className="mk-tb-start" aria-hidden="true" />
                  {SMALL.map(s => <span key={s} className="mk-tb-icon"><SmallArt id={s} size={16} /></span>)}
                  <span className="mk-tb-clock">12:00</span>
                </div>
                <p className="mk-hint">{t('small.taskbar')}</p>
                <div className="mk-cards mk-cards-3" role="radiogroup" aria-labelledby="mk-h">
                  {SMALL.map(s => (
                    <button key={s} type="button" role="radio" className="mk-card mk-small" aria-checked={a.small === s}
                      onClick={() => set({ small: a.small === s ? null : s })}>
                      <span className="mk-sizes"><SmallArt id={s} size={16} /><SmallArt id={s} size={24} /><SmallArt id={s} size={64} /></span>
                      <b>{t(`small.items.${s}`)}</b>
                    </button>
                  ))}
                </div>
              </Head>
            )}

            {step === 'free' && (
              <Head id={step} headRef={head} noLead>
                <label className="mk-field">
                  <span>{t('free.avoid')}</span>
                  <small>{t('free.avoidHint')}</small>
                  <textarea rows={3} maxLength={MAX_AVOID} value={a.avoid} onChange={e => set({ avoid: e.target.value })} />
                  <small className="mk-count">{t('free.count', { n: a.avoid.length, max: MAX_AVOID })}</small>
                </label>
                <label className="mk-field">
                  <span>{t('free.comment')}</span>
                  <textarea rows={5} maxLength={MAX_COMMENT} value={a.comment} onChange={e => set({ comment: e.target.value })} />
                  <small className="mk-count">{t('free.count', { n: a.comment.length, max: MAX_COMMENT })}</small>
                </label>
              </Head>
            )}

            {step === 'review' && (
              <Head id={step} headRef={head}>
                <Review d={d} onChange={go} />
              </Head>
            )}

            {error && <p className="mk-error" role="alert">{error}</p>}

            <nav className="mk-nav">
              <button type="button" className="mk-ghost" onClick={() => go(d.step - 1)}>{t('nav.back')}</button>
              {step === 'review'
                ? <button type="button" className="mk-primary" disabled={busy || d.name.trim().length < 2} onClick={submit}>
                    {busy ? t('nav.sending') : t(d.sent ? 'nav.save' : 'nav.send')}
                  </button>
                : <button type="button" className="mk-primary" onClick={() => go(d.step + 1)}>{t(d.step === TOTAL - 1 ? 'nav.review' : 'nav.next')}</button>}
            </nav>
          </section>
        )}
      </main>
      </div>
    </div>
  )
}

function Head({ id, headRef, children, extra, noLead }: { id: Step; headRef: React.RefObject<HTMLHeadingElement | null>; children: ReactNode; extra?: ReactNode; noLead?: boolean }) {
  const { t } = useTranslation('marca')
  return (
    <>
      <h1 id="mk-h" ref={headRef} tabIndex={-1}>{t(`${id}.title`, { brand: brand.name })}</h1>
      {!noLead && <p className="mk-lead">{t(`${id}.lead`)}</p>}
      {extra}
      {children}
    </>
  )
}

function Intro({ d, headRef, onName, website, onWebsite, onStart }: {
  d: Draft; headRef: React.RefObject<HTMLHeadingElement | null>; onName: (n: string) => void
  website: string; onWebsite: (v: string) => void; onStart: () => void
}) {
  const { t } = useTranslation('marca')
  const [touched, setTouched] = useState(false)
  const ok = d.name.trim().length >= 2 && d.name.trim().length <= 60
  const start = (e: FormEvent) => { e.preventDefault(); setTouched(true); if (ok) onStart() }
  return (
    <form className="mk-step mk-intro" onSubmit={start} noValidate>
      <h1 id="mk-h" ref={headRef} tabIndex={-1}>{t('intro.title', { brand: brand.name })}</h1>
      <p className="mk-lead">{t('intro.lead', { brand: brand.name })}</p>
      <p className="mk-lead">{t('intro.how')}</p>
      {d.sent && <p className="mk-note">{t('intro.edit')}</p>}
      <label className="mk-field">
        <span>{t('intro.name')}</span>
        <small id="mk-name-hint">{t('intro.nameHint')}</small>
        <input value={d.name} maxLength={60} autoComplete="nickname" aria-describedby="mk-name-hint" aria-invalid={touched && !ok}
          onChange={e => onName(e.target.value)} />
      </label>
      {touched && !ok && <p className="mk-error" role="alert">{t('errors.invalid_name')}</p>}
      {/* Campo trampa para bots: oculto a la vista y a los lectores de pantalla. */}
      <input className="mk-trap" tabIndex={-1} autoComplete="off" aria-hidden="true" name="website" value={website} onChange={e => onWebsite(e.target.value)} />
      <p className="mk-privacy">{t('intro.privacy', { brand: brand.name })}</p>
      <nav className="mk-nav"><span /><button type="submit" className="mk-primary">{t('intro.start')}</button></nav>
    </form>
  )
}

function Review({ d, onChange }: { d: Draft; onChange: (step: number) => void }) {
  const { t } = useTranslation('marca')
  const a = d.answers
  const none = <i className="mk-muted">{t('review.skipped')}</i>
  const pair = (k: string) => (t(k, { returnObjects: true }) as string[])
  const rows: [Step | 'name', ReactNode][] = [
    ['name', d.name || none],
    ['personality', Object.keys(a.personality).length ? (
      <ul>{AXES.filter(ax => ax in a.personality).map(ax => {
        const [l, r] = pair(`personality.axes.${ax}`)
        return <li key={ax}>{(t('personality.pos', { returnObjects: true, a: l, b: r }) as string[])[a.personality[ax] + 2]}</li>
      })}</ul>) : none],
    ['words', a.words.length ? a.words.map(w => t(`words.items.${w}`)).join(', ') : none],
    ['symbols', a.symbols.length ? a.symbols.map(s => pair(`symbols.items.${s}`)[0]).join(', ') : none],
    ['logoType', a.logoType ? pair(`logoType.items.${a.logoType}`)[0] : none],
    ['styles', Object.keys(a.styles).length ? (
      <ul>{STYLE_PAIRS.filter(p => p in a.styles).map(p => {
        const [la, lb] = pair(`styles.pairs.${p}`)
        return <li key={p}>{a.styles[p] === 'a' ? la : a.styles[p] === 'b' ? lb : `${la} / ${lb}: ${t('styles.same')}`}</li>
      })}</ul>) : none],
    ['palette', a.palette ? pair(`palette.items.${a.palette}`)[0] : none],
    ['small', a.small ? t(`small.items.${a.small}`) : none],
    ['free', a.avoid || a.comment ? <>{a.avoid && <p>{a.avoid}</p>}{a.comment && <p>{a.comment}</p>}</> : none],
  ]
  return (
    <dl className="mk-review">
      {rows.map(([k, v]) => {
        const stepIndex = k === 'name' ? 0 : STEPS.indexOf(k)
        return (
          <div key={k}>
            <dt>{k === 'name' ? t('review.name') : t(`${k}.title`, { brand: brand.name })}</dt>
            <dd>{v}</dd>
            <button type="button" className="mk-flat" onClick={() => onChange(stepIndex)}>{t('nav.change')}</button>
          </div>
        )
      })}
    </dl>
  )
}

function Done({ name, notice, headRef, onEdit }: { name: string; notice: string; headRef: React.RefObject<HTMLHeadingElement | null>; onEdit: () => void }) {
  const { t } = useTranslation('marca')
  return (
    <section className="mk-step mk-done">
      <span className="mk-done-mark" aria-hidden="true">芯</span>
      <h1 ref={headRef} tabIndex={-1}>{t('done.title', { name })}</h1>
      {notice && <p className="mk-note" role="status">{notice}</p>}
      <p className="mk-lead">{t('done.text')}</p>
      <nav className="mk-nav"><span /><button type="button" className="mk-ghost" onClick={onEdit}>{t('done.edit')}</button></nav>
    </section>
  )
}
