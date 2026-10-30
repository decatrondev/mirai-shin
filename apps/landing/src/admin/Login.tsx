import { useState, type FormEvent } from 'react'
import { useTranslation } from 'react-i18next'
import { Window } from '@mirai-shin/ui'
import { useAuth } from './auth'
import { ApiError } from './api'

/** Como la pantalla de bloqueo: una sola ventana chica sobre el escritorio. */
export function Login() {
  const { t } = useTranslation('admin')
  const { login } = useAuth()
  const [email, setEmail] = useState('')
  const [password, setPassword] = useState('')
  const [error, setError] = useState('')
  const [busy, setBusy] = useState(false)

  const submit = async (e: FormEvent) => {
    e.preventDefault()
    setBusy(true)
    setError('')
    try { await login(email, password) } catch (err) { setError(err instanceof ApiError ? err.code : 'generic') }
    setBusy(false)
  }

  return (
    <div className="adm-center">
      <Window as="section" title={t('login.title')} className="adm-dialog" aria-labelledby="adm-login-h">
        <form className="adm-form" onSubmit={submit}>
          <h1 id="adm-login-h" className="adm-dialog-h">{t('login.title')}</h1>
          <p className="adm-muted">{t('login.intro')}</p>
          <label className="ms-field">{t('login.email')}
            <input className="adm-input" type="email" autoComplete="username" required value={email} onChange={e => setEmail(e.target.value)} />
          </label>
          <label className="ms-field">{t('login.password')}
            <input className="adm-input" type="password" autoComplete="current-password" required value={password} onChange={e => setPassword(e.target.value)} />
          </label>
          {error && <p className="adm-error" role="alert">{t(`errors.${error}`, t('errors.generic'))}</p>}
          <div className="adm-actions">
            <button className="ms-btn adm-submit" type="submit" disabled={busy}>{t('login.submit')}</button>
            <a className="ms-link" href="/">{t('login.back')}</a>
          </div>
        </form>
      </Window>
    </div>
  )
}
