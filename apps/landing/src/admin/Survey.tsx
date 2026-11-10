import { useCallback, useEffect, useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'
import brand from '@brand'
import { api, ApiError, type SurveyAnswers, type SurveyResponse } from './api'
import { formatDate } from './status'

/* Mismas listas que la encuesta (apps/landing/src/marca) y el servidor (survey.rs). */
const AXES = ['serio_jugueton', 'tecnico_artistico', 'simple_detallado', 'tranquilo_energico', 'japones_evidente_sutil']
const WORDS = ['liviano', 'nocturno', 'vivo', 'tranquilo', 'preciso', 'calido', 'futurista', 'artesanal', 'libre', 'elegante', 'jugueton', 'tecnico']
const SYMBOLS = ['kanji', 'mecha', 'monitor', 'onda', 'abstracto', 'letras']
const LOGO_TYPES = ['simbolo', 'simbolo_nombre', 'nombre', 'sello']
const STYLE_PAIRS = ['geometrico_pincel', 'fina_rellena', 'plano_volumen', 'redondo_anguloso', 'simetrico_organico']
const PALETTES = ['actual', 'hora_azul', 'sello', 'amanecer', 'mono']
const SMALL = ['simple', 'medio', 'detallado']

/** Resultados de la encuesta de la marca (ADMIN o más): resumen por pregunta, respuestas por persona y descarga. */
export function Survey() {
  const { t, i18n } = useTranslation('admin')
  const m = useTranslation('marca').t
  const lang = i18n.resolvedLanguage ?? 'es'
  const [rows, setRows] = useState<SurveyResponse[] | null>(null)
  const [error, setError] = useState('')
  const [open, setOpen] = useState<string | null>(null)

  const load = useCallback(() => {
    api.surveyResponses().then(setRows).catch(e => setError(e instanceof ApiError ? e.code : 'generic'))
  }, [])
  useEffect(load, [load])

  const first = (k: string) => (m(k, { returnObjects: true }) as string[])[0]
  const pairOf = (k: string) => m(k, { returnObjects: true }) as string[]

  const remove = async (r: SurveyResponse) => {
    if (!confirm(t('survey.confirmRemove', { name: r.name }))) return
    try { await api.deleteSurveyResponse(r.id); load() } catch (e) { setError(e instanceof ApiError ? e.code : 'generic') }
  }

  const download = (kind: 'json' | 'csv') => {
    if (!rows) return
    const text = kind === 'json' ? JSON.stringify(rows, null, 2) : toCsv(rows)
    const url = URL.createObjectURL(new Blob([kind === 'csv' ? '﻿' + text : text], { type: kind === 'json' ? 'application/json' : 'text/csv' }))
    const a = Object.assign(document.createElement('a'), { href: url, download: `${brand.slug}-encuesta-marca.${kind}` })
    a.click()
    URL.revokeObjectURL(url)
  }

  const count = useMemo(() => {
    const all = rows?.map(r => r.answers) ?? []
    const tally = (pick: (a: SurveyAnswers) => string[], ids: string[]) => {
      const c = Object.fromEntries(ids.map(id => [id, 0])) as Record<string, number>
      all.forEach(a => pick(a).forEach(v => { if (v in c) c[v]++ }))
      return ids.map(id => ({ id, n: c[id] })).sort((x, y) => y.n - x.n)
    }
    return {
      words: tally(a => a.words ?? [], WORDS),
      symbols: tally(a => a.symbols ?? [], SYMBOLS),
      logoType: tally(a => (a.logoType ? [a.logoType] : []), LOGO_TYPES),
      palette: tally(a => (a.palette ? [a.palette] : []), PALETTES),
      small: tally(a => (a.small ? [a.small] : []), SMALL),
      axes: AXES.map(ax => {
        const vals = all.map(a => a.personality?.[ax]).filter((v): v is number => typeof v === 'number')
        const dist = [-2, -1, 0, 1, 2].map(p => vals.filter(v => v === p).length)
        return { ax, dist, n: vals.length, avg: vals.length ? vals.reduce((s, v) => s + v, 0) / vals.length : null }
      }),
      styles: STYLE_PAIRS.map(p => {
        const vals = all.map(a => a.styles?.[p]).filter(Boolean)
        return { p, a: vals.filter(v => v === 'a').length, same: vals.filter(v => v === 'igual').length, b: vals.filter(v => v === 'b').length }
      }),
    }
  }, [rows])

  if (error) return <section className="adm-doc"><h1>{t('survey.title')}</h1><p className="adm-note">{t(`errors.${error}`, t('errors.generic'))}</p></section>
  if (!rows) return <section className="adm-doc"><p role="status">{t('loading')}</p></section>

  const total = rows.length
  const texts = rows.filter(r => r.answers.avoid || r.answers.comment)

  return (
    <section className="adm-doc adm-wide sv">
      <h1>{t('survey.title')}</h1>
      <p className="adm-muted">{t('survey.intro')} <a href="/marca" target="_blank" rel="noreferrer">{location.host}/marca</a></p>
      <p className="sv-total"><b>{total}</b> {t('survey.count', { count: total })}</p>
      {total > 0 && (
        <p className="sv-actions">
          <button type="button" className="ms-chip" onClick={() => download('csv')}>{t('survey.csv')}</button>
          <button type="button" className="ms-chip" onClick={() => download('json')}>{t('survey.json')}</button>
        </p>
      )}

      {total === 0 ? <p className="adm-note">{t('survey.empty')}</p> : (
        <>
          <h2>{t('survey.summary')}</h2>

          <h3>{m('personality.title', { brand: brand.name })}</h3>
          <div className="sv-axes">
            {count.axes.map(({ ax, dist, n, avg }) => {
              const [l, r] = pairOf(`personality.axes.${ax}`)
              const max = Math.max(1, ...dist)
              return (
                <div key={ax} className="sv-axis">
                  <span className="sv-axis-l">{l}</span>
                  <span className="sv-dist" role="img" aria-label={t('survey.distLabel', { left: l, right: r, values: dist.join(', '), n })}>
                    {dist.map((v, i) => (
                      <span key={i} className="sv-col" title={`${(m('personality.pos', { returnObjects: true, a: l, b: r }) as string[])[i]}: ${v}`}>
                        <i style={{ height: `${(v / max) * 100}%` }} /><small>{v}</small>
                      </span>
                    ))}
                  </span>
                  <span className="sv-axis-r">{r}</span>
                  <span className="sv-avg">{avg === null ? '—' : t('survey.avg', { value: avg.toLocaleString(lang, { maximumFractionDigits: 1, signDisplay: 'exceptZero' }) })}</span>
                </div>
              )
            })}
          </div>
          <p className="adm-muted sv-foot">{t('survey.axisHelp')}</p>

          <Bars title={m('words.title')} items={count.words.map(x => ({ label: m(`words.items.${x.id}`), n: x.n }))} total={total} t={t} />
          <Bars title={m('symbols.title')} items={count.symbols.map(x => ({ label: first(`symbols.items.${x.id}`), n: x.n }))} total={total} t={t} />
          <Bars title={m('logoType.title')} items={count.logoType.map(x => ({ label: first(`logoType.items.${x.id}`), n: x.n }))} total={total} t={t} />

          <h3>{m('styles.title')}</h3>
          <div className="adm-table"><table>
            <thead><tr><th>{t('survey.left')}</th><th>{m('styles.same')}</th><th>{t('survey.right')}</th></tr></thead>
            <tbody>{count.styles.map(({ p, a, same, b }) => {
              const [la, lb] = pairOf(`styles.pairs.${p}`)
              return <tr key={p}><td className={a > b ? 'sv-win' : ''}>{la}: {a}</td><td>{same}</td><td className={b > a ? 'sv-win' : ''}>{lb}: {b}</td></tr>
            })}</tbody>
          </table></div>

          <Bars title={m('palette.title')} items={count.palette.map(x => ({ label: first(`palette.items.${x.id}`), n: x.n }))} total={total} t={t} />
          <Bars title={m('small.title')} items={count.small.map(x => ({ label: m(`small.items.${x.id}`), n: x.n }))} total={total} t={t} />

          <h3>{m('free.title')}</h3>
          {texts.length === 0 ? <p className="adm-muted">{t('survey.noTexts')}</p> : (
            <ul className="sv-texts">{texts.map(r => (
              <li key={r.id}><b>{r.name}</b>
                {r.answers.avoid && <p><span className="adm-muted">{t('survey.avoid')}</span> {r.answers.avoid}</p>}
                {r.answers.comment && <p><span className="adm-muted">{t('survey.comment')}</span> {r.answers.comment}</p>}
              </li>
            ))}</ul>
          )}

          <h2>{t('survey.people')}</h2>
          <div className="adm-table"><table>
            <thead><tr><th>{t('survey.name')}</th><th>{t('survey.date')}</th><th><span className="adm-sr">{t('survey.actions')}</span></th></tr></thead>
            <tbody>{rows.map(r => (
              <PersonRow key={r.id} r={r} open={open === r.id} onToggle={() => setOpen(open === r.id ? null : r.id)} onRemove={() => remove(r)} lang={lang} />
            ))}</tbody>
          </table></div>
        </>
      )}
    </section>
  )
}

function Bars({ title, items, total, t }: { title: string; items: { label: string; n: number }[]; total: number; t: (k: string, o?: Record<string, unknown>) => string }) {
  const max = Math.max(1, ...items.map(i => i.n))
  return (
    <>
      <h3>{title}</h3>
      <ul className="sv-bars">
        {items.map(i => (
          <li key={i.label} title={t('survey.ofTotal', { n: i.n, total })}>
            <span className="sv-label">{i.label}</span>
            <span className="sv-track"><i style={{ width: `${(i.n / max) * 100}%` }} /></span>
            <span className="sv-n">{i.n}</span>
          </li>
        ))}
      </ul>
    </>
  )
}

function PersonRow({ r, open, onToggle, onRemove, lang }: { r: SurveyResponse; open: boolean; onToggle: () => void; onRemove: () => void; lang: string }) {
  const { t } = useTranslation('admin')
  const m = useTranslation('marca').t
  const a = r.answers
  const first = (k: string) => (m(k, { returnObjects: true }) as string[])[0]
  const pair = (k: string) => m(k, { returnObjects: true }) as string[]
  const dash = '—'
  return (
    <>
      <tr>
        <td><b>{r.name}</b>{r.lang === 'en' && <span className="adm-muted"> (en)</span>}</td>
        <td>{formatDate(r.createdAt, lang)}{r.updatedAt !== r.createdAt && <span className="adm-muted"> · {t('survey.edited', { date: formatDate(r.updatedAt, lang) })}</span>}</td>
        <td className="sv-row-actions">
          <button type="button" className="adm-flat" aria-expanded={open} onClick={onToggle}>{t(open ? 'survey.hide' : 'survey.show')}</button>
          <button type="button" className="adm-flat adm-danger" onClick={onRemove}>{t('survey.remove')}</button>
        </td>
      </tr>
      {open && (
        <tr className="sv-detail"><td colSpan={3}>
          <dl>
            <dt>{m('personality.title', { brand: brand.name })}</dt>
            <dd>{AXES.filter(ax => ax in (a.personality ?? {})).map(ax => {
              const [l, rr] = pair(`personality.axes.${ax}`)
              return (m('personality.pos', { returnObjects: true, a: l, b: rr }) as string[])[(a.personality?.[ax] ?? 0) + 2]
            }).join(' · ') || dash}</dd>
            <dt>{m('words.title')}</dt><dd>{a.words?.map(w => m(`words.items.${w}`)).join(', ') || dash}</dd>
            <dt>{m('symbols.title')}</dt><dd>{a.symbols?.map(s => first(`symbols.items.${s}`)).join(', ') || dash}</dd>
            <dt>{m('logoType.title')}</dt><dd>{a.logoType ? first(`logoType.items.${a.logoType}`) : dash}</dd>
            <dt>{m('styles.title')}</dt>
            <dd>{STYLE_PAIRS.filter(p => a.styles?.[p]).map(p => {
              const [la, lb] = pair(`styles.pairs.${p}`), v = a.styles![p]
              return v === 'a' ? la : v === 'b' ? lb : `${la}/${lb}: ${m('styles.same')}`
            }).join(' · ') || dash}</dd>
            <dt>{m('palette.title')}</dt><dd>{a.palette ? first(`palette.items.${a.palette}`) : dash}</dd>
            <dt>{m('small.title')}</dt><dd>{a.small ? m(`small.items.${a.small}`) : dash}</dd>
          </dl>
        </td></tr>
      )}
    </>
  )
}

/** CSV con una columna por pregunta (los ejes y pares, una cada uno). Valores con su id, para poder cruzarlos. */
function toCsv(rows: SurveyResponse[]): string {
  const head = ['nombre', 'idioma', 'fecha', 'editada', ...AXES.map(a => `personalidad_${a}`), 'palabras', 'simbolos', 'tipo_logo',
    ...STYLE_PAIRS.map(p => `estilo_${p}`), 'paleta', 'tamano_chico', 'evitar', 'comentario']
  const esc = (v: unknown) => { const s = v === undefined || v === null ? '' : String(v); return /[",\n;]/.test(s) ? `"${s.replace(/"/g, '""')}"` : s }
  const lines = rows.map(r => {
    const a = r.answers
    return [r.name, r.lang, r.createdAt, r.updatedAt, ...AXES.map(ax => a.personality?.[ax]), (a.words ?? []).join('|'), (a.symbols ?? []).join('|'),
      a.logoType, ...STYLE_PAIRS.map(p => a.styles?.[p]), a.palette, a.small, a.avoid, a.comment].map(esc).join(',')
  })
  return [head.join(','), ...lines].join('\n')
}
