import { useTranslation } from 'react-i18next'
import { Tile } from '@mirai-shin/ui'
import { LANGS, type Lang } from '@mirai-shin/i18n'
import brand from '@brand'
import type { Theme } from '../theme'

type Props = { theme: Theme; onToggleTheme: () => void; onToggleLang: () => void }

export function Footer({ theme, onToggleTheme, onToggleLang }: Props) {
  const { t, i18n } = useTranslation()
  const other: Lang = i18n.resolvedLanguage === 'en' ? 'es' : 'en'
  return (
    <section className="tiled">
      <Tile as="footer" className="foot">
        <span className="name">{brand.name}</span>
        <span className="soon">{t('footer.docs')}</span>
        <a href={brand.repo} target="_blank" rel="noopener">GitHub</a>
        <a href={`${brand.repo}/issues`} target="_blank" rel="noopener">{t('footer.contact')}</a>
        <small>{t('footer.privacy')}</small>
        <span className="prefs">
          <button className="ms-chip" type="button" lang={other} onClick={onToggleLang}>{LANGS[other]}</button>
          <button className="ms-chip" type="button" onClick={onToggleTheme}>{t(theme === 'dark' ? 'taskbar.toLight' : 'taskbar.toDark')}</button>
        </span>
      </Tile>
    </section>
  )
}
