import { useEffect, useRef, useState, type ReactNode } from 'react'
import { useTranslation } from 'react-i18next'
import brand from '@brand'
import { applyTheme, currentTheme, type Theme } from '../theme'
import { setLang } from '../i18n'
import { loadDraft, nameOk, saveDraft, sendSurvey, type Sent } from '../surveyClient'
import { live, MAX_CHANGE, MAX_MIX, shot, VERSIONS, type V } from './versions'
import '../marca/marca.css'
import './interfaz.css'

const STEPS = ['intro', 'pick', 'second', 'change', 'mix', 'ratings', 'review'] as const
type Step = typeof STEPS[number]
const TOTAL = STEPS.length - 1

interface Answers {
  pick: V | null
  second: V | null
  change: string
  mixFrom: V | null
  mixPart: string
  ratings: Partial<Record<V, number>>
}
interface Draft { step: number; name: string; answers: Answers; sent: Sent | null; done: boolean }

const KEY = 'interfaz-1'
const empty = (): Draft => ({
  step: 0, name: '', sent: null, done: false,
  answers: { pick: null, second: null, change: '', mixFrom: null, mixPart: '', ratings: {} },
})
const isV = (v: string | null): v is V => !!v && (VERSIONS as readonly string[]).includes(v)

/** Borrador guardado; si se llega desde la vista previa (`?v=c`) y todavía no hay elegida, viene marcada. */
function load(): Draft {
  const d = loadDraft(KEY, empty)
  const v = new URLSearchParams(location.search).get('v')
  if (isV(v) && !d.answers.pick) {
    d.answers.pick = v
    if (d.answers.second === v) d.answers.second = null
    if (d.answers.mixFrom === v) d.answers.mixFrom = null
  }
  return d
}

function answered(s: Step, d: Draft): boolean {
  const a = d.answers
  switch (s) {
    case 'intro': return d.step > 0 || !!a.pick
    case 'pick': return !!a.pick
    case 'second': return !!a.second
    case 'change': return !!a.change.trim()
    case 'mix': return !!(a.mixFrom || a.mixPart.trim())
    case 'ratings': return Object.keys(a.ratings).length > 0
    case 'review': return !!d.sent
  }
}

