// Validador de traducciones: cada idioma debe tener las mismas claves que el español,
// sin textos vacíos y con las mismas variables {{...}}. Sale con código 1 si falta algo.
import { readdirSync, readFileSync } from 'node:fs'
import { join, dirname } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = join(dirname(fileURLToPath(import.meta.url)), '..', 'locales')
const base = 'es'
const langs = readdirSync(root)
const flat = (obj, prefix = '') =>
  Object.entries(obj).flatMap(([k, v]) =>
    v && typeof v === 'object' ? flat(v, `${prefix}${k}.`) : [[`${prefix}${k}`, v]])
const vars = s => [...String(s).matchAll(/{{\s*(\w+)\s*}}/g)].map(m => m[1]).sort().join(',')

let problems = 0
const report = msg => { problems++; console.error('  ' + msg) }

for (const file of readdirSync(join(root, base))) {
  const ref = new Map(flat(JSON.parse(readFileSync(join(root, base, file), 'utf8'))))
  for (const lang of langs) {
    let data
    try { data = new Map(flat(JSON.parse(readFileSync(join(root, lang, file), 'utf8')))) }
    catch { report(`${lang}/${file}: no existe o no es JSON válido`); continue }
    for (const [key, value] of ref) {
      if (!data.has(key)) report(`${lang}/${file}: falta «${key}»`)
      else if (typeof data.get(key) !== 'string' || !data.get(key).trim()) report(`${lang}/${file}: «${key}» está vacío`)
      else if (vars(data.get(key)) !== vars(value)) report(`${lang}/${file}: «${key}» usa variables distintas que ${base}`)
    }
    for (const key of data.keys()) if (!ref.has(key)) report(`${lang}/${file}: «${key}» sobra (no está en ${base})`)
  }
}

if (problems) { console.error(`Traducciones: ${problems} problema(s).`); process.exit(1) }
console.log(`Traducciones: 0 faltantes (${langs.join(', ')}).`)
