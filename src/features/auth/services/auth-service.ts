export type StoredAuthSession = {
  session: AuthResult
  createdAt: number
}

import {
  changePasswordRequest,
  getAccountChangeStatusRequest,
  loginRequest,
  registerRequest,
  setSkinUrlRequest,
  uploadSkinDataRequest,
  updateAccountRequest,
  uploadSkinRequest
} from '../api/auth-client'
import type {
  AccountChangeStatus,
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

export async function updateAccount(
  payload: UpdateAccountPayload,
  userId?: string | null,
  identity?: string,
): Promise<void> {
  await updateAccountRequest(payload, userId, identity)
}

export async function changePassword(
  payload: ChangePasswordPayload,
  userId?: string | null,
  identity?: string,
): Promise<void> {
  await changePasswordRequest(payload, userId, identity)
}

export async function getAccountChangeStatus(
  userId?: string | null,
  identity?: string,
): Promise<AccountChangeStatus> {
  return getAccountChangeStatusRequest(userId, identity)
}

export async function uploadSkin(
  userId: string | null | undefined,
  filePath: string,
  identity?: string,
): Promise<string> {
  const storedSkinUrl = await uploadSkinRequest(userId, filePath, identity)
  updateStoredSessionSkinUrl(storedSkinUrl)
  return storedSkinUrl
}

export async function uploadSkinData(
  userId: string | null | undefined,
  skinName: string,
  skinDataUrl: string,
  identity?: string,
): Promise<string> {
  const storedSkinUrl = await uploadSkinDataRequest(userId, skinName, skinDataUrl, identity)
  updateStoredSessionSkinUrl(storedSkinUrl)
  return storedSkinUrl
}

export async function setSkinUrl(
  userId: string | null | undefined,
  skinUrl: string,
  identity?: string,
): Promise<void> {
  await setSkinUrlRequest(userId, skinUrl, identity)
  updateStoredSessionSkinUrl(skinUrl)
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

export function updateStoredSessionSkinUrl(skinUrl: string): void {
  const session = restoreSession()
  if (!session) {
    return
  }

  const normalized = skinUrl.trim()
  if (!normalized) {
    return
  }

  persistSession({
    ...session,
    user: {
      ...session.user,
      skinUrl: normalized
    }
  })
}

export function updateStoredSessionNickname(nickname: string): void {
  const session = restoreSession()
  if (!session) {
    return
  }

  const normalized = nickname.trim()
  if (!normalized) {
    return
  }

  persistSession({
    ...session,
    user: {
      ...session.user,
      nickname: normalized
    }
  })
}

function persistSession(session: AuthResult): void {
  const payload: StoredAuthSession = {
    session,
    createdAt: Date.now()
  }

  localStorage.setItem(authStorageKey, JSON.stringify(payload))
}
