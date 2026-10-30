import { useCallback, useEffect, useState, type FormEvent } from 'react'
import { useTranslation } from 'react-i18next'
import { Swatches, Window } from '@mirai-shin/ui'
import { useAuth } from './auth'
import { api, ApiError, type AdminUser, type Role } from './api'
import { formatDate } from './status'

const ROLES: Role[] = ['VIEWER', 'ADMIN', 'SUPERADMIN']

/** Gestión de usuarios (solo SUPERADMIN). La clave temporal se muestra una vez y no se guarda en el navegador. */
export function Users() {
  const { t, i18n } = useTranslation('admin')
  const lang = i18n.resolvedLanguage ?? 'es'
  const { user: me } = useAuth()
  const [users, setUsers] = useState<AdminUser[] | null>(null)
  const [error, setError] = useState('')
  const [email, setEmail] = useState('')
  const [role, setRole] = useState(1)
  const [temp, setTemp] = useState<{ email: string; password: string } | null>(null)
  const [copied, setCopied] = useState(false)

  const load = useCallback(() => { api.users().then(setUsers).catch(fail) }, [])
  useEffect(load, [load])

  function fail(e: unknown) { setError(e instanceof ApiError ? e.code : 'generic') }
  const reveal = (r: { user: AdminUser; temporaryPassword: string }) => {
    setTemp({ email: r.user.email, password: r.temporaryPassword })
    setCopied(false)
    load()
  }

  const create = async (e: FormEvent) => {
    e.preventDefault()
    setError('')
    try { reveal(await api.createUser(email, ROLES[role])); setEmail('') } catch (err) { fail(err) }
  }
  const reset = async (u: AdminUser) => {
    if (!confirm(t('users.confirmReset', { email: u.email }))) return
    setError('')
    try { reveal(await api.resetUser(u.id)) } catch (err) { fail(err) }
  }
  const remove = async (u: AdminUser) => {
    if (!confirm(t('users.confirmRemove', { email: u.email }))) return
    setError('')
    try { await api.deleteUser(u.id); load() } catch (err) { fail(err) }
  }
  const copy = async () => {
    if (!temp) return
    try { await navigator.clipboard.writeText(temp.password); setCopied(true) } catch { /* el texto queda visible para copiarlo a mano */ }
  }

  return (
    <section className="adm-doc adm-wide">
      <h1>{t('users.title')}</h1>
      <p className="adm-muted">{t('users.intro')}</p>

      {temp && (
        <Window as="section" title={t('users.tempTitle', { email: temp.email })} className="adm-temp" role="status">
          <p>{t('users.tempText')}</p>
          <code className="adm-secret">{temp.password}</code>
          <div className="adm-actions">
            <button className="ms-chip" type="button" onClick={copy}>{t(copied ? 'users.copied' : 'users.copy')}</button>
            <button className="ms-btn" type="button" onClick={() => setTemp(null)}>{t('users.close')}</button>
          </div>
        </Window>
      )}

      {error && <p className="adm-error" role="alert">{t(`errors.${error}`, t('errors.generic'))}</p>}

      {users && (
        <div className="adm-table">
          <table>
            <thead>
              <tr>
                <th scope="col">{t('users.email')}</th>
                <th scope="col">{t('users.role')}</th>
                <th scope="col">{t('users.created')}</th>
                <th scope="col">{t('users.lastLogin')}</th>
                <th scope="col"><span className="adm-sr">{t('users.reset')}</span></th>
              </tr>
            </thead>
            <tbody>
              {users.map(u => (
                <tr key={u.id}>
                  <td>
                    {u.email}
                    {u.id === me?.id && <span className="adm-tag">{t('users.you')}</span>}
                    {u.mustChangePassword && <span className="adm-tag adm-tag-warn">{t('users.pending')}</span>}
                  </td>
                  <td>{t(`roles.${u.role}`)}</td>
                  <td>{formatDate(u.createdAt, lang)}</td>
                  <td>{u.lastLoginAt ? formatDate(u.lastLoginAt, lang) : t('users.never')}</td>
                  <td className="adm-row-actions">
                    {u.id !== me?.id && (
                      <>
                        <button className="adm-flat" type="button" onClick={() => reset(u)}>{t('users.reset')}</button>
                        <button className="adm-flat adm-danger" type="button" onClick={() => remove(u)}>{t('users.remove')}</button>
                      </>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      <h2>{t('users.add')}</h2>
      <form className="adm-form adm-narrow" onSubmit={create}>
        <label className="ms-field">{t('users.email')}
          <input className="adm-input" type="email" required value={email} onChange={e => setEmail(e.target.value)} />
        </label>
        <div className="ms-field">
          <span>{t('users.role')}</span>
          <Swatches label={t('users.role')} value={role} onChange={setRole} options={ROLES.map((r, i) => ({ value: i, label: t(`roles.${r}`) }))} />
        </div>
        <button className="ms-btn adm-submit" type="submit">{t('users.create')}</button>
      </form>
    </section>
  )
}