/** Encuesta pública de la interfaz: `/interfaz`. Cuál de las 5 versiones de la app, la segunda, qué cambiar y qué mezclar. */
export default function InterfazApp() {
  const { t, i18n } = useTranslation('interfaz')
  const [d, setD] = useState<Draft>(load)
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

  useEffect(() => { saveDraft(KEY, d) }, [d])
  useEffect(() => { document.title = t('meta.title', { brand: brand.name }) }, [t, lang])
  // El ?v= de la vista previa ya se usó: se quita para que una recarga no lo vuelva a aplicar.
  useEffect(() => { if (location.search) history.replaceState(null, '', location.pathname) }, [])
  useEffect(() => {
    if (first.current) { first.current = false; return }
    head.current?.focus({ preventScroll: true })
    window.scrollTo({ top: 0 })
  }, [d.step, d.done])

  const set = (patch: Partial<Answers>) => setD(x => ({ ...x, answers: { ...x.answers, ...patch } }))
  const go = (n: number) => {
    setError(''); setNotice('')
    const target = Math.max(0, Math.min(TOTAL, n))
    // Sin elegida no se pasa del paso de elegir.
    if (target > 1 && !d.answers.pick) { setError(t('pick.required')); setD(x => ({ ...x, step: 1, done: false })); return }
    setD(x => ({ ...x, step: target, done: false }))
  }
  const choosePick = (v: V) => set({
    pick: v,
    second: a.second === v ? null : a.second,
    mixFrom: a.mixFrom === v ? null : a.mixFrom,
  })

  const submit = async () => {
    setTouchedName(true)
    if (!nameOk(d.name)) { setError(t('errors.invalid_name')); return }
    setBusy(true); setError(''); setNotice('')
    try {
      const created = await sendSurvey('interfaz', { name: d.name, lang, answers: d.answers, website }, d.sent)
      setD(x => ({ ...x, sent: created ?? x.sent, done: true }))
      if (!created) setNotice(t('done.saved'))
    } catch (e) {
      const code = e instanceof Error ? e.message : 'generic'
      if (code === 'not_found') setD(x => ({ ...x, sent: null }))
      setError(t(`errors.${code}`, t('errors.generic')))
    } finally { setBusy(false) }
  }

  const toggleTheme = () => { const next: Theme = theme === 'dark' ? 'light' : 'dark'; applyTheme(next); setTheme(next) }
  const name = (v: string) => (t(`versions.items.${v}`, { returnObjects: true }) as string[])[0]
  const letter = (v: string) => v.toUpperCase()

  return (
    <div className="mk ui">
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
                  <button type="button" className="mk-stepbtn" aria-current={current ? 'step' : undefined} disabled={i > 1 && !a.pick} onClick={() => go(i)}>
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
              <span className="mk-done-mark" aria-hidden="true">芯</span>
              <h1 ref={head} tabIndex={-1}>{t('done.title', { name: d.name })}</h1>
              {notice && <p className="mk-note" role="status">{notice}</p>}
              <p className="mk-lead">{t('done.text')}</p>
              <p><a className="ui-link" href="/marca">{t('done.brand')}</a></p>
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
                  <ul className="ui-gallery">
                    {VERSIONS.map(v => (
                      <li key={v} className="ui-version">
                        <a href={live(v)} target="_blank" rel="noreferrer" className="ui-shot-link" aria-label={`${t('versions.live')}: ${t('versions.letter', { letter: letter(v) })}`}>
                          <img src={shot(v, theme)} width={1200} height={781} loading="lazy" alt={t('versions.alt', { letter: letter(v), name: name(v) })} />
                        </a>
                        <span className="ui-letter">{t('versions.letter', { letter: letter(v) })}</span>
                        <b>{name(v)}</b>
                        <small>{(t(`versions.items.${v}`, { returnObjects: true }) as string[])[1]}</small>
                        <a className="ui-link" href={live(v)} target="_blank" rel="noreferrer">{t('versions.live')}</a>
                      </li>
                    ))}
                  </ul>
                  <p className="mk-hint">{t('versions.liveHint')}</p>
                  {/* Campo trampa para bots: oculto a la vista y a los lectores de pantalla. */}
                  <input className="mk-trap" tabIndex={-1} autoComplete="off" aria-hidden="true" name="website" value={website} onChange={e => setWebsite(e.target.value)} />
                </>
              )}

              {step === 'pick' && (
                <Head id="pick" headRef={head}>
                  <Cards theme={theme} value={a.pick} options={VERSIONS} onPick={v => v && choosePick(v)} />
                </Head>
              )}

              {step === 'second' && (
                <Head id="second" headRef={head}>
                  <Cards theme={theme} value={a.second} options={VERSIONS.filter(v => v !== a.pick)} onPick={v => set({ second: v })} allowNone />
                </Head>
              )}

              {step === 'change' && a.pick && (
                <Head id="change" headRef={head} vars={{ letter: letter(a.pick) }}>
                  <div className="ui-chosen">
                    <img src={shot(a.pick, theme)} width={1200} height={781} alt={t('versions.alt', { letter: letter(a.pick), name: name(a.pick) })} />
                    <span><b>{t('versions.letter', { letter: letter(a.pick) })}</b> {name(a.pick)}<br />
                      <a className="ui-link" href={live(a.pick)} target="_blank" rel="noreferrer">{t('versions.live')}</a></span>
                  </div>
                  <label className="mk-field">
                    <span>{t('change.label')}</span>
                    <textarea rows={5} maxLength={MAX_CHANGE} value={a.change} onChange={e => set({ change: e.target.value })} />
                    <small className="mk-count">{t('change.count', { n: a.change.length, max: MAX_CHANGE })}</small>
                  </label>
                </Head>
              )}

              {step === 'mix' && (
                <Head id="mix" headRef={head}>
                  <fieldset className="ui-fieldset">
                    <legend>{t('mix.from')}</legend>
                    <div className="ui-letters" role="radiogroup" aria-label={t('mix.from')}>
                      {VERSIONS.filter(v => v !== a.pick).map(v => (
                        <button key={v} type="button" role="radio" className="mk-word ui-mini" aria-checked={a.mixFrom === v}
                          onClick={() => set({ mixFrom: a.mixFrom === v ? null : v })}>
                          <img src={shot(v, theme)} width={1200} height={781} alt="" />
                          <span><b>{letter(v)}</b> {name(v)}</span>
                        </button>
                      ))}
                    </div>
                  </fieldset>
                  <label className="mk-field">
                    <span>{t('mix.part')}</span>
                    <textarea rows={3} maxLength={MAX_MIX} value={a.mixPart} onChange={e => set({ mixPart: e.target.value })} />
                    <small className="mk-count">{t('mix.count', { n: a.mixPart.length, max: MAX_MIX })}</small>
                  </label>
                </Head>
              )}

              {step === 'ratings' && (
                <Head id="ratings" headRef={head}>
                  <Ratings theme={theme} value={a.ratings} onChange={ratings => set({ ratings })} />
                </Head>
              )}

              {step === 'review' && (
                <Head id="review" headRef={head}>
                  <label className="mk-field">
                    <span>{t('review.name')}</span>
                    <small id="ui-name-hint">{t('review.nameHint')}</small>
                    <input value={d.name} maxLength={60} autoComplete="nickname" aria-describedby="ui-name-hint" aria-invalid={touchedName && !nameOk(d.name)}
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
                  ? <button type="button" className="mk-primary" disabled={busy || !a.pick} onClick={submit}>
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

function Head({ id, headRef, children, vars }: { id: Step; headRef: React.RefObject<HTMLHeadingElement | null>; children: ReactNode; vars?: Record<string, string> }) {
  const { t } = useTranslation('interfaz')
  return (
    <>
      <h1 id="mk-h" ref={headRef} tabIndex={-1}>{t(`${id}.title`, { brand: brand.name, ...vars })}</h1>
      <p className="mk-lead">{t(`${id}.lead`)}</p>
      {children}
    </>
  )
}

/** Versiones para elegir una, con su captura. `allowNone` agrega «Ninguna» (deja el valor vacío). */
function Cards({ theme, value, options, onPick, allowNone }: { theme: Theme; value: V | null; options: readonly V[]; onPick: (v: V | null) => void; allowNone?: boolean }) {
  const { t } = useTranslation('interfaz')
  return (
    <div className="ui-cards" role="radiogroup" aria-labelledby="mk-h">
      {options.map(v => {
        const [name, desc] = t(`versions.items.${v}`, { returnObjects: true }) as string[]
        return (
          <div key={v} className="ui-card-wrap">
            <button type="button" role="radio" className="mk-card ui-card" aria-checked={value === v} onClick={() => onPick(v)}>
              <img src={shot(v, theme)} width={1200} height={781} alt="" />
              <span className="ui-letter">{t('versions.letter', { letter: v.toUpperCase() })}</span>
              <b>{name}</b>
              <small>{desc}</small>
            </button>
            <a className="ui-link ui-card-live" href={live(v)} target="_blank" rel="noreferrer">{t('versions.live')}</a>
          </div>
        )
      })}
      {allowNone && (
        <button type="button" role="radio" className="mk-word ui-none" aria-checked={value === null} onClick={() => onPick(null)}>{t('second.none')}</button>
      )}
    </div>
  )
}

function Ratings({ theme, value, onChange }: { theme: Theme; value: Partial<Record<V, number>>; onChange: (r: Partial<Record<V, number>>) => void }) {
  const { t } = useTranslation('interfaz')
  const [open, setOpen] = useState(Object.keys(value).length > 0)
  if (!open) return (
    <div className="ui-ratings-ask">
      <button type="button" className="mk-ghost" aria-expanded={false} onClick={() => setOpen(true)}>{t('ratings.open')}</button>
      <p className="mk-hint">{t('ratings.skip')}</p>
    </div>
  )
  const setOne = (v: V, n: number) => {
    const next = { ...value }
    if (next[v] === n) delete next[v]; else next[v] = n
    onChange(next)
  }
  return (
    <div className="ui-ratings">
      {VERSIONS.map(v => {
        const label = t('ratings.label', { letter: v.toUpperCase() })
        return (
          <div key={v} className="ui-rate">
            <img src={shot(v, theme)} width={1200} height={781} alt="" />
            <span className="ui-rate-name"><b>{v.toUpperCase()}</b> {(t(`versions.items.${v}`, { returnObjects: true }) as string[])[0]}</span>
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
  const { t } = useTranslation('interfaz')
  const a = d.answers
  const none = <i className="mk-muted">{t('review.skipped')}</i>
  const ver = (v: V) => `${t('versions.letter', { letter: v.toUpperCase() })}: ${(t(`versions.items.${v}`, { returnObjects: true }) as string[])[0]}`
  const rated = VERSIONS.filter(v => a.ratings[v])
  const rows: [Exclude<Step, 'intro' | 'review'>, ReactNode][] = [
    ['pick', a.pick ? ver(a.pick) : none],
    ['second', a.second ? ver(a.second) : none],
    ['change', a.change.trim() ? <p>{a.change}</p> : none],
    ['mix', a.mixFrom && a.mixPart.trim() ? t('review.mixValue', { letter: a.mixFrom.toUpperCase(), part: a.mixPart })
      : a.mixFrom ? t('review.mixOnly', { letter: a.mixFrom.toUpperCase() }) : a.mixPart.trim() ? <p>{a.mixPart}</p> : none],
    ['ratings', rated.length ? rated.map(v => `${v.toUpperCase()} ${a.ratings[v]}/5`).join(' · ') : none],
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
