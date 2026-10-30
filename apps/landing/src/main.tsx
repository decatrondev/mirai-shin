import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import '@mirai-shin/ui/fonts.css'
import '@mirai-shin/ui/tokens.css'
import '@mirai-shin/ui/components.css'
import './styles.css'
import './i18n'
import { Root } from './Root'

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <Root />
  </StrictMode>,
)
