import { createContext, useContext, useEffect, useState, type ReactNode } from 'react'

// Texto de la barra de estado (por ejemplo, cuándo se actualizó el documento abierto).
const StatusCtx = createContext<(s: string) => void>(() => {})

export function StatusProvider({ children, onChange }: { children: ReactNode; onChange: (s: string) => void }) {
  return <StatusCtx.Provider value={onChange}>{children}</StatusCtx.Provider>
}

export function useStatus(text: string) {
  const set = useContext(StatusCtx)
  useEffect(() => { set(text); return () => set('') }, [set, text])
}

export function useStatusState() {
  return useState('')
}

export function formatDate(iso: string | null, lang: string) {
  if (!iso) return ''
  return new Intl.DateTimeFormat(lang, { dateStyle: 'medium', timeStyle: 'short' }).format(new Date(iso))
}
