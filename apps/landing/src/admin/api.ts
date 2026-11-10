export type Role = 'VIEWER' | 'ADMIN' | 'SUPERADMIN'
export interface User { id: string; email: string; role: Role; mustChangePassword: boolean }
export type DocNode =
  | { kind: 'folder'; title: string; path: string; role: Role; children: DocNode[] }
  | { kind: 'file'; title: string; path: string }
export interface DocContent { path: string; title: string; markdown: string; modifiedAt: string | null }
export interface Hit { path: string; title: string; snippet: string }
export interface AdminUser {
  id: string; email: string; role: Role; mustChangePassword: boolean; createdAt: string; lastLoginAt: string | null
}
export interface SurveyAnswers {
  personality?: Record<string, number>; words?: string[]; symbols?: string[]; logoType?: string | null
  styles?: Record<string, string>; palette?: string | null; small?: string | null; avoid?: string; comment?: string
}
export interface SurveyResponse { id: string; name: string; lang: string; answers: SurveyAnswers; createdAt: string; updatedAt: string }
interface Session { token: string; user: User }

export const MIN_PASSWORD = 12
const TOKEN_KEY = 'admin_token'

export function getToken(): string | null {
  try { return localStorage.getItem(TOKEN_KEY) } catch { return null }
}
export function setToken(t: string | null) {
  try { if (t) localStorage.setItem(TOKEN_KEY, t); else localStorage.removeItem(TOKEN_KEY) } catch { /* ignorar */ }
}

/** Error con el código estable que manda la API; el front lo traduce con `errors.<code>`. */
export class ApiError extends Error {
  constructor(public code: string, public status: number) { super(code) }
}

let onUnauthorized: (() => void) | null = null
export function setUnauthorizedHandler(fn: () => void) { onUnauthorized = fn }

async function req<T>(path: string, init: RequestInit = {}): Promise<T> {
  const headers: Record<string, string> = { ...(init.headers as Record<string, string>) }
  if (init.body) headers['content-type'] = 'application/json'
  const token = getToken()
  if (token) headers.authorization = `Bearer ${token}`
  let res: Response
  try { res = await fetch(`/api${path}`, { ...init, headers }) } catch { throw new ApiError('network', 0) }
  if (!res.ok) {
    let code = 'generic'
    try { code = (await res.json()).error ?? code } catch { /* no es JSON */ }
    // Sesión vencida o clave cambiada en otro lado: se vuelve al login.
    if (res.status === 401 && token && !path.startsWith('/auth/')) { setToken(null); onUnauthorized?.() }
    throw new ApiError(code, res.status)
  }
  return res.json()
}

const post = (body: unknown): RequestInit => ({ method: 'POST', body: JSON.stringify(body) })

export const api = {
  login: (email: string, password: string) => req<Session>('/auth/login', post({ email, password })),
  me: () => req<User>('/auth/me'),
  changePassword: (current: string, next: string) => req<Session>('/auth/password', post({ current, new: next })),
  docs: () => req<DocNode[]>('/docs'),
  doc: (path: string) => req<DocContent>(`/docs/content?path=${encodeURIComponent(path)}`),
  search: (q: string) => req<Hit[]>(`/docs/search?q=${encodeURIComponent(q)}`),
  users: () => req<AdminUser[]>('/users'),
  createUser: (email: string, role: Role) =>
    req<{ user: AdminUser; temporaryPassword: string }>('/users', post({ email, role })),
  resetUser: (id: string) => req<{ user: AdminUser; temporaryPassword: string }>(`/users/${id}/reset`, { method: 'POST' }),
  deleteUser: (id: string) => req<{ ok: true }>(`/users/${id}`, { method: 'DELETE' }),
  surveyResponses: () => req<SurveyResponse[]>('/survey/marca/responses'),
  deleteSurveyResponse: (id: string) => req<{ ok: true }>(`/survey/marca/responses/${id}`, { method: 'DELETE' }),
}
