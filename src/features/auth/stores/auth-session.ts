import { writable } from 'svelte/store'
import type { AuthResult } from '../models/auth'

export const authSession = writable<AuthResult | null>(null)

export function setAuthSession(session: AuthResult): void {
  authSession.set(session)
}

export function resetAuthSession(): void {
  authSession.set(null)
}
