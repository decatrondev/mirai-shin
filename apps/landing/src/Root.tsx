import { lazy, Suspense } from 'react'
import { BrowserRouter, Navigate, Route, Routes } from 'react-router-dom'
import { App } from './App'

// El panel se carga aparte: quien solo visita la landing no descarga su código.
const AdminApp = lazy(() => import('./admin/AdminApp'))
// La encuesta de la marca, igual: va en su propio archivo. En /marca está la ronda 2 (votación del logo);
// la ronda 1 queda en /marca/ronda-1 para quien quiera cambiar sus respuestas.
const MarcaApp = lazy(() => import('./marca/MarcaApp'))
const LogoApp = lazy(() => import('./logo/LogoApp'))
// La de la interfaz de la app, también aparte.
const InterfazApp = lazy(() => import('./interfaz/InterfazApp'))

export function Root() {
  return (
    <BrowserRouter>
      <Routes>
        <Route path="/" element={<App />} />
        <Route path="/marca" element={<Suspense fallback={null}><LogoApp /></Suspense>} />
        <Route path="/marca/ronda-1" element={<Suspense fallback={null}><MarcaApp /></Suspense>} />
        <Route path="/interfaz" element={<Suspense fallback={null}><InterfazApp /></Suspense>} />
        <Route path="/admin/*" element={<Suspense fallback={null}><AdminApp /></Suspense>} />
        <Route path="*" element={<Navigate to="/" replace />} />
      </Routes>
    </BrowserRouter>
  )
}
