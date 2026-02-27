import type {
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

export async function updateAccountRequest(payload: UpdateAccountPayload): Promise<void> {
  await getInvoke()<void>('update_account', { payload })
}

export async function changePasswordRequest(payload: ChangePasswordPayload): Promise<void> {
  await getInvoke()<void>('change_password', { payload })
}

export async function uploadSkinRequest(filePath: string): Promise<string> {
  return getInvoke()<string>('upload_skin', { filePath })
}
