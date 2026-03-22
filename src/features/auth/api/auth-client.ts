import type {
  AdminUserSummary,
  AccountChangeStatus,
  AuthResult,
  ChangePasswordPayload,
  LoginPayload,
  RegisterPayload,
  UpdateAccountPayload
} from '../models/auth'

type InvokeFn = <T>(command: string, args?: Record<string, unknown>) => Promise<T>

type TauriWindow = Window & {
  __TAURI__?: {
    core?: {
      invoke?: InvokeFn
    }
  }
  __TAURI_INTERNALS__?: {
    invoke?: InvokeFn
  }
}

function getInvoke(): InvokeFn {
  const tauriWindow = window as TauriWindow
  const invokeFromCore = tauriWindow.__TAURI__?.core?.invoke
  if (invokeFromCore) {
    return invokeFromCore
  }

  const invokeFromInternals = tauriWindow.__TAURI_INTERNALS__?.invoke
  if (invokeFromInternals) {
    return invokeFromInternals
  }

  throw new Error('Tauri IPC API недоступен. Убедись, что приложение запущено в окне Tauri.')
}

export async function registerRequest(payload: RegisterPayload): Promise<AuthResult> {
  return getInvoke()<AuthResult>('register', { payload })
}

export async function loginRequest(payload: LoginPayload): Promise<AuthResult> {
  return getInvoke()<AuthResult>('login', { payload })
}

export async function updateAccountRequest(
  payload: UpdateAccountPayload,
  userId?: string | null,
  identity?: string,
): Promise<void> {
  await getInvoke()<void>('update_account', {
    userId: userId?.trim() || undefined,
    identity: identity?.trim() || undefined,
    payload,
  })
}

export async function changePasswordRequest(
  payload: ChangePasswordPayload,
  userId?: string | null,
  identity?: string,
): Promise<void> {
  await getInvoke()<void>('change_password', {
    userId: userId?.trim() || undefined,
    identity: identity?.trim() || undefined,
    payload,
  })
}

export async function getAccountChangeStatusRequest(
  userId?: string | null,
  identity?: string,
): Promise<AccountChangeStatus> {
  return getInvoke()<AccountChangeStatus>('get_account_change_status', {
    userId: userId?.trim() || undefined,
    identity: identity?.trim() || undefined,
  })
}

export async function uploadSkinRequest(
  userId: string | null | undefined,
  filePath: string,
  identity?: string,
): Promise<string> {
  return getInvoke()<string>('upload_skin', {
    userId: userId?.trim() || undefined,
    identity: identity?.trim() || undefined,
    filePath,
  })
}

export async function uploadSkinDataRequest(
  userId: string | null | undefined,
  skinName: string,
  skinDataUrl: string,
  identity?: string,
): Promise<string> {
  return getInvoke()<string>('upload_skin_data', {
    userId: userId?.trim() || undefined,
    identity: identity?.trim() || undefined,
    skinName,
    skinDataUrl,
  })
}

export async function setSkinUrlRequest(
  userId: string | null | undefined,
  skinUrl: string,
  identity?: string,
): Promise<void> {
  await getInvoke()<void>('set_skin_url', {
    userId: userId?.trim() || undefined,
    identity: identity?.trim() || undefined,
    skinUrl,
  })
}

export async function adminListUsersRequest(
  actorUserId?: string | null,
  actorIdentity?: string,
  search?: string,
): Promise<AdminUserSummary[]> {
  return getInvoke()<AdminUserSummary[]>('admin_list_users', {
    actorUserId: actorUserId?.trim() || undefined,
    actorIdentity: actorIdentity?.trim() || undefined,
    search: search?.trim() || undefined,
  })
}

export async function adminSetUserRoleRequest(
  actorUserId: string | null | undefined,
  actorIdentity: string | undefined,
  targetNickname: string,
  role: string,
): Promise<void> {
  await getInvoke()<void>('admin_set_user_role', {
    actorUserId: actorUserId?.trim() || undefined,
    actorIdentity: actorIdentity?.trim() || undefined,
    targetNickname,
    role,
  })
}

export async function adminSetUserBannedRequest(
  actorUserId: string | null | undefined,
  actorIdentity: string | undefined,
  targetNickname: string,
  banned: boolean,
): Promise<void> {
  await getInvoke()<void>('admin_set_user_banned', {
    actorUserId: actorUserId?.trim() || undefined,
    actorIdentity: actorIdentity?.trim() || undefined,
    targetNickname,
    banned,
  })
}
