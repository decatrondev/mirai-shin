// El nombre del producto vive solo en brand.json: falla si aparece escrito a mano en el código o en los textos.
import { readFileSync, readdirSync, statSync } from 'node:fs'
import { join, dirname, extname } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = join(dirname(fileURLToPath(import.meta.url)), '..')
const brand = JSON.parse(readFileSync(join(root, 'brand.json'), 'utf8'))
const needles = [brand.name, brand.kanji]
const dirs = ['apps', 'packages']
const exts = new Set(['.ts', '.tsx', '.js', '.mjs', '.json', '.css', '.html'])
const skip = new Set(['node_modules', 'dist'])

const hits = []
const walk = dir => {
  for (const name of readdirSync(dir)) {
    if (skip.has(name)) continue
    const p = join(dir, name)
    if (statSync(p).isDirectory()) walk(p)
    else if (exts.has(extname(p)) && !p.endsWith('package.json')) {
      readFileSync(p, 'utf8').split('\n').forEach((line, i) => {
        if (needles.some(n => line.includes(n))) hits.push(`${p.slice(root.length + 1)}:${i + 1}`)
      })
    }
  }
}
dirs.forEach(d => walk(join(root, d)))

if (hits.length) {
  console.error('El nombre está escrito a mano (debe salir de brand.json):\n  ' + hits.join('\n  '))
  process.exit(1)
}
console.log('Marca: el nombre solo vive en brand.json.')
