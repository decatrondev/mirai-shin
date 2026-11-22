import { useEffect, useRef, useState, type ReactNode } from 'react'
import { useTranslation } from 'react-i18next'
import brand from '@brand'
import { applyTheme, currentTheme, type Theme } from '../theme'
import { setLang } from '../i18n'
import { loadDraft, nameOk, saveDraft, sendSurvey, type Sent } from '../surveyClient'
import { Mark } from './Mark'
import { LOGO_PALETTES, MAX_COMMENT, PALETTE_COLORS, PROPOSALS, SHEET, type LogoPalette, type Proposal } from './proposals'
import '../marca/marca.css'
import '../interfaz/interfaz.css'
import './logo.css'

const STEPS = ['intro', 'pick', 'second', 'palette', 'ratings', 'comment', 'review'] as const
type Step = typeof STEPS[number]
const TOTAL = STEPS.length - 1
const PALETTE_STEP = STEPS.indexOf('palette')

interface Answers {
  pick: Proposal | null
  second: Proposal | null
  palette: LogoPalette | null
  ratings: Partial<Record<Proposal, number>>
  comment: string
}
interface Draft { step: number; name: string; answers: Answers; sent: Sent | null; done: boolean }

const KEY = 'marca-2'
const empty = (): Draft => ({
  step: 0, name: '', sent: null, done: false,
  answers: { pick: null, second: null, palette: null, ratings: {}, comment: '' },
})

function answered(s: Step, d: Draft): boolean {
  const a = d.answers
  switch (s) {
    case 'intro': return d.step > 0 || !!a.pick
    case 'pick': return !!a.pick
    case 'second': return !!a.second
    case 'palette': return !!a.palette
    case 'ratings': return Object.keys(a.ratings).length > 0
    case 'comment': return !!a.comment.trim()
    case 'review': return !!d.sent
  }
}

