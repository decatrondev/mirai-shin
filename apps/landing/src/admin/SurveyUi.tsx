import { Fragment, useCallback, useEffect, useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { api, ApiError, type SurveyResponse, type UiAnswers } from './api'
import { formatDate } from './status'
import { Bars, csvCell, downloadRows } from './Survey'
import { VERSIONS } from '../interfaz/versions'

type Row = SurveyResponse<UiAnswers>

/** Resultados de la encuesta de la interfaz (ADMIN o más): elegida, segunda, puntos, notas, mezclas y textos. */
export function SurveyUi() {
  const { t, i18n } = useTranslation('admin')
  const u = useTranslation('interfaz').t
  const lang = i18n.resolvedLanguage ?? 'es'
  const [rows, setRows] = useState<Row[] | null>(null)
  const [error, setError] = useState('')
  const [open, setOpen] = useState<string | null>(null)

  const load = useCallback(() => {
    api.surveyResponses<UiAnswers>('interfaz').then(setRows).catch(e => setError(e instanceof ApiError ? e.code : 'generic'))
  }, [])
  useEffect(load, [load])

  const label = useCallback((v: string) => `${v.toUpperCase()} · ${(u(`versions.items.${v}`, { returnObjects: true }) as string[])[0]}`, [u])

  const remove = async (r: Row) => {
    if (!confirm(t('survey.confirmRemove', { name: r.name }))) return
    try { await api.deleteSurveyResponse(r.id, 'interfaz'); load() } catch (e) { setError(e instanceof ApiError ? e.code : 'generic') }
  }

  const sum = useMemo(() => {
    const all = rows?.map(r => r.answers) ?? []
    const count = (pick: (a: UiAnswers) => string | null | undefined) =>
      VERSIONS.map(v => ({ label: label(v), n: all.filter(a => pick(a) === v).length })).sort((x, y) => y.n - x.n)
    return {
      pick: count(a => a.pick),
      second: count(a => a.second),
      points: VERSIONS.map(v => ({ label: label(v), n: all.reduce((s, a) => s + (a.pick === v ? 2 : 0) + (a.second === v ? 1 : 0), 0) }))
        .sort((x, y) => y.n - x.n),
      ratings: VERSIONS.map(v => {
        const vals = all.map(a => a.ratings?.[v]).filter((n): n is number => typeof n === 'number')
        return { v, n: vals.length, avg: vals.length ? vals.reduce((s, n) => s + n, 0) / vals.length : null }
      }),
    }
  }, [rows, label])

  if (error) return <section className="adm-doc"><h1>{t('surveyUi.title')}</h1><p className="adm-note">{t(`errors.${error}`, t('errors.generic'))}</p></section>
  if (!rows) return <section className="adm-doc"><p role="status">{t('loading')}</p></section>

  const total = rows.length
  const mixes = rows.filter(r => r.answers.mixFrom || r.answers.mixPart?.trim())
  const changes = rows.filter(r => r.answers.change?.trim())

  return (
    <section className="adm-doc adm-wide sv">
      <h1>{t('surveyUi.title')}</h1>
      <p className="adm-muted">{t('surveyUi.intro')} <a href="/interfaz" target="_blank" rel="noreferrer">{location.host}/interfaz</a></p>
      <p className="sv-total"><b>{total}</b> {t('survey.count', { count: total })}</p>
      {total > 0 && (
        <p className="sv-actions">
          <button type="button" className="ms-chip" onClick={() => downloadRows(rows, 'csv', 'interfaz', toCsv)}>{t('survey.csv')}</button>
          <button type="button" className="ms-chip" onClick={() => downloadRows(rows, 'json', 'interfaz', toCsv)}>{t('survey.json')}</button>
        </p>
      )}

      {total === 0 ? <p className="adm-note">{t('surveyUi.empty')}</p> : (
        <>
          <h2>{t('survey.summary')}</h2>
          <Bars title={t('surveyUi.points')} items={sum.points} total={total} t={t} />
          <p className="adm-muted sv-foot">{t('surveyUi.pointsHelp')}</p>
          <Bars title={u('pick.title')} items={sum.pick} total={total} t={t} />
          <Bars title={u('second.title')} items={sum.second} total={total} t={t} />

          <h3>{t('surveyUi.ratings')}</h3>
          <div className="adm-table"><table>
            <thead><tr><th>{t('surveyUi.version')}</th><th>{t('surveyUi.avg')}</th><th>{t('surveyUi.votes')}</th></tr></thead>
            <tbody>{sum.ratings.map(({ v, n, avg }) => (
              <tr key={v}><td>{label(v)}</td><td>{avg === null ? '—' : avg.toLocaleString(lang, { maximumFractionDigits: 1 })}</td><td>{n}</td></tr>
            ))}</tbody>
          </table></div>

          <h3>{t('surveyUi.mixes')}</h3>
          {mixes.length === 0 ? <p className="adm-muted">{t('survey.noTexts')}</p> : (
            <ul className="sv-texts">{mixes.map(r => (
              <li key={r.id}><b>{r.name}</b> <span className="adm-muted">({t('surveyUi.chose', { v: r.answers.pick?.toUpperCase() })})</span>
                <p>{mixText(r.answers, u)}</p>
              </li>
            ))}</ul>
          )}

          <h3>{t('surveyUi.changes')}</h3>
          {changes.length === 0 ? <p className="adm-muted">{t('survey.noTexts')}</p> : VERSIONS.filter(v => changes.some(r => r.answers.pick === v)).map(v => (
            <div key={v}>
              <h4>{label(v)}</h4>
              <ul className="sv-texts">{changes.filter(r => r.answers.pick === v).map(r => (
                <li key={r.id}><b>{r.name}</b><p>{r.answers.change}</p></li>
              ))}</ul>
            </div>
          ))}

          <h2>{t('survey.people')}</h2>
          <div className="adm-table"><table>
            <thead><tr><th>{t('survey.name')}</th><th>{u('review.pick')}</th><th>{t('survey.date')}</th><th><span className="adm-sr">{t('survey.actions')}</span></th></tr></thead>
            <tbody>{rows.map(r => {
              const a = r.answers, isOpen = open === r.id
              const rated = VERSIONS.filter(v => a.ratings?.[v])
              return (
                <Fragment key={r.id}>
                  <tr>
                    <td><b>{r.name}</b>{r.lang === 'en' && <span className="adm-muted"> (en)</span>}</td>
                    <td>{a.pick?.toUpperCase()}{a.second && <span className="adm-muted"> · {a.second.toUpperCase()}</span>}</td>
                    <td>{formatDate(r.createdAt, lang)}{r.updatedAt !== r.createdAt && <span className="adm-muted"> · {t('survey.edited', { date: formatDate(r.updatedAt, lang) })}</span>}</td>
                    <td className="sv-row-actions">
                      <button type="button" className="adm-flat" aria-expanded={isOpen} onClick={() => setOpen(isOpen ? null : r.id)}>{t(isOpen ? 'survey.hide' : 'survey.show')}</button>
                      <button type="button" className="adm-flat adm-danger" onClick={() => remove(r)}>{t('survey.remove')}</button>
                    </td>
                  </tr>
                  {isOpen && (
                    <tr className="sv-detail"><td colSpan={4}>
                      <dl>
                        <dt>{u('review.pick')}</dt><dd>{a.pick ? label(a.pick) : '—'}</dd>
                        <dt>{u('review.second')}</dt><dd>{a.second ? label(a.second) : '—'}</dd>
                        <dt>{u('review.change')}</dt><dd>{a.change?.trim() || '—'}</dd>
                        <dt>{u('review.mix')}</dt><dd>{mixText(a, u) || '—'}</dd>
                        <dt>{u('review.ratings')}</dt><dd>{rated.map(v => `${v.toUpperCase()} ${a.ratings![v]}/5`).join(' · ') || '—'}</dd>
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

function mixText(a: UiAnswers, u: (k: string, o?: Record<string, unknown>) => string): string {
  const part = a.mixPart?.trim()
  if (a.mixFrom && part) return u('review.mixValue', { letter: a.mixFrom.toUpperCase(), part })
  if (a.mixFrom) return u('review.mixOnly', { letter: a.mixFrom.toUpperCase() })
  return part ?? ''
}

/** CSV con una columna por pregunta y una por nota. Versiones con su letra, para poder cruzarlas. */
function toCsv(rows: Row[]): string {
  const head = ['nombre', 'idioma', 'fecha', 'editada', 'elegida', 'segunda', 'cambiaria', 'mezcla_de', 'mezcla_parte', ...VERSIONS.map(v => `nota_${v}`)]
  const lines = rows.map(r => {
    const a = r.answers
    return [r.name, r.lang, r.createdAt, r.updatedAt, a.pick, a.second, a.change, a.mixFrom, a.mixPart, ...VERSIONS.map(v => a.ratings?.[v])].map(csvCell).join(',')
  })
  return [head.join(','), ...lines].join('\n')
}
