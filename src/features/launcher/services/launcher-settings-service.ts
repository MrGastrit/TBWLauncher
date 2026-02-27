import type { LauncherSettings } from '../models/settings'

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

export async function loadLauncherSettings(): Promise<LauncherSettings> {
  return getInvoke()<LauncherSettings>('load_launcher_settings')
}

export async function saveLauncherSettings(settings: LauncherSettings): Promise<void> {
  await getInvoke()<void>('save_launcher_settings', { settings })
}

export async function getTotalRamMb(): Promise<number> {
  return getInvoke()<number>('get_total_ram_mb')
}