/** Ronda 2 de la marca (`/marca`): votar entre las propuestas de logo y la paleta. La ronda 1 sigue en `/marca/ronda-1`. */
export default function LogoApp() {
  const { t, i18n } = useTranslation('logo')
  const [d, setD] = useState<Draft>(() => loadDraft(KEY, empty))
  const [theme, setTheme] = useState<Theme>(currentTheme)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const [notice, setNotice] = useState('')
  const [website, setWebsite] = useState('')
  const [touchedName, setTouchedName] = useState(false)
  const head = useRef<HTMLHeadingElement>(null)
  const first = useRef(true)
  const step: Step = d.done ? 'review' : STEPS[d.step]
  const lang = i18n.resolvedLanguage === 'en' ? 'en' : 'es'
  const a = d.answers
  // Mientras no elija paleta, las propuestas se muestran en la actual.
  const pal: LogoPalette = a.palette ?? 'actual'

  useEffect(() => { saveDraft(KEY, d) }, [d])
  useEffect(() => { document.title = t('meta.title', { brand: brand.name }) }, [t, lang])
  useEffect(() => {
    if (first.current) { first.current = false; return }
    head.current?.focus({ preventScroll: true })
    window.scrollTo({ top: 0 })
  }, [d.step, d.done])

  const set = (patch: Partial<Answers>) => setD(x => ({ ...x, answers: { ...x.answers, ...patch } }))
  const go = (n: number) => {
    setError(''); setNotice('')
    const target = Math.max(0, Math.min(TOTAL, n))
    // Sin propuesta no se pasa del paso de elegir; sin paleta, no se pasa del de colores.
    if (target > 1 && !d.answers.pick) { setError(t('pick.required')); setD(x => ({ ...x, step: 1, done: false })); return }
    if (target > PALETTE_STEP && !d.answers.palette) { setError(t('palette.required')); setD(x => ({ ...x, step: PALETTE_STEP, done: false })); return }
    setD(x => ({ ...x, step: target, done: false }))
  }
  // Al elegir lo obligatorio se quita el aviso «Elige…».
  const choosePick = (v: Proposal) => { setError(''); set({ pick: v, second: a.second === v ? null : a.second }) }
  const choosePalette = (p: LogoPalette) => { setError(''); set({ palette: p }) }

  const submit = async () => {
    setTouchedName(true)
    if (!nameOk(d.name)) { setError(t('errors.invalid_name')); return }
    setBusy(true); setError(''); setNotice('')
    try {
      const created = await sendSurvey('logo', { name: d.name, lang, answers: d.answers, website }, d.sent)
      setD(x => ({ ...x, sent: created ?? x.sent, done: true }))
      if (!created) setNotice(t('done.saved'))
    } catch (e) {
      const code = e instanceof Error ? e.message : 'generic'
      if (code === 'not_found') setD(x => ({ ...x, sent: null }))
      setError(t(`errors.${code}`, t('errors.generic')))
    } finally { setBusy(false) }
  }

  const toggleTheme = () => { const next: Theme = theme === 'dark' ? 'light' : 'dark'; applyTheme(next); setTheme(next) }
  const ready = !!a.pick && !!a.palette

  return (
    <div className="mk ui lg">
      <header className="mk-top">
        <div className="mk-top-in">
          <span className="mk-brand"><span className="mk-kanji" aria-hidden="true">芯</span>{brand.name}</span>
          <span className="mk-top-actions">
            <button type="button" className="mk-flat" lang={lang === 'es' ? 'en' : 'es'} onClick={() => setLang(lang === 'es' ? 'en' : 'es')}>{t('top.lang', { ns: 'marca' })}</button>
            <button type="button" className="mk-flat" onClick={toggleTheme}>{t(theme === 'dark' ? 'top.toLight' : 'top.toDark', { ns: 'marca' })}</button>
          </span>
        </div>
      </header>

      <div className="mk-layout">
        <aside className="mk-aside" aria-label={t('steps.label')}>
          <div className="mk-progress">
            <span>{d.step > 0 && !d.done ? t('top.step', { ns: 'marca', n: d.step, total: TOTAL }) : t('steps.label')}</span>
            <div className="mk-bar" role="progressbar" aria-valuemin={0} aria-valuemax={TOTAL} aria-valuenow={d.done ? TOTAL : d.step}
              aria-label={t('top.step', { ns: 'marca', n: d.done ? TOTAL : d.step, total: TOTAL })}>
              <i style={{ width: `${((d.done ? TOTAL : d.step) / TOTAL) * 100}%` }} />
            </div>
          </div>
          <ol className="mk-steps">
            {STEPS.map((s, i) => {
              const current = !d.done && d.step === i, ok = answered(s, d)
              return (
                <li key={s}>
                  <button type="button" className="mk-stepbtn" aria-current={current ? 'step' : undefined}
                    disabled={(i > 1 && !a.pick) || (i > PALETTE_STEP && !a.palette)} onClick={() => go(i)}>
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
            <section className="mk-step mk-done">
              {a.pick && a.palette && <span className="lg-done-mark" style={{ background: PALETTE_COLORS[a.palette].bg }}><Mark kind={a.pick} palette={a.palette} size="100%" /></span>}
              <h1 ref={head} tabIndex={-1}>{t('done.title', { name: d.name })}</h1>
              {notice && <p className="mk-note" role="status">{notice}</p>}
              <p className="mk-lead">{t('done.text')}</p>
              <p><a className="ui-link" href="/interfaz">{t('done.interfaz')}</a></p>
              <nav className="mk-nav"><span /><button type="button" className="mk-ghost" onClick={() => go(1)}>{t('done.edit')}</button></nav>
            </section>
          ) : (
            <section className="mk-step" aria-labelledby="mk-h">
              {step === 'intro' && (
                <>
                  <h1 id="mk-h" ref={head} tabIndex={-1}>{t('intro.title', { brand: brand.name })}</h1>
                  <p className="mk-lead">{t('intro.lead', { brand: brand.name })}</p>
                  <p className="mk-lead">{t('intro.how')}</p>
                  {d.sent && <p className="mk-note">{t('intro.edit')}</p>}
                  <ul className="lg-gallery">
                    {PROPOSALS.map(v => (
                      <li key={v} className="lg-item">
                        <div className="lg-palettes">
                          {LOGO_PALETTES.map(p => (
                            <span key={p} className="lg-tile" style={{ background: PALETTE_COLORS[p].bg }}>
                              <Mark kind={v} palette={p} size="100%" title={`${t(`proposals.${v}.name`)} · ${t(`palettes.${p}.name`)}`} />
                            </span>
                          ))}
                        </div>
                        <b>{t(`proposals.${v}.name`)}</b>
                        <small>{t(`proposals.${v}.desc`)}</small>
                        <Small kind={v} palette="actual" />
                      </li>
                    ))}
                  </ul>
                  <p className="mk-hint">{t('intro.small')} <a className="ui-link" href={SHEET} target="_blank" rel="noreferrer">{t('intro.sheet')}</a></p>
                  {/* Campo trampa para bots: oculto a la vista y a los lectores de pantalla. */}
                  <input className="mk-trap" tabIndex={-1} autoComplete="off" aria-hidden="true" name="website" value={website} onChange={e => setWebsite(e.target.value)} />
                </>
              )}

              {step === 'pick' && (
                <Head id="pick" headRef={head}>
                  <Cards palette={pal} value={a.pick} options={PROPOSALS} onPick={v => v && choosePick(v)} />
                </Head>
              )}

              {step === 'second' && (
                <Head id="second" headRef={head}>
                  <Cards palette={pal} value={a.second} options={PROPOSALS.filter(v => v !== a.pick)} onPick={v => set({ second: v })} allowNone />
                </Head>
              )}

              {step === 'palette' && a.pick && (
                <Head id="palette" headRef={head}>
                  <div className="lg-cards lg-cards-3" role="radiogroup" aria-labelledby="mk-h">
                    {LOGO_PALETTES.map(p => (
                      <button key={p} type="button" role="radio" className="mk-card lg-card" aria-checked={a.palette === p} onClick={() => choosePalette(p)}>
                        <span className="lg-stage" style={{ background: PALETTE_COLORS[p].bg }}>
                          <Mark kind={a.pick!} palette={p} size="100%" />
                        </span>
                        <span className="lg-swatches" aria-hidden="true">
                          {(['bg', 'ink', 'acc'] as const).map(c => <i key={c} style={{ background: PALETTE_COLORS[p][c] }} />)}
                        </span>
                        <b>{t(`palettes.${p}.name`)}</b>
                        <small>{t(`palettes.${p}.desc`)}</small>
                      </button>
                    ))}
                  </div>
                </Head>
              )}

              {step === 'ratings' && (
                <Head id="ratings" headRef={head}>
                  <Ratings palette={pal} value={a.ratings} onChange={ratings => set({ ratings })} />
                </Head>
              )}

              {step === 'comment' && (
                <Head id="comment" headRef={head}>
                  <label className="mk-field">
                    <span>{t('comment.label')}</span>
                    <textarea rows={5} maxLength={MAX_COMMENT} value={a.comment} onChange={e => set({ comment: e.target.value })} />
                    <small className="mk-count">{t('comment.count', { n: a.comment.length, max: MAX_COMMENT })}</small>
                  </label>
                </Head>
              )}

              {step === 'review' && (
                <Head id="review" headRef={head}>
                  <label className="mk-field">
                    <span>{t('review.name')}</span>
                    <small id="lg-name-hint">{t('review.nameHint')}</small>
                    <input value={d.name} maxLength={60} autoComplete="nickname" aria-describedby="lg-name-hint" aria-invalid={touchedName && !nameOk(d.name)}
                      onChange={e => { const name = e.target.value; setD(x => ({ ...x, name })); if (nameOk(name)) setError('') }}
                      onBlur={() => setTouchedName(true)} />
                  </label>
                  <p className="mk-privacy">{t('review.privacy', { brand: brand.name })}</p>
                  <Review d={d} onChange={go} />
                </Head>
              )}

              {error && <p className="mk-error" role="alert">{error}</p>}

              <nav className="mk-nav">
                {d.step > 0 ? <button type="button" className="mk-ghost" onClick={() => go(d.step - 1)}>{t('nav.back')}</button> : <span />}
                {step === 'review'
                  ? <button type="button" className="mk-primary" disabled={busy || !ready} onClick={submit}>
                      {busy ? t('nav.sending') : t(d.sent ? 'nav.save' : 'nav.send')}
                    </button>
                  : <button type="button" className="mk-primary" onClick={() => go(d.step + 1)}>
                      {t(step === 'intro' ? 'intro.start' : d.step === TOTAL - 1 ? 'nav.review' : 'nav.next')}
                    </button>}
              </nav>
            </section>
          )}
        </main>
      </div>
    </div>
  )
}

function Head({ id, headRef, children }: { id: Step; headRef: React.RefObject<HTMLHeadingElement | null>; children: ReactNode }) {
  const { t } = useTranslation('logo')
  return (
    <>
      <h1 id="mk-h" ref={headRef} tabIndex={-1}>{t(`${id}.title`, { brand: brand.name })}</h1>
      <p className="mk-lead">{t(`${id}.lead`)}</p>
      {children}
    </>
  )
}

/** La versión chica, en su tamaño real (32, 24 y 16 px), como en una barra de tareas. */
function Small({ kind, palette }: { kind: Proposal; palette: LogoPalette }) {
  const { t } = useTranslation('logo')
  return (
    <span className="lg-small" style={{ background: PALETTE_COLORS[palette].bg }} title={t('intro.smallTitle')}>
      {[32, 24, 16].map(s => <Mark key={s} kind={kind} palette={palette} size={s} small />)}
    </span>
  )
}

/** Propuestas para elegir una. `allowNone` agrega «Ninguna» (deja el valor vacío). */
function Cards({ palette, value, options, onPick, allowNone }: { palette: LogoPalette; value: Proposal | null; options: readonly Proposal[]; onPick: (v: Proposal | null) => void; allowNone?: boolean }) {
  const { t } = useTranslation('logo')
  return (
    <div className="lg-cards" role="radiogroup" aria-labelledby="mk-h">
      {options.map(v => (
        <button key={v} type="button" role="radio" className="mk-card lg-card" aria-checked={value === v} onClick={() => onPick(v)}>
          <span className="lg-stage" style={{ background: PALETTE_COLORS[palette].bg }}>
            <Mark kind={v} palette={palette} size="100%" />
            {v === 'enso' && <span className="lg-wordmark" style={{ color: PALETTE_COLORS[palette].ink }}>{brand.name}</span>}
          </span>
          <b>{t(`proposals.${v}.name`)}</b>
          <small>{t(`proposals.${v}.desc`)}</small>
          <Small kind={v} palette={palette} />
        </button>
      ))}
      {allowNone && (
        <button type="button" role="radio" className="mk-word ui-none" aria-checked={value === null} onClick={() => onPick(null)}>{t('second.none')}</button>
      )}
    </div>
  )
}

function Ratings({ palette, value, onChange }: { palette: LogoPalette; value: Partial<Record<Proposal, number>>; onChange: (r: Partial<Record<Proposal, number>>) => void }) {
  const { t } = useTranslation('logo')
  const [open, setOpen] = useState(Object.keys(value).length > 0)
  if (!open) return (
    <div className="ui-ratings-ask">
      <button type="button" className="mk-ghost" aria-expanded={false} onClick={() => setOpen(true)}>{t('ratings.open')}</button>
      <p className="mk-hint">{t('ratings.skip')}</p>
    </div>
  )
  const setOne = (v: Proposal, n: number) => {
    const next = { ...value }
    if (next[v] === n) delete next[v]; else next[v] = n
    onChange(next)
  }
  return (
    <div className="ui-ratings">
      {PROPOSALS.map(v => {
        const label = t('ratings.label', { name: t(`proposals.${v}.name`) })
        return (
          <div key={v} className="ui-rate lg-rate">
            <span className="lg-rate-mark" style={{ background: PALETTE_COLORS[palette].bg }}><Mark kind={v} palette={palette} size="100%" /></span>
            <span className="ui-rate-name"><b>{t(`proposals.${v}.name`)}</b></span>
            <span className="ui-scale" role="radiogroup" aria-label={label}>
              {[1, 2, 3, 4, 5].map(n => (
                <button key={n} type="button" role="radio" aria-checked={value[v] === n} title={value[v] === n ? t('ratings.clear') : t('ratings.star', { n })}
                  aria-label={t('ratings.star', { n })} onClick={() => setOne(v, n)}>{n}</button>
              ))}
            </span>
          </div>
        )
      })}
    </div>
  )
}

function Review({ d, onChange }: { d: Draft; onChange: (step: number) => void }) {
  const { t } = useTranslation('logo')
  const a = d.answers
  const none = <i className="mk-muted">{t('review.skipped')}</i>
  const rated = PROPOSALS.filter(v => a.ratings[v])
  const rows: [Exclude<Step, 'intro' | 'review'>, ReactNode][] = [
    ['pick', a.pick ? t(`proposals.${a.pick}.name`) : none],
    ['second', a.second ? t(`proposals.${a.second}.name`) : none],
    ['palette', a.palette ? t(`palettes.${a.palette}.name`) : none],
    ['ratings', rated.length ? rated.map(v => `${t(`proposals.${v}.name`)} ${a.ratings[v]}/5`).join(' · ') : none],
    ['comment', a.comment.trim() ? <p>{a.comment}</p> : none],
  ]
  return (
    <dl className="mk-review">
      {rows.map(([k, v]) => (
        <div key={k}>
          <dt>{t(`review.${k}`)}</dt>
          <dd>{v}</dd>
          <button type="button" className="mk-flat" onClick={() => onChange(STEPS.indexOf(k))}>{t('nav.change')}</button>
        </div>
      ))}
    </dl>
  )
}
