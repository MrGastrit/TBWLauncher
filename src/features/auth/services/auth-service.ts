export type StoredAuthSession = {
  session: AuthResult
  createdAt: number
}

import {
  changePasswordRequest,
  loginRequest,
  registerRequest,
  updateAccountRequest,
  uploadSkinRequest
} from '../api/auth-client'
import type {
  AuthResult,
  ChangePasswordPayload,
  LoginPayload,
  RegisterPayload,
  UpdateAccountPayload
} from '../models/auth'

const authStorageKey = 'tbwlauncher-auth-session'
const sessionTtlMs = 7 * 24 * 60 * 60 * 1000

export async function login(payload: LoginPayload): Promise<AuthResult> {
  const result = await loginRequest(payload)
  persistSession(result)
  return result
}

export async function register(payload: RegisterPayload): Promise<AuthResult> {
  const result = await registerRequest(payload)
  persistSession(result)
  return result
}

export async function updateAccount(payload: UpdateAccountPayload): Promise<void> {
  await updateAccountRequest(payload)
}

export async function changePassword(payload: ChangePasswordPayload): Promise<void> {
  await changePasswordRequest(payload)
}

export async function uploadSkin(filePath: string): Promise<string> {
  return uploadSkinRequest(filePath)
}

export function restoreSession(): AuthResult | null {
  const raw = localStorage.getItem(authStorageKey)
  if (!raw) {
    return null
  }

  try {
    const parsed = JSON.parse(raw) as StoredAuthSession
    if (!parsed?.session || typeof parsed.createdAt !== 'number') {
      clearSession()
      return null
    }

    const age = Date.now() - parsed.createdAt
    if (age > sessionTtlMs) {
      clearSession()
      return null
    }

    return parsed.session
  } catch {
    clearSession()
    return null
  }
}

export function clearSession(): void {
  localStorage.removeItem(authStorageKey)
}

function persistSession(session: AuthResult): void {
  const payload: StoredAuthSession = {
    session,
    createdAt: Date.now()
  }

  localStorage.setItem(authStorageKey, JSON.stringify(payload))
}
