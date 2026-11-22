/* Lo que comparten las encuestas públicas (/marca, /marca/ronda-1 y /interfaz): el borrador en el navegador y el envío. */

export interface Sent { id: string; token: string }

/** Lee el borrador guardado y lo completa con los valores por defecto (por si cambió la forma). */
export function loadDraft<T extends { answers: object }>(key: string, empty: () => T): T {
  try {
    const raw = localStorage.getItem(key)
    if (raw) { const d = JSON.parse(raw); const e = empty(); return { ...e, ...d, answers: { ...e.answers, ...d.answers } } }
  } catch { /* sin almacenamiento: se empieza de cero */ }
  return empty()
}

export function saveDraft(key: string, d: unknown) { try { localStorage.setItem(key, JSON.stringify(d)) } catch { /* ignorar */ } }

/** Crea la respuesta (devuelve id y código) o, si ya se envió, la cambia con su código (devuelve null).
 *  Si falla, el mensaje del error es el código de la API (`errors.<código>` en los textos). */
export async function sendSurvey(kind: 'marca' | 'logo' | 'interfaz', body: { name: string; lang: string; answers: unknown; website: string }, sent: Sent | null): Promise<Sent | null> {
  const headers: Record<string, string> = { 'content-type': 'application/json' }
  if (sent) headers['x-edit-token'] = sent.token
  let res: Response
  try {
    res = await fetch(sent ? `/api/survey/${kind}/${sent.id}` : `/api/survey/${kind}`, { method: sent ? 'PUT' : 'POST', headers, body: JSON.stringify(body) })
  } catch { throw new Error('network') }
  if (!res.ok) {
    let code = 'generic'
    try { code = (await res.json()).error ?? code } catch { /* no es JSON */ }
    throw new Error(code)
  }
  return sent ? null : res.json()
}

export const nameOk = (n: string) => { const l = n.trim().length; return l >= 2 && l <= 60 }
