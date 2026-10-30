import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { useAuth } from './auth'
import { PasswordForm } from './ChangePassword'

export function Account() {
  const { t } = useTranslation('admin')
  const { user } = useAuth()
  const [done, setDone] = useState(false)
  return (
    <section className="adm-doc">
      <h1>{t('account.title')}</h1>
      <dl className="adm-dl">
        <dt>{t('account.email')}</dt><dd>{user?.email}</dd>
        <dt>{t('account.role')}</dt><dd>{t(`roles.${user?.role}`)}</dd>
      </dl>
      <h2>{t('account.changeTitle')}</h2>
      {done && <p className="adm-ok" role="status">{t('change.done')}</p>}
      <div className="adm-narrow">
        <PasswordForm currentLabel={t('change.currentAccount')} onDone={() => setDone(true)} />
      </div>
    </section>
  )
}
