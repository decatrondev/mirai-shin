import { useEffect, useState } from 'react'
import { Link, useParams } from 'react-router-dom'
import { useTranslation } from 'react-i18next'
import Markdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import { api, ApiError, type DocContent } from './api'
import { formatDate, useStatus } from './status'

export function DocView() {
  const { t, i18n } = useTranslation('admin')
  const path = useParams()['*'] ?? ''
  const [doc, setDoc] = useState<DocContent | null>(null)
  const [error, setError] = useState('')

  useEffect(() => {
    let alive = true
    setDoc(null); setError('')
    api.doc(path).then(d => { if (alive) setDoc(d) }).catch(e => { if (alive) setError(e instanceof ApiError ? e.code : 'generic') })
    return () => { alive = false }
  }, [path])

  useStatus(doc?.modifiedAt ? t('status.updated', { when: formatDate(doc.modifiedAt, i18n.resolvedLanguage ?? 'es') }) : '')

  if (error) return <p className="adm-note adm-pad">{error === 'not_found' ? t('doc.notFound') : t(`errors.${error}`, t('errors.generic'))}</p>
  if (!doc) return <p className="adm-note adm-pad" role="status">{t('loading')}</p>

  const dir = path.includes('/') ? path.slice(0, path.lastIndexOf('/') + 1) : ''
  return (
    <article className="adm-doc">
      <Markdown
        remarkPlugins={[remarkGfm]}
        components={{
          // Los enlaces a otros .md del panel se abren dentro del panel; el resto, en otra pestaña.
          a: ({ href = '', children }) => {
            if (/^https?:\/\//.test(href)) return <a href={href} target="_blank" rel="noopener">{children}</a>
            if (href.endsWith('.md')) return <Link to={`/admin/doc/${resolve(dir, href)}`}>{children}</Link>
            return <a href={href}>{children}</a>
          },
          table: ({ children }) => <div className="adm-table"><table>{children}</table></div>,
        }}
      >
        {doc.markdown}
      </Markdown>
    </article>
  )
}

function resolve(dir: string, href: string) {
  const parts = (dir + href).split('/')
  const out: string[] = []
  for (const p of parts) {
    if (p === '..') out.pop()
    else if (p && p !== '.') out.push(p)
  }
  return out.join('/')
}
