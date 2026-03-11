import { check, type DownloadEvent, type Update } from '@tauri-apps/plugin-updater'
import { relaunch } from '@tauri-apps/plugin-process'

export type LauncherUpdateHandle = Update

export type LauncherUpdateDownloadProgress = {
  downloadedBytes: number
  totalBytes: number | null
  percent: number | null
  phase: 'started' | 'progress' | 'finished'
}

export async function checkLauncherUpdate(): Promise<LauncherUpdateHandle | null> {
  return check({
    timeout: 12_000,
  })
}

export async function downloadAndInstallLauncherUpdate(
  update: LauncherUpdateHandle,
  onProgress?: (progress: LauncherUpdateDownloadProgress) => void,
): Promise<void> {
  let downloadedBytes = 0
  let totalBytes: number | null = null

  await update.downloadAndInstall((event: DownloadEvent) => {
    if (event.event === 'Started') {
      downloadedBytes = 0
      totalBytes =
        typeof event.data.contentLength === 'number' ? event.data.contentLength : null
      onProgress?.({
        phase: 'started',
        downloadedBytes,
        totalBytes,
        percent: totalBytes && totalBytes > 0 ? 0 : null,
      })
      return
    }

    if (event.event === 'Progress') {
      downloadedBytes += event.data.chunkLength
      const percent =
        totalBytes && totalBytes > 0
          ? Math.min(100, Math.round((downloadedBytes / totalBytes) * 100))
          : null

      onProgress?.({
        phase: 'progress',
        downloadedBytes,
        totalBytes,
        percent,
      })
      return
    }

    onProgress?.({
      phase: 'finished',
      downloadedBytes,
      totalBytes,
      percent: totalBytes && totalBytes > 0 ? 100 : null,
    })
  })
}

export async function restartLauncher(): Promise<void> {
  await relaunch()
}
