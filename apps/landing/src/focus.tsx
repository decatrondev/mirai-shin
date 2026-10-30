import { createContext, useContext, useState, type ReactNode } from 'react'

// Como en un gestor de ventanas en mosaico: la ventana que tocas queda enfocada (borde de acento).
const FocusContext = createContext<{ focused: string; setFocused: (id: string) => void }>({ focused: '', setFocused: () => {} })

export function FocusProvider({ children }: { children: ReactNode }) {
  const [focused, setFocused] = useState('')
  return <FocusContext.Provider value={{ focused, setFocused }}>{children}</FocusContext.Provider>
}

export function useFocusable(id: string) {
  const { focused, setFocused } = useContext(FocusContext)
  return { focused: focused === id, onPointerDown: () => setFocused(id) }
}
