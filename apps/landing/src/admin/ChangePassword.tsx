import { useState, type FormEvent } from 'react'
import { useTranslation } from 'react-i18next'
import { Window } from '@mirai-shin/ui'
import { useAuth } from './auth'
import { ApiError, MIN_PASSWORD } from './api'

/** Primer ingreso con clave temporal: no se ve nada más hasta elegir una propia. */
export function ChangePassword() {
  const { t } = useTranslation('admin')
  const { logout } = useAuth()
  return (
    <div className="adm-center">
      <Window as="section" title={t('change.title')} className="adm-dialog" aria-labelledby="adm-change-h">
        <div className="adm-form">
          <h1 id="adm-change-h" className="adm-dialog-h">{t('change.title')}</h1>
          <p className="adm-muted">{t('change.intro')}</p>
          <PasswordForm currentLabel={t('change.current')} />
          <button className="adm-flat adm-flat-start" type="button" onClick={logout}>{t('nav.logout')}</button>
        </div>
      </Window>
    </div>
  )
}

export function PasswordForm({ currentLabel, onDone }: { currentLabel: string; onDone?: () => void }) {
  const { t } = useTranslation('admin')
  const { changePassword } = useAuth()
  const [current, setCurrent] = useState('')
  const [next, setNext] = useState('')
  const [repeat, setRepeat] = useState('')
  const [error, setError] = useState('')
  const [busy, setBusy] = useState(false)

  const submit = async (e: FormEvent) => {
    e.preventDefault()
    setError('')
    if (next !== repeat) { setError('mismatch'); return }
    setBusy(true)
    try {
      await changePassword(current, next)
      setCurrent(''); setNext(''); setRepeat('')
      onDone?.()
    } catch (err) {
      setError(err instanceof ApiError ? err.code : 'generic')
    }
    setBusy(false)
  }

  const message = error === 'mismatch' ? t('change.mismatch') : error ? t(`errors.${error}`, t('errors.generic')) : ''
  return (
    <form className="adm-form" onSubmit={submit}>
      <label className="ms-field">{currentLabel}
        <input className="adm-input" type="password" autoComplete="current-password" required value={current} onChange={e => setCurrent(e.target.value)} />
      </label>
      <label className="ms-field">{t('change.new')}
        <input className="adm-input" type="password" autoComplete="new-password" required minLength={MIN_PASSWORD} value={next} onChange={e => setNext(e.target.value)} />
      </label>
      <label className="ms-field">{t('change.repeat')}
        <input className="adm-input" type="password" autoComplete="new-password" required minLength={MIN_PASSWORD} value={repeat} onChange={e => setRepeat(e.target.value)} />
      </label>
      <p className="adm-muted adm-small">{t('change.hint', { min: MIN_PASSWORD })}</p>
      {message && <p className="adm-error" role="alert">{message}</p>}
      <button className="ms-btn adm-submit" type="submit" disabled={busy}>{t('change.submit')}</button>
    </form>
  )
}
