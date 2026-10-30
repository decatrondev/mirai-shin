import { defineConfig, type Plugin } from 'vite'
import react from '@vitejs/plugin-react'
import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

const brandPath = fileURLToPath(new URL('../../brand.json', import.meta.url))
const brand = JSON.parse(readFileSync(brandPath, 'utf8'))
const apiUrl = process.env.API_URL ?? 'http://127.0.0.1:3034'

// index.html usa %BRAND_NAME% y %BRAND_DOMAIN%: el nombre tampoco se escribe a mano ahí.
const brandHtml = (): Plugin => ({
  name: 'brand-html',
  transformIndexHtml: html => html.replaceAll('%BRAND_NAME%', brand.name).replaceAll('%BRAND_DOMAIN%', brand.domain),
})

export default defineConfig({
  plugins: [react(), brandHtml()],
  resolve: {
    alias: { '@brand': brandPath },
  },
  // En local, /api va al backend del panel (miraishin-web). En producción lo hace nginx.
  server: { port: 5193, proxy: { '/api': apiUrl } },
  preview: { port: 5193, proxy: { '/api': apiUrl } },
})
