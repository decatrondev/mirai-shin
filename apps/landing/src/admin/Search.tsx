import { useEffect, useState } from 'react'
import { Link, useSearchParams } from 'react-router-dom'
import { useTranslation } from 'react-i18next'
import { api, ApiError, type Hit } from './api'

export function Search() {
  const { t } = useTranslation('admin')
  const [params] = useSearchParams()
  const q = (params.get('q') ?? '').trim()
  const [hits, setHits] = useState<Hit[] | null>(null)
  const [error, setError] = useState('')

  useEffect(() => {
    setHits(null); setError('')
    if (q.length < 2) { setError('short'); return }
    api.search(q).then(setHits).catch(e => setError(e instanceof ApiError ? e.code : 'generic'))
  }, [q])

  return (
    <section className="adm-doc">
      <h1>{t('search.title', { q })}</h1>
      {error === 'short' && <p className="adm-note">{t('search.short')}</p>}
      {error && error !== 'short' && <p className="adm-error">{t(`errors.${error}`, t('errors.generic'))}</p>}
      {!error && !hits && <p className="adm-note" role="status">{t('loading')}</p>}
      {hits && hits.length === 0 && <p className="adm-note">{t('search.none', { q })}</p>}
      {hits && hits.length > 0 && (
        <ul className="adm-hits">
          {hits.map(h => (
            <li key={h.path}>
              <Link to={`/admin/doc/${h.path}`}>{h.title}</Link>
              <p>{h.snippet || t('search.inTitle')}</p>
            </li>
          ))}
        </ul>
      )}
    </section>
  )
}
