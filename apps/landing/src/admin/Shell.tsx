import { useEffect, useState, type FormEvent } from 'react'
import { NavLink, Outlet, useLocation, useNavigate } from 'react-router-dom'
import { useTranslation } from 'react-i18next'
import { LANGS, type Lang } from '@mirai-shin/i18n'
import brand from '@brand'
import { useAuth } from './auth'
import { api, ApiError, type DocNode } from './api'
import { StatusProvider, useStatusState } from './status'
import { setLang } from '../i18n'
import type { Theme } from '../theme'

type Props = { theme: Theme; onToggleTheme: () => void }

export function Shell({ theme, onToggleTheme }: Props) {
  const { t, i18n } = useTranslation('admin')
  const { user, logout } = useAuth()
  const [tree, setTree] = useState<DocNode[] | null>(null)
  const [treeError, setTreeError] = useState('')
  const [open, setOpen] = useState(false)
  const [status, setStatus] = useStatusState()
  const [q, setQ] = useState('')
  const navigate = useNavigate()
  const location = useLocation()
  const other: Lang = i18n.resolvedLanguage === 'en' ? 'es' : 'en'

  useEffect(() => {
    api.docs().then(setTree).catch(e => setTreeError(e instanceof ApiError ? e.code : 'generic'))
  }, [])
  // En el celular el índice se cierra al elegir algo.
  useEffect(() => { setOpen(false) }, [location.pathname, location.search])

  const onSearch = (e: FormEvent) => {
    e.preventDefault()
    if (q.trim()) navigate(`/admin/search?q=${encodeURIComponent(q.trim())}`)
  }

  return (
    <div className="adm-desk">
      <section className="ms-win adm-win" aria-label={t('title')}>
        <header className="ms-win-bar adm-bar">
          <span className="adm-bar-title"><b>{brand.name}</b> {t('title')}</span>
          <button className="ms-chip adm-index-btn" type="button" aria-expanded={open} aria-controls="adm-side" onClick={() => setOpen(o => !o)}>
            {t(open ? 'nav.close' : 'nav.index')}
          </button>
          <span className="adm-bar-user">{user?.email}</span>
        </header>

        <div className="adm-body">
          <aside id="adm-side" className={open ? 'adm-side is-open' : 'adm-side'}>
            <form className="adm-search" role="search" onSubmit={onSearch}>
              <input type="search" value={q} onChange={e => setQ(e.target.value)} placeholder={t('nav.search')} aria-label={t('nav.search')} />
            </form>
            <nav aria-label={t('nav.docs')}>
              <NavLink end to="/admin" className="adm-link">{t('nav.guide')}</NavLink>
              <p className="adm-group">{t('nav.docs')}</p>
              {treeError && <p className="adm-note">{t(`errors.${treeError}`, t('errors.generic'))}</p>}
              {tree && tree.length === 0 && <p className="adm-note">{t('doc.empty')}</p>}
              {tree && <Tree nodes={tree} />}
              <p className="adm-group adm-group-sep" aria-hidden="true" />
              {user?.role === 'SUPERADMIN' && <NavLink to="/admin/users" className="adm-link">{t('nav.users')}</NavLink>}
              <NavLink to="/admin/account" className="adm-link">{t('nav.account')}</NavLink>
            </nav>
          </aside>

          <main className="adm-main">
            <StatusProvider onChange={setStatus}>
              <Outlet />
            </StatusProvider>
          </main>
        </div>

        <footer className="adm-status">
          <span className="adm-status-text">{status}</span>
          <span className="adm-status-right">
            <span className="adm-role">{t('status.role', { role: t(`roles.${user?.role}`) })}</span>
            <button className="adm-flat" type="button" lang={other} onClick={() => setLang(other)}>{LANGS[other]}</button>
            <button className="adm-flat" type="button" onClick={onToggleTheme}>
              {t(theme === 'dark' ? 'taskbar.toLight' : 'taskbar.toDark', { ns: 'translation' })}
            </button>
            <a className="adm-flat" href="/">{t('nav.site')}</a>
            <button className="adm-flat" type="button" onClick={logout}>{t('nav.logout')}</button>
          </span>
        </footer>
      </section>
    </div>
  )
}

function Tree({ nodes, depth = 0 }: { nodes: DocNode[]; depth?: number }) {
  return (
    <ul className={depth ? 'adm-tree adm-tree-sub' : 'adm-tree'}>
      {nodes.map(n => (
        <li key={n.path}>
          {n.kind === 'folder' ? (
            <>
              <span className="adm-folder">{n.title}</span>
              <Tree nodes={n.children} depth={depth + 1} />
            </>
          ) : (
            <NavLink to={`/admin/doc/${n.path}`} className="adm-file">{n.title}</NavLink>
          )}
        </li>
      ))}
    </ul>
  )
}
