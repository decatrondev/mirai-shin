import { useTranslation } from 'react-i18next'
import brand from '@brand'

/** Primera pestaña de todo panel: qué hay, cómo se usa y avisos. */
export function Guide() {
  const { t } = useTranslation('admin')
  const url = `https://${brand.domain}/direcciones/`
  return (
    <article className="adm-doc">
      <h1>{t('guide.title')}</h1>
      <p className="adm-lead">{t('guide.lead')}</p>
      <h2>{t('guide.whatTitle')}</h2>
      <ul>
        <li>{t('guide.what1')}</li>
        <li>{t('guide.what2')}</li>
        <li>{t('guide.what3')}</li>
      </ul>
      <h2>{t('guide.howTitle')}</h2>
      <ol>
        <li>{t('guide.how1')}</li>
        <li>{t('guide.how2')}</li>
        <li>{t('guide.how3')}</li>
      </ol>
      <h2>{t('guide.rolesTitle')}</h2>
      <ul>
        <li>{t('guide.roleViewer')}</li>
        <li>{t('guide.roleAdmin')}</li>
        <li>{t('guide.roleSuper')}</li>
      </ul>
      <h2>{t('guide.notesTitle')}</h2>
      <ul>
        <li>{t('guide.note1')}</li>
        <li>{t('guide.note2', { url })}</li>
        <li>{t('guide.note3')}</li>
      </ul>
    </article>
  )
}
