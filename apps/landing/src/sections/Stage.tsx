import { useTranslation } from 'react-i18next'
import { Tile } from '@mirai-shin/ui'
import brand from '@brand'
import { useFocusable } from '../focus'

// La etapa sí es una secuencia, por eso va numerada. La primera es la actual.
const STEPS = ['dev', 'first', 'later'] as const

export function Stage() {
  const { t } = useTranslation()
  return (
    <section className="tiled stage" id="etapa" aria-labelledby="h-stage">
      <Tile {...useFocusable('stage')}>
        <h2 id="h-stage">{t('stage.title')}</h2>
        <ol className="steps">
          {STEPS.map((k, i) => (
            <li key={k} className={i === 0 ? 'now' : undefined}>
              <b>{t(`stage.${k}Title`)}</b>
              <p>{t(`stage.${k}`)}</p>
              {i === 0 && <span className="mark">{t('stage.now')}</span>}
            </li>
          ))}
        </ol>
      </Tile>
      <Tile {...useFocusable('oss')}>
        <h2>{t('oss.title')}</h2>
        <p>{t('oss.text')}</p>
        <a className="ms-btn" href={brand.repo} target="_blank" rel="noopener">{t('oss.github')}</a>
      </Tile>
    </section>
  )
}
