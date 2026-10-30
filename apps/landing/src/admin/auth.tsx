import { createContext, useContext, useEffect, useState, type ReactNode } from 'react'
import { api, getToken, setToken, setUnauthorizedHandler, type User } from './api'

interface Auth {
  user: User | null
  ready: boolean
  login: (email: string, password: string) => Promise<void>
  changePassword: (current: string, next: string) => Promise<void>
  logout: () => void
}

const Ctx = createContext<Auth>(null!)

export function AuthProvider({ children }: { children: ReactNode }) {
  const [user, setUser] = useState<User | null>(null)
  const [ready, setReady] = useState(false)

  useEffect(() => {
    setUnauthorizedHandler(() => setUser(null))
    if (!getToken()) { setReady(true); return }
    api.me().then(setUser).catch(() => setToken(null)).finally(() => setReady(true))
  }, [])

  const login = async (email: string, password: string) => {
    const s = await api.login(email, password)
    setToken(s.token)
    setUser(s.user)
  }
  // El servidor devuelve un token nuevo: el anterior ya no vale.
  const changePassword = async (current: string, next: string) => {
    const s = await api.changePassword(current, next)
    setToken(s.token)
    setUser(s.user)
  }
  const logout = () => { setToken(null); setUser(null) }

  return <Ctx.Provider value={{ user, ready, login, changePassword, logout }}>{children}</Ctx.Provider>
}

export const useAuth = () => useContext(Ctx)
