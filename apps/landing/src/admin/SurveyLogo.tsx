import { Fragment, useCallback, useEffect, useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { api, ApiError, type LogoAnswers, type SurveyResponse } from './api'
import { formatDate } from './status'
import { Bars, csvCell, downloadRows } from './Survey'
import { LOGO_PALETTES, PROPOSALS, SHEET } from '../logo/proposals'

type Row = SurveyResponse<LogoAnswers>

/** Resultados de la votación del logo (ronda 2 de la marca, ADMIN o más): puntos, elegida, segunda, paleta, notas y comentarios. */
export function SurveyLogo() {
  const { t, i18n } = useTranslation('admin')
  const g = useTranslation('logo').t
  const lang = i18n.resolvedLanguage ?? 'es'
  const [rows, setRows] = useState<Row[] | null>(null)
  const [error, setError] = useState('')
  const [open, setOpen] = useState<string | null>(null)

  const load = useCallback(() => {
    api.surveyResponses<LogoAnswers>('logo').then(setRows).catch(e => setError(e instanceof ApiError ? e.code : 'generic'))
  }, [])
  useEffect(load, [load])

  const prop = useCallback((v: string) => g(`proposals.${v}.name`), [g])
  const pal = useCallback((p: string) => g(`palettes.${p}.name`), [g])

  const remove = async (r: Row) => {
    if (!confirm(t('survey.confirmRemove', { name: r.name }))) return
    try { await api.deleteSurveyResponse(r.id, 'logo'); load() } catch (e) { setError(e instanceof ApiError ? e.code : 'generic') }
  }

  const sum = useMemo(() => {
    const all = rows?.map(r => r.answers) ?? []
    const count = <T extends string>(list: readonly T[], label: (v: T) => string, pick: (a: LogoAnswers) => string | null | undefined) =>
      list.map(v => ({ label: label(v), n: all.filter(a => pick(a) === v).length })).sort((x, y) => y.n - x.n)
    return {
      points: PROPOSALS.map(v => ({ label: prop(v), n: all.reduce((s, a) => s + (a.pick === v ? 2 : 0) + (a.second === v ? 1 : 0), 0) }))
        .sort((x, y) => y.n - x.n),
      pick: count(PROPOSALS, prop, a => a.pick),
      second: count(PROPOSALS, prop, a => a.second),
      palette: count(LOGO_PALETTES, pal, a => a.palette),
      combo: PROPOSALS.flatMap(v => LOGO_PALETTES.map(p => ({ label: `${prop(v)} · ${pal(p)}`, n: all.filter(a => a.pick === v && a.palette === p).length })))
        .filter(x => x.n > 0).sort((x, y) => y.n - x.n),
      ratings: PROPOSALS.map(v => {
        const vals = all.map(a => a.ratings?.[v]).filter((n): n is number => typeof n === 'number')
        return { v, n: vals.length, avg: vals.length ? vals.reduce((s, n) => s + n, 0) / vals.length : null }
      }),
    }
  }, [rows, prop, pal])

  if (error) return <section className="adm-doc"><h1>{t('surveyLogo.title')}</h1><p className="adm-note">{t(`errors.${error}`, t('errors.generic'))}</p></section>
  if (!rows) return <section className="adm-doc"><p role="status">{t('loading')}</p></section>

  const total = rows.length
  const comments = rows.filter(r => r.answers.comment?.trim())

  return (
    <section className="adm-doc adm-wide sv">
      <h1>{t('surveyLogo.title')}</h1>
      <p className="adm-muted">{t('surveyLogo.intro')} <a href="/marca" target="_blank" rel="noreferrer">{location.host}/marca</a> · <a href={SHEET} target="_blank" rel="noreferrer">{t('surveyLogo.sheet')}</a></p>
      <p className="sv-total"><b>{total}</b> {t('survey.count', { count: total })}</p>
      {total > 0 && (
        <p className="sv-actions">
          <button type="button" className="ms-chip" onClick={() => downloadRows(rows, 'csv', 'logo', toCsv)}>{t('survey.csv')}</button>
          <button type="button" className="ms-chip" onClick={() => downloadRows(rows, 'json', 'logo', toCsv)}>{t('survey.json')}</button>
        </p>
      )}

      {total === 0 ? <p className="adm-note">{t('surveyLogo.empty')}</p> : (
        <>
          <h2>{t('survey.summary')}</h2>
          <Bars title={t('surveyLogo.points')} items={sum.points} total={total} t={t} />
          <p className="adm-muted sv-foot">{t('surveyLogo.pointsHelp')}</p>
          <Bars title={g('pick.title')} items={sum.pick} total={total} t={t} />
          <Bars title={g('second.title')} items={sum.second} total={total} t={t} />
          <Bars title={g('palette.title')} items={sum.palette} total={total} t={t} />
          <Bars title={t('surveyLogo.byPalette')} items={sum.combo} total={total} t={t} />

          <h3>{t('surveyLogo.ratings')}</h3>
          <div className="adm-table"><table>
            <thead><tr><th>{t('surveyLogo.proposal')}</th><th>{t('surveyUi.avg')}</th><th>{t('surveyUi.votes')}</th></tr></thead>
            <tbody>{sum.ratings.map(({ v, n, avg }) => (
              <tr key={v}><td>{prop(v)}</td><td>{avg === null ? '—' : avg.toLocaleString(lang, { maximumFractionDigits: 1 })}</td><td>{n}</td></tr>
            ))}</tbody>
          </table></div>

          <h3>{t('surveyLogo.comments')}</h3>
          {comments.length === 0 ? <p className="adm-muted">{t('survey.noTexts')}</p> : (
            <ul className="sv-texts">{comments.map(r => (
              <li key={r.id}><b>{r.name}</b> <span className="adm-muted">({r.answers.pick ? prop(r.answers.pick) : '—'})</span><p>{r.answers.comment}</p></li>
            ))}</ul>
          )}

          <h2>{t('survey.people')}</h2>
          <div className="adm-table"><table>
            <thead><tr><th>{t('survey.name')}</th><th>{g('review.pick')}</th><th>{g('review.palette')}</th><th>{t('survey.date')}</th><th><span className="adm-sr">{t('survey.actions')}</span></th></tr></thead>
            <tbody>{rows.map(r => {
              const a = r.answers, isOpen = open === r.id
              const rated = PROPOSALS.filter(v => a.ratings?.[v])
              return (
                <Fragment key={r.id}>
                  <tr>
                    <td><b>{r.name}</b>{r.lang === 'en' && <span className="adm-muted"> (en)</span>}</td>
                    <td>{a.pick ? prop(a.pick) : '—'}{a.second && <span className="adm-muted"> · {prop(a.second)}</span>}</td>
                    <td>{a.palette ? pal(a.palette) : '—'}</td>
                    <td>{formatDate(r.createdAt, lang)}{r.updatedAt !== r.createdAt && <span className="adm-muted"> · {t('survey.edited', { date: formatDate(r.updatedAt, lang) })}</span>}</td>
                    <td className="sv-row-actions">
                      <button type="button" className="adm-flat" aria-expanded={isOpen} onClick={() => setOpen(isOpen ? null : r.id)}>{t(isOpen ? 'survey.hide' : 'survey.show')}</button>
                      <button type="button" className="adm-flat adm-danger" onClick={() => remove(r)}>{t('survey.remove')}</button>
                    </td>
                  </tr>
                  {isOpen && (
                    <tr className="sv-detail"><td colSpan={5}>
                      <dl>
                        <dt>{g('review.pick')}</dt><dd>{a.pick ? prop(a.pick) : '—'}</dd>
                        <dt>{g('review.second')}</dt><dd>{a.second ? prop(a.second) : '—'}</dd>
                        <dt>{g('review.palette')}</dt><dd>{a.palette ? pal(a.palette) : '—'}</dd>
                        <dt>{g('review.ratings')}</dt><dd>{rated.map(v => `${prop(v)} ${a.ratings![v]}/5`).join(' · ') || '—'}</dd>
                        <dt>{g('review.comment')}</dt><dd>{a.comment?.trim() || '—'}</dd>
                      </dl>
                    </td></tr>
                  )}
                </Fragment>
              )
            })}</tbody>
          </table></div>
        </>
      )}
    </section>
  )
}

/** CSV con una columna por pregunta y una por nota, con los identificadores (sello, enso…) para poder cruzarlos. */
function toCsv(rows: Row[]): string {
  const head = ['nombre', 'idioma', 'fecha', 'editada', 'elegida', 'segunda', 'paleta', ...PROPOSALS.map(v => `nota_${v}`), 'comentario']
  const lines = rows.map(r => {
    const a = r.answers
    return [r.name, r.lang, r.createdAt, r.updatedAt, a.pick, a.second, a.palette, ...PROPOSALS.map(v => a.ratings?.[v]), a.comment].map(csvCell).join(',')
  })
  return [head.join(','), ...lines].join('\n')
}
