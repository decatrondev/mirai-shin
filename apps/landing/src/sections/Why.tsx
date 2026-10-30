import { useTranslation } from 'react-i18next'
import { Tile } from '@mirai-shin/ui'
import { useFocusable } from '../focus'

/** Por qué y dónde funciona: dos ventanas en mosaico. */
export function Why() {
  const { t } = useTranslation()
  const terms = ['pause', 'linux', 'monitors'] as const
  return (
    <section className="tiled why" id="por-que" aria-label={t('taskbar.why')}>
      <Tile {...useFocusable('why')}>
        <h2>{t('why.title')}</h2>
        <dl className="terms">
          {terms.map(k => [
            <dt key={`${k}-t`}>{t(`why.${k}Term`)}</dt>,
            <dd key={k}>{t(`why.${k}`)}</dd>,
          ])}
        </dl>
      </Tile>
      <Tile {...useFocusable('where')}>
        <h2>{t('where.title')}</h2>
        <p className="lead">{t('where.now')}</p>
        <p>{t('where.later')}</p>
      </Tile>
    </section>
  )
}
