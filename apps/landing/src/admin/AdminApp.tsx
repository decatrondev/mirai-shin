import { useState } from 'react'
import { Navigate, Route, Routes } from 'react-router-dom'
import { useTranslation } from 'react-i18next'
import { AuthProvider, useAuth } from './auth'
import { Desktop } from './Desktop'
import { Shell } from './Shell'
import { Login } from './Login'
import { ChangePassword } from './ChangePassword'
import { Guide } from './Guide'
import { DocView } from './DocView'
import { Search } from './Search'
import { Users } from './Users'
import { Account } from './Account'
import { applyTheme, currentTheme, type Theme } from '../theme'
import './admin.css'

/** Panel `/admin`: la app maximizada sobre el escritorio, con el fondo quieto detrás para leer sin distracción. */
export default function AdminApp() {
  const [theme, setTheme] = useState<Theme>(currentTheme)
  const toggleTheme = () => {
    const next: Theme = theme === 'dark' ? 'light' : 'dark'
    applyTheme(next)
    setTheme(next)
  }
  return (
    <AuthProvider>
      <Desktop theme={theme} />
      <Gate theme={theme} onToggleTheme={toggleTheme} />
    </AuthProvider>
  )
}

function Gate({ theme, onToggleTheme }: { theme: Theme; onToggleTheme: () => void }) {
  const { user, ready } = useAuth()
  const { t } = useTranslation('admin')
  if (!ready) return <p className="adm-loading" role="status">{t('loading')}</p>
  if (!user) return <Login />
  if (user.mustChangePassword) return <ChangePassword />
  return (
    <Routes>
      <Route element={<Shell theme={theme} onToggleTheme={onToggleTheme} />}>
        <Route index element={<Guide />} />
        <Route path="doc/*" element={<DocView />} />
        <Route path="search" element={<Search />} />
        <Route path="users" element={user.role === 'SUPERADMIN' ? <Users /> : <Navigate to="/admin" replace />} />
        <Route path="account" element={<Account />} />
        <Route path="*" element={<Navigate to="/admin" replace />} />
      </Route>
    </Routes>
  )
}
