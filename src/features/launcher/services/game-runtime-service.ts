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

export type GameRuntimeState = {
  running: boolean
  activeModeName: string | null
}

export type BuildInstallationState = {
  modeName: string
  installed: boolean
  updateAvailable: boolean
}

export type BuildInstallProgressState = {
  modeName: string
  progressPercent: number
  stageText: string
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

  throw new Error('Tauri IPC API недоступен. Убедитесь, что приложение запущено в окне Tauri.')
}

export async function toggleGameRuntime(
  modeName: string,
  nickname: string,
  gameVersion?: string,
  skinUrl?: string,
): Promise<GameRuntimeState> {
  return getInvoke()<GameRuntimeState>('toggle_game_runtime', {
    payload: {
      modeName,
      nickname,
      gameVersion,
      skinUrl,
    },
  })
}

export async function getGameRuntimeState(): Promise<GameRuntimeState> {
  return getInvoke()<GameRuntimeState>('get_game_runtime_state')
}

export async function getBuildInstallationStates(
  modeNames: string[],
): Promise<BuildInstallationState[]> {
  return getInvoke()<BuildInstallationState[]>('get_build_installation_states', {
    payload: {
      modeNames,
    },
  })
}

export async function installBuild(
  modeName: string,
): Promise<BuildInstallationState> {
  return getInvoke()<BuildInstallationState>('install_build', {
    payload: {
      modeName,
    },
  })
}

export async function getInstallProgressState(): Promise<BuildInstallProgressState | null> {
  return getInvoke()<BuildInstallProgressState | null>('get_install_progress_state')
}

export async function cancelActiveDownloads(): Promise<void> {
  await getInvoke()<null>('cancel_active_downloads')
}

export async function updateDiscordPresence(
  activeModeName: string | null,
  nickname: string,
): Promise<void> {
  await getInvoke()<null>('update_discord_presence', {
    payload: {
      activeModeName,
      nickname,
    },
  })
}
