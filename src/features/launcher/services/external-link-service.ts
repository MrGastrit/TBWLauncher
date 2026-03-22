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

  throw new Error('Tauri IPC API is unavailable.')
}

export async function openExternalUrl(url: string): Promise<void> {
  await getInvoke()<null>('open_external_url', { url })
}

