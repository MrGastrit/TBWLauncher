use crate::{settings, AppState};
use base64::Engine;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use std::collections::{HashMap, HashSet};
use std::ffi::{OsStr, OsString};
use std::fs;
use std::fs::OpenOptions;
use std::io::Cursor;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use sysinfo::{Pid, ProcessesToUpdate, System};
use tauri::State;
use uuid::Uuid;
use zip::ZipArchive;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

const TBW_RUNTIME_BUNDLE_REPO: &str = "MrGastrit/tbw-gamemodes";
const TBW_RUNTIME_BUNDLE_TAG_PREFIX: &str = "dot.tbw_";
const TBW_RUNTIME_BUNDLE_ASSET_NAME: &str = "dot.tbw";
const TBW_REQUIRED_VERSION_DIRS: [&str; 4] = [
    "1.12.2",
    "1.12.2-forge-14.23.5.2859",
    "1.20.1",
    "1.20.1-forge-47.4.10",
];
const DOWNLOAD_CANCELLED_ERROR_CODE: &str = "TBW_OPERATION_CANCELLED";
const DOWNLOAD_CANCELLED_STAGE_TEXT: &str = "Отмена загрузки...";

#[derive(Debug, Clone)]
pub struct RunningGame {
    pub mode_name: String,
    pub pid: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToggleGameRuntimePayload {
    pub mode_name: String,
    pub user_id: Option<String>,
    pub nickname: String,
    pub game_version: Option<String>,
    pub skin_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildInstallationQueryPayload {
    pub mode_names: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallBuildPayload {
    pub mode_name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameRuntimeState {
    pub running: bool,
    pub active_mode_name: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildInstallationState {
    pub mode_name: String,
    pub installed: bool,
    pub update_available: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildInstallProgressState {
    pub mode_name: String,
    pub progress_percent: u8,
    pub stage_text: String,
}

pub type SharedInstallProgress = Arc<Mutex<Option<BuildInstallProgressState>>>;
pub type SharedInstallCancel = Arc<AtomicBool>;

struct LaunchPlan {
    java_executable: OsString,
    working_dir: PathBuf,
    game_dir: PathBuf,
    assets_dir: PathBuf,
    libraries_dir: PathBuf,
    natives_dir: PathBuf,
    client_jar: PathBuf,
    classpath: OsString,
    main_class: String,
    version_id: String,
    jvm_argument_tokens: Vec<String>,
    game_argument_tokens: Vec<String>,
    asset_index_name: String,
    logging_argument: Option<OsString>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct LauncherSkinRecord {
    nickname: String,
    skin_url: String,
}

struct ResolvedVersionManifest {
    version_name: String,
    manifest: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BuildSourceManifest {
    #[serde(default)]
    builds: Vec<BuildSourceEntry>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BuildSourceEntry {
    mode_name: String,
    download_url: Option<String>,
    target_folder: Option<String>,
    release_tag: Option<String>,
    repo: Option<String>,
    tag_prefix: Option<String>,
    asset_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstalledBuildMetadata {
    release_tag: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstalledRuntimeBundleMetadata {
    release_tag: String,
    directories: Vec<String>,
    files: Vec<String>,
}

#[derive(Debug, Clone)]
struct ResolvedBuildDownload {
    download_url: String,
    release_tag: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct GithubRelease {
    tag_name: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    assets: Vec<GithubReleaseAsset>,
}

#[derive(Debug, Clone, Deserialize)]
struct GithubReleaseAsset {
    name: String,
    browser_download_url: String,
}

#[tauri::command]
pub fn get_game_runtime_state(state: State<'_, AppState>) -> Result<GameRuntimeState, String> {
    let mut running_game = state
        .running_game
        .lock()
        .map_err(|_| "Failed to access game runtime state.".to_string())?;

    sync_running_game_state(&mut running_game);

    Ok(build_runtime_state(running_game.as_ref()))
}

#[tauri::command]
pub fn get_install_progress_state(
    state: State<'_, AppState>,
) -> Result<Option<BuildInstallProgressState>, String> {
    let progress = state
        .install_progress
        .lock()
        .map_err(|_| "Failed to access install progress state.".to_string())?;

    Ok(progress.clone())
}

#[tauri::command]
pub fn cancel_active_downloads(state: State<'_, AppState>) -> Result<(), String> {
    state.install_cancel.store(true, Ordering::Release);

    if let Ok(mut progress_state) = state.install_progress.lock() {
        if let Some(progress) = progress_state.as_mut() {
            progress.stage_text = DOWNLOAD_CANCELLED_STAGE_TEXT.to_string();
        }
    }

    Ok(())
}

#[tauri::command]
pub fn get_build_installation_states(
    payload: BuildInstallationQueryPayload,
) -> Result<Vec<BuildInstallationState>, String> {
    let tbw_root = find_tbw_root()?;
    let versions_dir = tbw_root.join("versions");
    ensure_directory_ready(&versions_dir)?;

    payload
        .mode_names
        .into_iter()
        .filter_map(|mode_name| {
            let trimmed = mode_name.trim().to_string();
            (!trimmed.is_empty()).then_some(trimmed)
        })
        .map(|mode_name| resolve_build_installation_state(&tbw_root, &versions_dir, &mode_name))
        .collect()
}

#[tauri::command]
pub async fn install_build(
    state: State<'_, AppState>,
    payload: InstallBuildPayload,
) -> Result<BuildInstallationState, String> {
    let mode_name = payload.mode_name.trim();
    if mode_name.is_empty() {
        return Err("Mode name is required for installation.".to_string());
    }

    let mode_name_owned = mode_name.to_string();
    let progress_state = state.install_progress.clone();
    let cancel_state = state.install_cancel.clone();
    reset_download_cancel_state(&cancel_state);
    let task_progress_state = progress_state.clone();
    let task_cancel_state = cancel_state.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        install_build_blocking(&mode_name_owned, task_progress_state, task_cancel_state)
    })
    .await;
    clear_install_progress(&progress_state);
    reset_download_cancel_state(&cancel_state);
    let result = result.map_err(|error| format!("Failed to complete the install task: {error}"))?;

    Ok(result?)
}

#[tauri::command]
pub async fn toggle_game_runtime(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    payload: ToggleGameRuntimePayload,
) -> Result<GameRuntimeState, String> {
    let mode_name = payload.mode_name.trim();
    if mode_name.is_empty() {
        return Err("Mode name is required for launch.".to_string());
    }

    let nickname = if payload.nickname.trim().is_empty() {
        "Player".to_string()
    } else {
        payload.nickname.trim().to_string()
    };

    if let Some(user_record) =
        resolve_launch_user_record(&state, payload.user_id.as_deref(), nickname.as_str()).await
    {
        if user_record.banned {
            return Err("Account is banned. Launch is unavailable.".to_string());
        }
    }
    let current_game = {
        let mut running_game = state
            .running_game
            .lock()
            .map_err(|_| "Failed to access game runtime state.".to_string())?;
        sync_running_game_state(&mut running_game);
        running_game.clone()
    };

    if let Some(current) = current_game {
        if current.mode_name == mode_name {
            let _ = stop_process_tree(current.pid);

            let mut running_game = state
                .running_game
                .lock()
                .map_err(|_| "Failed to access game runtime state.".to_string())?;
            *running_game = None;

            return Ok(GameRuntimeState {
                running: false,
                active_mode_name: None,
            });
        }

        let _ = stop_process_tree(current.pid);

        let mut running_game = state
            .running_game
            .lock()
            .map_err(|_| "Failed to access game runtime state.".to_string())?;
        *running_game = None;
    }

    let settings = settings::load_launcher_settings(app)?;
    let mode_name_owned = mode_name.to_string();
    let launch_mode_name = mode_name_owned.clone();
    let launch_nickname = nickname.clone();
    let launch_game_version = payload
        .game_version
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let launch_skin_url = payload
        .skin_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let launch_skin_url = if launch_skin_url.is_some() {
        launch_skin_url
    } else {
        resolve_user_skin_url_from_database(&state, &nickname).await
    };
    if let Some(source) = launch_skin_url.as_deref() {
        eprintln!("Resolved launch skin source: {source}");
    } else {
        eprintln!("No launch skin source was resolved for nickname {nickname}.");
    }
    let mut launcher_skin_records = resolve_launcher_skin_records_from_database(&state).await;
    if let Some(current_skin_source) = launch_skin_url.as_deref() {
        let current_nickname_key = nickname.trim().to_ascii_lowercase();
        let already_present = launcher_skin_records.iter().any(|entry| {
            entry
                .nickname
                .trim()
                .eq_ignore_ascii_case(nickname.as_str())
        });
        if !current_nickname_key.is_empty() && !already_present {
            launcher_skin_records.push(LauncherSkinRecord {
                nickname: nickname.clone(),
                skin_url: current_skin_source.to_string(),
            });
        }
    }
    let launch_settings = settings.clone();
    let progress_state = state.install_progress.clone();
    let cancel_state = state.install_cancel.clone();
    reset_download_cancel_state(&cancel_state);
    let task_progress_state = progress_state.clone();
    let task_stage_progress_state = progress_state.clone();
    let task_cancel_state = cancel_state.clone();
    let pid = tauri::async_runtime::spawn_blocking(move || -> Result<u32, String> {
        set_install_progress(
            Some(&task_progress_state),
            &launch_mode_name,
            6,
            "Проверка файлов режима...",
        );
        ensure_mode_build_current(
            &launch_mode_name,
            task_progress_state.clone(),
            task_cancel_state.clone(),
        )?;
        set_install_progress(
            Some(&task_stage_progress_state),
            &launch_mode_name,
            72,
            "Подготовка запуска...",
        );
        let launch_plan = resolve_launch_plan(
            &launch_mode_name,
            launch_game_version.as_deref(),
            Some(&task_cancel_state),
        )?;
        set_install_progress(
            Some(&task_stage_progress_state),
            &launch_mode_name,
            92,
            "Запуск игры...",
        );
        ensure_download_not_cancelled(Some(&task_cancel_state))?;
        spawn_game_process(
            &launch_plan,
            &launch_settings,
            &launch_nickname,
            launch_skin_url.as_deref(),
            launcher_skin_records.as_slice(),
            Some(&task_cancel_state),
        )
    })
    .await;
    clear_install_progress(&progress_state);
    reset_download_cancel_state(&cancel_state);
    let pid = pid.map_err(|error| format!("Failed to complete the launch task: {error}"))?;
    let pid = pid?;

    let mut running_game = state
        .running_game
        .lock()
        .map_err(|_| "Failed to access game runtime state.".to_string())?;

    *running_game = Some(RunningGame {
        mode_name: mode_name_owned.clone(),
        pid,
    });

    Ok(GameRuntimeState {
        running: true,
        active_mode_name: Some(mode_name_owned),
    })
}

async fn resolve_launch_user_record(
    state: &State<'_, AppState>,
    user_id: Option<&str>,
    nickname: &str,
) -> Option<crate::auth::models::DbUser> {
    if let Some(resolved_user_id) = user_id.map(str::trim).filter(|value| !value.is_empty()) {
        match crate::auth::repository::find_user_by_id(&state.pool, resolved_user_id).await {
            Ok(Some(user)) => return Some(user),
            Ok(None) => {
                eprintln!("Launch user id {resolved_user_id} was not found. Falling back to nickname lookup.");
            }
            Err(error) => {
                eprintln!(
                    "Failed to resolve launch user by id {resolved_user_id}: {error}. Falling back to nickname lookup.",
                );
            }
        }
    }

    let normalized_nickname = nickname.trim();
    if normalized_nickname.is_empty() {
        return None;
    }

    match crate::auth::repository::find_user_by_nickname_case_insensitive(
        &state.pool,
        normalized_nickname,
    )
    .await
    {
        Ok(user) => user,
        Err(error) => {
            eprintln!("Failed to resolve launch user by nickname {normalized_nickname}: {error}");
            None
        }
    }
}
async fn resolve_user_skin_url_from_database(
    state: &State<'_, AppState>,
    nickname: &str,
) -> Option<String> {
    let identity = nickname.trim();
    if identity.is_empty() {
        return None;
    }

    match crate::auth::repository::find_user_by_identity(&state.pool, identity).await {
        Ok(Some(user)) => {
            if user.banned {
                None
            } else {
                normalize_user_skin_url(user.skin_url.as_deref())
            }
        }
        Ok(None) => match crate::auth::repository::find_user_by_nickname_case_insensitive(
            &state.pool,
            identity,
        )
        .await
        {
            Ok(Some(user)) => {
                if user.banned {
                    None
                } else {
                    normalize_user_skin_url(user.skin_url.as_deref())
                }
            }
            Ok(None) => None,
            Err(error) => {
                eprintln!(
          "Failed to resolve user skin URL from DB (case-insensitive) for nickname {identity}: {error}"
        );
                None
            }
        },
        Err(error) => {
            eprintln!("Failed to resolve user skin URL from DB for nickname {identity}: {error}");
            None
        }
    }
}

async fn resolve_launcher_skin_records_from_database(
    state: &State<'_, AppState>,
) -> Vec<LauncherSkinRecord> {
    match sqlx::query_as::<_, LauncherSkinRecord>(
        r#"
    SELECT nickname, skin_url
    FROM users
    WHERE btrim(nickname) <> ''
      AND skin_url IS NOT NULL
      AND btrim(skin_url) <> ''
      AND COALESCE((to_jsonb(users)->>'banned')::boolean, FALSE) = FALSE
    "#,
    )
    .fetch_all(&state.pool)
    .await
    {
        Ok(records) => records,
        Err(error) => {
            eprintln!("Failed to fetch launcher skin records from DB: {error}");
            Vec::new()
        }
    }
}

fn normalize_user_skin_url(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn sync_running_game_state(running_game: &mut Option<RunningGame>) {
    if running_game
        .as_ref()
        .is_some_and(|current| !process_is_running(current.pid))
    {
        *running_game = None;
    }
}

fn build_runtime_state(running_game: Option<&RunningGame>) -> GameRuntimeState {
    GameRuntimeState {
        running: running_game.is_some(),
        active_mode_name: running_game.map(|current| current.mode_name.clone()),
    }
}

fn process_is_running(pid: u32) -> bool {
    let mut system = System::new();
    let process_pid = Pid::from_u32(pid);
    let _ = system.refresh_processes(ProcessesToUpdate::Some(&[process_pid]), true);

    system.process(process_pid).is_some()
}

fn install_build_blocking(
    mode_name: &str,
    progress_state: SharedInstallProgress,
    cancel_state: SharedInstallCancel,
) -> Result<BuildInstallationState, String> {
    let tbw_root = find_tbw_root()?;
    ensure_tbw_runtime_bundle(
        &tbw_root,
        false,
        Some(&progress_state),
        Some(&cancel_state),
        mode_name,
    )?;
    let source = find_build_source(&tbw_root, mode_name)?;

    if !build_source_has_remote_download(&source) {
        let versions_dir = tbw_root.join("versions");
        ensure_directory_ready(&versions_dir)?;
        let target_dir = versions_dir.join(source_target_folder(&source));
        let state = build_installation_state_from_target(
            mode_name,
            &target_dir,
            source_release_tag(&source).as_deref(),
        )?;

        if state.installed {
            return Ok(state);
        }
        cleanup_placeholder_build_directory(&target_dir)?;

        return Err(format!(
      "Build \"{mode_name}\" does not have a download source configured and is not installed in {}.",
      target_dir.display()
    ));
    }

    install_or_update_build(
        &tbw_root,
        mode_name,
        &source,
        Some(&progress_state),
        Some(&cancel_state),
    )
}

fn ensure_mode_build_current(
    mode_name: &str,
    progress_state: SharedInstallProgress,
    cancel_state: SharedInstallCancel,
) -> Result<(), String> {
    let tbw_root = find_tbw_root()?;
    ensure_tbw_runtime_bundle(
        &tbw_root,
        false,
        Some(&progress_state),
        Some(&cancel_state),
        mode_name,
    )?;

    if let Some(source) = try_find_build_source(&tbw_root, mode_name)? {
        if build_source_has_remote_download(&source) {
            let _ = install_or_update_build(
                &tbw_root,
                mode_name,
                &source,
                Some(&progress_state),
                Some(&cancel_state),
            )?;
        }
    }

    Ok(())
}

fn install_or_update_build(
    tbw_root: &Path,
    mode_name: &str,
    source: &BuildSourceEntry,
    progress_state: Option<&SharedInstallProgress>,
    cancel_state: Option<&SharedInstallCancel>,
) -> Result<BuildInstallationState, String> {
    ensure_download_not_cancelled(cancel_state)?;
    let versions_dir = tbw_root.join("versions");
    ensure_directory_ready(&versions_dir)?;

    let target_folder = source_target_folder(source);
    let target_dir = versions_dir.join(&target_folder);
    let downloads_dir = tbw_root.join("downloads").join(".tbw-installer");
    ensure_directory_ready(&downloads_dir)?;

    let temp_id = Uuid::new_v4().to_string();
    let archive_path = downloads_dir.join(format!("{temp_id}.zip"));
    let extract_root = downloads_dir.join(format!("extract-{temp_id}"));
    let client = build_http_client()?;
    ensure_download_not_cancelled(cancel_state)?;
    set_install_progress(progress_state, mode_name, 4, "Проверка релиза...");
    let desired_download = resolve_build_download(&client, source)?;
    let current_state = build_installation_state_from_target(
        mode_name,
        &target_dir,
        desired_download.release_tag.as_deref(),
    )?;

    if current_state.installed && !current_state.update_available {
        return Ok(current_state);
    }

    let install_result = (|| -> Result<(), String> {
        ensure_download_not_cancelled(cancel_state)?;
        set_install_progress(progress_state, mode_name, 8, "Загрузка архива...");
        let archive_bytes = download_bytes_with_progress(
            &client,
            &desired_download.download_url,
            cancel_state,
            Some(|downloaded_bytes, total_bytes| {
                let progress_percent = match total_bytes {
                    Some(total) if total > 0 => {
                        map_fraction_to_progress(downloaded_bytes, total, 8, 72)
                    }
                    _ => 18,
                };
                let stage_text = match total_bytes {
                    Some(total) if total > 0 => format!(
                        "Загрузка архива... {} из {}",
                        format_compact_bytes(downloaded_bytes),
                        format_compact_bytes(total)
                    ),
                    _ => format!(
                        "Загрузка архива... {}",
                        format_compact_bytes(downloaded_bytes)
                    ),
                };
                set_install_progress(progress_state, mode_name, progress_percent, stage_text);
            }),
        )
        .map_err(|error| {
            format!(
                "Failed to download {}: {error}",
                desired_download.download_url
            )
        })?;
        ensure_download_not_cancelled(cancel_state)?;
        if let Some(parent) = archive_path.parent() {
            ensure_directory_ready(parent)?;
        }
        set_install_progress(progress_state, mode_name, 74, "Сохранение архива...");
        ensure_download_not_cancelled(cancel_state)?;
        fs::write(&archive_path, &archive_bytes)
            .map_err(|error| format!("Failed to write {}: {error}", archive_path.display()))?;

        ensure_directory_ready(&extract_root)?;
        set_install_progress(progress_state, mode_name, 78, "Распаковка архива...");
        extract_zip_to_directory(
            &archive_path,
            &extract_root,
            cancel_state,
            Some(|processed_entries, total_entries| {
                let progress_percent = if total_entries == 0 {
                    88
                } else {
                    map_fraction_to_progress(processed_entries as u64, total_entries as u64, 78, 90)
                };
                let stage_text = if total_entries == 0 {
                    "Распаковка архива...".to_string()
                } else {
                    format!("Распаковка архива... {processed_entries}/{total_entries}")
                };
                set_install_progress(progress_state, mode_name, progress_percent, stage_text);
            }),
        )?;

        set_install_progress(progress_state, mode_name, 92, "Подготовка файлов...");
        ensure_download_not_cancelled(cancel_state)?;
        let extracted_root = resolve_extracted_build_root(&extract_root)?;
        if target_dir.exists() {
            set_install_progress(progress_state, mode_name, 95, "Замена файлов сборки...");
            fs::remove_dir_all(&target_dir)
                .map_err(|error| format!("Failed to replace {}: {error}", target_dir.display()))?;
        }
        set_install_progress(progress_state, mode_name, 97, "Перенос файлов сборки...");
        ensure_download_not_cancelled(cancel_state)?;
        move_installed_build_into_versions(&extracted_root, &target_dir)?;
        set_install_progress(progress_state, mode_name, 99, "Сохранение данных сборки...");
        ensure_download_not_cancelled(cancel_state)?;
        write_installed_build_metadata(&target_dir, desired_download.release_tag.as_deref())?;

        Ok(())
    })();

    let _ = fs::remove_file(&archive_path);
    let _ = fs::remove_dir_all(&extract_root);

    install_result?;

    Ok(BuildInstallationState {
        mode_name: mode_name.to_string(),
        installed: true,
        update_available: false,
    })
}

fn set_install_progress(
    progress_state: Option<&SharedInstallProgress>,
    mode_name: &str,
    progress_percent: u8,
    stage_text: impl Into<String>,
) {
    let Some(progress_state) = progress_state else {
        return;
    };

    if let Ok(mut state) = progress_state.lock() {
        *state = Some(BuildInstallProgressState {
            mode_name: mode_name.to_string(),
            progress_percent: progress_percent.min(100),
            stage_text: stage_text.into(),
        });
    }
}

fn clear_install_progress(progress_state: &SharedInstallProgress) {
    if let Ok(mut state) = progress_state.lock() {
        *state = None;
    }
}

fn reset_download_cancel_state(cancel_state: &SharedInstallCancel) {
    cancel_state.store(false, Ordering::Release);
}

fn ensure_download_not_cancelled(cancel_state: Option<&SharedInstallCancel>) -> Result<(), String> {
    if cancel_state.is_some_and(|state| state.load(Ordering::Acquire)) {
        return Err(DOWNLOAD_CANCELLED_ERROR_CODE.to_string());
    }

    Ok(())
}

fn map_fraction_to_progress(current: u64, total: u64, start: u8, end: u8) -> u8 {
    if total == 0 || end <= start {
        return end;
    }

    let bounded_current = current.min(total);
    let span = u64::from(end - start);
    let offset = (bounded_current.saturating_mul(span) + (total / 2)) / total;
    let progress = u64::from(start) + offset;

    progress.min(u64::from(end)) as u8
}

fn format_compact_bytes(bytes: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = KIB * 1024;
    const GIB: u64 = MIB * 1024;

    if bytes >= GIB {
        return format!("{:.1} GB", bytes as f64 / GIB as f64);
    }

    if bytes >= MIB {
        return format!("{:.1} MB", bytes as f64 / MIB as f64);
    }

    if bytes >= KIB {
        return format!("{:.1} KB", bytes as f64 / KIB as f64);
    }

    format!("{bytes} B")
}

fn ensure_tbw_runtime_bundle(
    tbw_root: &Path,
    force_refresh: bool,
    progress_state: Option<&SharedInstallProgress>,
    cancel_state: Option<&SharedInstallCancel>,
    mode_name: &str,
) -> Result<(), String> {
    ensure_download_not_cancelled(cancel_state)?;
    let refresh_needed = force_refresh || tbw_runtime_bundle_needs_refresh(tbw_root)?;
    if !refresh_needed {
        return Ok(());
    }

    let roaming_dir = tbw_root.parent().ok_or_else(|| {
        format!(
            "Failed to resolve the parent Roaming directory for {}.",
            tbw_root.display()
        )
    })?;

    let downloads_dir = roaming_dir.join(".tbw-bootstrap");
    ensure_directory_ready(&downloads_dir)?;

    let temp_id = Uuid::new_v4().to_string();
    let archive_path = downloads_dir.join(format!("dot-tbw-{temp_id}.zip"));
    let extract_root = downloads_dir.join(format!("extract-{temp_id}"));
    let client = build_http_client()?;
    ensure_download_not_cancelled(cancel_state)?;
    let desired_download = resolve_runtime_bundle_download(&client)?;

    let install_result = (|| -> Result<(), String> {
        ensure_download_not_cancelled(cancel_state)?;
        set_install_progress(progress_state, mode_name, 2, "Проверка базовых файлов...");
        let archive_bytes = download_bytes_with_progress(
            &client,
            &desired_download.download_url,
            cancel_state,
            Some(|downloaded_bytes, total_bytes| {
                let progress_percent = match total_bytes {
                    Some(total) if total > 0 => {
                        map_fraction_to_progress(downloaded_bytes, total, 2, 52)
                    }
                    _ => 12,
                };
                let stage_text = match total_bytes {
                    Some(total) if total > 0 => format!(
                        "Загрузка базовых файлов... {} из {}",
                        format_compact_bytes(downloaded_bytes),
                        format_compact_bytes(total)
                    ),
                    _ => format!(
                        "Загрузка базовых файлов... {}",
                        format_compact_bytes(downloaded_bytes)
                    ),
                };
                set_install_progress(progress_state, mode_name, progress_percent, stage_text);
            }),
        )
        .map_err(|error| {
            format!(
                "Failed to download {}: {error}",
                desired_download.download_url
            )
        })?;

        set_install_progress(
            progress_state,
            mode_name,
            54,
            "Сохранение базового архива...",
        );
        fs::write(&archive_path, &archive_bytes)
            .map_err(|error| format!("Failed to write {}: {error}", archive_path.display()))?;

        ensure_directory_ready(&extract_root)?;
        set_install_progress(
            progress_state,
            mode_name,
            58,
            "Распаковка базовых файлов...",
        );
        let extract_progress_end = if force_refresh { 70 } else { 76 };
        extract_zip_to_directory(
            &archive_path,
            &extract_root,
            cancel_state,
            Some(|processed_entries, total_entries| {
                let progress_percent = if total_entries == 0 {
                    extract_progress_end
                } else {
                    map_fraction_to_progress(
                        processed_entries as u64,
                        total_entries as u64,
                        58,
                        extract_progress_end,
                    )
                };
                let stage_text = if total_entries == 0 {
                    "Распаковка базовых файлов...".to_string()
                } else {
                    format!("Распаковка базовых файлов... {processed_entries}/{total_entries}")
                };
                set_install_progress(progress_state, mode_name, progress_percent, stage_text);
            }),
        )?;

        let extracted_root = resolve_extracted_build_root(&extract_root)?;
        ensure_download_not_cancelled(cancel_state)?;
        let runtime_source_root = resolve_runtime_bundle_source_root(&extracted_root)?;
        let merge_start = if force_refresh { 72 } else { 78 };
        let merge_end = 96;
        let mut on_progress = |processed_files: usize, total_files: usize| {
            let progress_percent = if total_files == 0 {
                merge_end
            } else {
                map_fraction_to_progress(
                    processed_files as u64,
                    total_files as u64,
                    merge_start,
                    merge_end,
                )
            };
            let stage_text = if total_files == 0 {
                "Обновление базовых файлов...".to_string()
            } else {
                format!("Обновление базовых файлов... {processed_files}/{total_files}")
            };
            set_install_progress(progress_state, mode_name, progress_percent, stage_text);
        };

        merge_directory_contents(
            &runtime_source_root,
            tbw_root,
            cancel_state,
            Some(&mut on_progress),
        )?;
        ensure_download_not_cancelled(cancel_state)?;
        write_installed_runtime_bundle_metadata(
            tbw_root,
            &runtime_source_root,
            desired_download.release_tag.as_deref(),
        )?;

        set_install_progress(progress_state, mode_name, 98, "Базовые файлы готовы.");
        Ok(())
    })();

    let _ = fs::remove_file(&archive_path);
    let _ = fs::remove_dir_all(&extract_root);

    install_result
}

fn tbw_runtime_bundle_needs_refresh(tbw_root: &Path) -> Result<bool, String> {
    if !tbw_root.is_dir() {
        return Ok(true);
    }

    let java_storage_ready =
        tbw_root.join("runtime").is_dir() || tbw_root.join("java_versions").is_dir();
    let required_top_level_ready = tbw_root.join("assets").is_dir()
        && tbw_root.join("libraries").is_dir()
        && java_storage_ready;

    if !required_top_level_ready {
        return Ok(true);
    }

    if !tbw_root.join("build_sources.json").is_file() {
        return Ok(true);
    }

    let versions_dir = tbw_root.join("versions");
    if !versions_dir.is_dir() {
        return Ok(true);
    }

    let missing_required_version_dir = TBW_REQUIRED_VERSION_DIRS
        .into_iter()
        .any(|folder_name| !versions_dir.join(folder_name).is_dir());

    if missing_required_version_dir {
        return Ok(true);
    }

    let metadata = match read_installed_runtime_bundle_metadata(tbw_root) {
        Ok(Some(metadata)) => metadata,
        Ok(None) => return Ok(true),
        Err(_) => return Ok(true),
    };

    if metadata.release_tag.trim().is_empty() {
        return Ok(true);
    }

    let missing_directory = metadata
        .directories
        .iter()
        .filter(|relative_path| !relative_path.trim().is_empty())
        .any(|relative_path| !tbw_root.join(Path::new(relative_path)).is_dir());

    if missing_directory {
        return Ok(true);
    }

    let missing_file = metadata
        .files
        .iter()
        .filter(|relative_path| !relative_path.trim().is_empty())
        .any(|relative_path| !tbw_root.join(Path::new(relative_path)).is_file());

    Ok(missing_file)
}

fn resolve_runtime_bundle_source_root(extracted_root: &Path) -> Result<PathBuf, String> {
    if extracted_root
        .file_name()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case(".tbw"))
    {
        return Ok(extracted_root.to_path_buf());
    }

    let nested_tbw = extracted_root.join(".tbw");
    if nested_tbw.is_dir() {
        return Ok(nested_tbw);
    }

    Ok(extracted_root.to_path_buf())
}

fn read_build_sources_manifest(tbw_root: &Path) -> Result<BuildSourceManifest, String> {
    let manifest_path = tbw_root.join("build_sources.json");
    let raw = fs::read_to_string(&manifest_path)
        .map_err(|error| format!("Failed to read {}: {error}", manifest_path.display()))?;

    serde_json::from_str::<BuildSourceManifest>(&raw)
        .map_err(|error| format!("The file {} is invalid: {error}", manifest_path.display()))
}

fn find_build_source(tbw_root: &Path, mode_name: &str) -> Result<BuildSourceEntry, String> {
    let manifest = read_build_sources_manifest(tbw_root)?;
    let target = normalize_mode_name(mode_name);

    manifest
        .builds
        .into_iter()
        .find(|entry| normalize_mode_name(&entry.mode_name) == target)
        .ok_or_else(|| {
            format!(
        "No download source was found for build \"{mode_name}\" in .tbw/build_sources.json."
      )
        })
}

fn try_find_build_source(
    tbw_root: &Path,
    mode_name: &str,
) -> Result<Option<BuildSourceEntry>, String> {
    let manifest_path = tbw_root.join("build_sources.json");
    if !manifest_path.is_file() {
        return Ok(None);
    }

    let manifest = read_build_sources_manifest(tbw_root)?;
    let target = normalize_mode_name(mode_name);

    Ok(manifest
        .builds
        .into_iter()
        .find(|entry| normalize_mode_name(&entry.mode_name) == target))
}

fn resolve_build_installation_state(
    tbw_root: &Path,
    versions_dir: &Path,
    mode_name: &str,
) -> Result<BuildInstallationState, String> {
    if let Some(source) = try_find_build_source(tbw_root, mode_name)? {
        let target_dir = versions_dir.join(source_target_folder(&source));
        return build_installation_state_from_target(
            mode_name,
            &target_dir,
            source_release_tag(&source).as_deref(),
        );
    }

    Ok(BuildInstallationState {
        mode_name: mode_name.to_string(),
        installed: is_build_installed_in_versions(versions_dir, mode_name)?,
        update_available: false,
    })
}

fn source_target_folder(source: &BuildSourceEntry) -> String {
    source
        .target_folder
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| source.mode_name.clone())
}

fn build_source_has_remote_download(source: &BuildSourceEntry) -> bool {
    source
        .repo
        .as_deref()
        .map(str::trim)
        .is_some_and(|value| !value.is_empty())
        || source
            .download_url
            .as_deref()
            .map(str::trim)
            .is_some_and(|value| !value.is_empty())
}

fn source_release_tag(source: &BuildSourceEntry) -> Option<String> {
    source
        .release_tag
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            source
                .download_url
                .as_deref()
                .and_then(parse_github_release_tag)
        })
}

fn parse_github_release_tag(download_url: &str) -> Option<String> {
    let marker = "/releases/download/";
    let (_, remainder) = download_url.split_once(marker)?;
    let (tag, _) = remainder.split_once('/')?;
    let trimmed = tag.trim();

    (!trimmed.is_empty()).then_some(trimmed.to_string())
}

fn resolve_runtime_bundle_download(http_client: &Client) -> Result<ResolvedBuildDownload, String> {
    resolve_github_release_download(
        http_client,
        TBW_RUNTIME_BUNDLE_REPO,
        TBW_RUNTIME_BUNDLE_TAG_PREFIX,
        TBW_RUNTIME_BUNDLE_ASSET_NAME,
        "the base .tbw runtime bundle",
    )
}

fn resolve_build_download(
    http_client: &Client,
    source: &BuildSourceEntry,
) -> Result<ResolvedBuildDownload, String> {
    if source
        .repo
        .as_deref()
        .map(str::trim)
        .is_some_and(|value| !value.is_empty())
    {
        return resolve_github_build_download(http_client, source);
    }

    resolve_static_build_download(source)
}

fn resolve_static_build_download(
    source: &BuildSourceEntry,
) -> Result<ResolvedBuildDownload, String> {
    let download_url = source
        .download_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            format!(
                "Build source \"{}\" does not contain downloadUrl or repo.",
                source.mode_name
            )
        })?;

    Ok(ResolvedBuildDownload {
        download_url: download_url.to_string(),
        release_tag: source_release_tag(source),
    })
}

fn resolve_github_build_download(
    http_client: &Client,
    source: &BuildSourceEntry,
) -> Result<ResolvedBuildDownload, String> {
    let repo = source
        .repo
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            format!(
                "Build source \"{}\" does not contain repo.",
                source.mode_name
            )
        })?;
    let default_prefix = source.mode_name.trim();
    let tag_prefix = source
        .tag_prefix
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(default_prefix);
    let asset_hint = source
        .asset_name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(default_prefix)
        .to_ascii_lowercase();

    resolve_github_release_download(
        http_client,
        repo,
        tag_prefix,
        &asset_hint,
        &format!("build source \"{}\"", source.mode_name),
    )
}

fn resolve_github_release_download(
    http_client: &Client,
    repo: &str,
    tag_prefix: &str,
    asset_hint: &str,
    resource_name: &str,
) -> Result<ResolvedBuildDownload, String> {
    let api_url = format!("https://api.github.com/repos/{repo}/releases?per_page=100");
    let response = http_client
        .get(&api_url)
        .send()
        .and_then(|value| value.error_for_status())
        .map_err(|error| format!("Failed to read GitHub releases for {repo}: {error}"))?;
    let response_body = response
        .text()
        .map_err(|error| format!("Failed to read the GitHub response body for {repo}: {error}"))?;
    let releases = serde_json::from_str::<Vec<GithubRelease>>(&response_body).map_err(|error| {
        format!("GitHub returned an invalid releases payload for {repo}: {error}")
    })?;

    let tag_prefix_lower = tag_prefix.to_ascii_lowercase();
    let asset_hint_lower = asset_hint.to_ascii_lowercase();
    let release = releases
        .into_iter()
        .find(|release| {
            !release.draft
                && !release.prerelease
                && release
                    .tag_name
                    .to_ascii_lowercase()
                    .starts_with(&tag_prefix_lower)
        })
        .ok_or_else(|| {
            format!(
        "No published GitHub release matched prefix \"{tag_prefix}\" for {resource_name} in {repo}."
      )
        })?;

    let asset = release
        .assets
        .iter()
        .find(|asset| {
            let asset_name = asset.name.to_ascii_lowercase();
            asset_name.ends_with(".zip")
                && (asset_hint_lower.is_empty() || asset_name.contains(&asset_hint_lower))
        })
        .or_else(|| {
            release
                .assets
                .iter()
                .find(|asset| asset.name.to_ascii_lowercase().ends_with(".zip"))
        })
        .ok_or_else(|| {
            format!(
                "Release \"{}\" for {resource_name} in {repo} does not contain a zip asset.",
                release.tag_name
            )
        })?;

    Ok(ResolvedBuildDownload {
        download_url: asset.browser_download_url.clone(),
        release_tag: Some(release.tag_name),
    })
}

fn build_installation_state_from_target(
    mode_name: &str,
    target_dir: &Path,
    desired_release_tag: Option<&str>,
) -> Result<BuildInstallationState, String> {
    if !build_directory_has_payload(target_dir)? {
        return Ok(BuildInstallationState {
            mode_name: mode_name.to_string(),
            installed: false,
            update_available: false,
        });
    }

    let update_available = if let Some(expected_tag) = desired_release_tag {
        read_installed_build_metadata(target_dir)?
            .map(|metadata| metadata.release_tag.trim() != expected_tag)
            .unwrap_or(true)
    } else {
        false
    };

    Ok(BuildInstallationState {
        mode_name: mode_name.to_string(),
        installed: true,
        update_available,
    })
}

fn build_directory_has_payload(target_dir: &Path) -> Result<bool, String> {
    if !target_dir.is_dir() {
        return Ok(false);
    }

    let entries = fs::read_dir(target_dir)
        .map_err(|error| format!("Failed to read directory {}: {error}", target_dir.display()))?;

    for entry in entries {
        let entry = entry.map_err(|error| {
            format!(
                "Failed to read directory entry in {}: {error}",
                target_dir.display()
            )
        })?;
        let name = entry.file_name().to_string_lossy().to_string();

        if name == ".tbw-build.json" {
            continue;
        }

        return Ok(true);
    }

    Ok(false)
}

fn cleanup_placeholder_build_directory(target_dir: &Path) -> Result<(), String> {
    if !target_dir.is_dir() || build_directory_has_payload(target_dir)? {
        return Ok(());
    }

    fs::remove_dir_all(target_dir)
        .map_err(|error| format!("Failed to remove {}: {error}", target_dir.display()))
}

fn read_installed_build_metadata(
    target_dir: &Path,
) -> Result<Option<InstalledBuildMetadata>, String> {
    let metadata_path = target_dir.join(".tbw-build.json");
    if !metadata_path.is_file() {
        return Ok(None);
    }

    let raw = fs::read_to_string(&metadata_path)
        .map_err(|error| format!("Failed to read {}: {error}", metadata_path.display()))?;

    serde_json::from_str::<InstalledBuildMetadata>(&raw)
        .map(Some)
        .map_err(|error| format!("The file {} is invalid: {error}", metadata_path.display()))
}

fn write_installed_build_metadata(
    target_dir: &Path,
    release_tag: Option<&str>,
) -> Result<(), String> {
    let Some(release_tag) = release_tag.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(());
    };

    ensure_directory_ready(target_dir)?;

    let metadata_path = target_dir.join(".tbw-build.json");
    let payload = InstalledBuildMetadata {
        release_tag: release_tag.to_string(),
    };
    let serialized = serde_json::to_string_pretty(&payload)
        .map_err(|error| format!("Failed to serialize {}: {error}", metadata_path.display()))?;

    fs::write(&metadata_path, serialized)
        .map_err(|error| format!("Failed to write {}: {error}", metadata_path.display()))
}

fn read_installed_runtime_bundle_metadata(
    tbw_root: &Path,
) -> Result<Option<InstalledRuntimeBundleMetadata>, String> {
    let metadata_path = tbw_root.join(".tbw-runtime.json");
    if !metadata_path.is_file() {
        return Ok(None);
    }

    let raw = fs::read_to_string(&metadata_path)
        .map_err(|error| format!("Failed to read {}: {error}", metadata_path.display()))?;

    serde_json::from_str::<InstalledRuntimeBundleMetadata>(&raw)
        .map(Some)
        .map_err(|error| format!("The file {} is invalid: {error}", metadata_path.display()))
}

fn write_installed_runtime_bundle_metadata(
    tbw_root: &Path,
    source_root: &Path,
    release_tag: Option<&str>,
) -> Result<(), String> {
    ensure_directory_ready(tbw_root)?;

    let metadata_path = tbw_root.join(".tbw-runtime.json");
    let payload = InstalledRuntimeBundleMetadata {
        release_tag: release_tag
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or_default()
            .to_string(),
        directories: collect_relative_directory_paths(source_root)?
            .into_iter()
            .map(|path| path.to_string_lossy().to_string())
            .collect(),
        files: collect_relative_file_paths(source_root)?
            .into_iter()
            .map(|path| path.to_string_lossy().to_string())
            .collect(),
    };
    let serialized = serde_json::to_string_pretty(&payload)
        .map_err(|error| format!("Failed to serialize {}: {error}", metadata_path.display()))?;

    fs::write(&metadata_path, serialized)
        .map_err(|error| format!("Failed to write {}: {error}", metadata_path.display()))
}

fn is_build_installed_in_versions(versions_dir: &Path, mode_name: &str) -> Result<bool, String> {
    let target = normalize_mode_name(mode_name);
    if target.is_empty() {
        return Ok(false);
    }

    let entries = fs::read_dir(versions_dir).map_err(|error| {
        format!(
            "Failed to read directory {}: {error}",
            versions_dir.display()
        )
    })?;

    for entry in entries {
        let entry = entry.map_err(|error| {
            format!(
                "Failed to read directory entry in {}: {error}",
                versions_dir.display()
            )
        })?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let directory_name = entry.file_name().to_string_lossy().to_string();
        if normalize_mode_name(&directory_name) == target {
            return Ok(true);
        }
    }

    Ok(false)
}

fn extract_zip_to_directory<F>(
    archive_path: &Path,
    target_dir: &Path,
    cancel_state: Option<&SharedInstallCancel>,
    mut on_progress: Option<F>,
) -> Result<(), String>
where
    F: FnMut(usize, usize),
{
    ensure_download_not_cancelled(cancel_state)?;
    let archive_bytes = fs::read(archive_path)
        .map_err(|error| format!("Failed to read archive {}: {error}", archive_path.display()))?;
    let cursor = Cursor::new(archive_bytes);
    let mut archive = ZipArchive::new(cursor)
        .map_err(|error| format!("Failed to open archive {}: {error}", archive_path.display()))?;
    let total_entries = archive.len();

    for index in 0..total_entries {
        ensure_download_not_cancelled(cancel_state)?;
        let mut file = archive.by_index(index).map_err(|error| {
            format!(
                "Failed to read archive entry in {}: {error}",
                archive_path.display()
            )
        })?;
        let entry_name = file.name().replace('\\', "/");
        if entry_name.is_empty() || should_skip_zip_install_entry(&entry_name) {
            if let Some(callback) = on_progress.as_mut() {
                callback(index + 1, total_entries);
            }
            continue;
        }

        let output_path = target_dir.join(&entry_name);

        if file.is_dir() {
            ensure_directory_ready(&output_path)?;
            if let Some(callback) = on_progress.as_mut() {
                callback(index + 1, total_entries);
            }
            continue;
        }

        if let Some(parent) = output_path.parent() {
            ensure_directory_ready(parent)?;
        }

        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer).map_err(|error| {
            format!(
                "Failed to extract archive entry {} from {}: {error}",
                entry_name,
                archive_path.display()
            )
        })?;

        fs::write(&output_path, buffer).map_err(|error| {
            format!(
                "Failed to write extracted file {}: {error}",
                output_path.display()
            )
        })?;

        if let Some(callback) = on_progress.as_mut() {
            callback(index + 1, total_entries);
        }
    }

    Ok(())
}

fn merge_directory_contents<F>(
    source_root: &Path,
    target_root: &Path,
    cancel_state: Option<&SharedInstallCancel>,
    mut on_progress: Option<&mut F>,
) -> Result<(), String>
where
    F: FnMut(usize, usize),
{
    ensure_download_not_cancelled(cancel_state)?;
    ensure_directory_ready(target_root)?;

    let directory_entries = collect_relative_directory_paths(source_root)?;
    for relative_dir in directory_entries {
        ensure_directory_ready(&target_root.join(relative_dir))?;
    }

    let file_entries = collect_relative_file_paths(source_root)?;
    let total_files = file_entries.len();

    for (index, relative_file) in file_entries.iter().enumerate() {
        ensure_download_not_cancelled(cancel_state)?;
        let source_path = source_root.join(relative_file);
        let target_path = target_root.join(relative_file);

        if target_path.exists() && should_preserve_existing_runtime_file(relative_file) {
            if let Some(callback) = on_progress.as_mut() {
                (**callback)(index + 1, total_files);
            }
            continue;
        }

        if let Some(parent) = target_path.parent() {
            ensure_directory_ready(parent)?;
        }

        fs::copy(&source_path, &target_path).map_err(|error| {
            format!(
                "Failed to copy {} into {}: {error}",
                source_path.display(),
                target_path.display()
            )
        })?;

        if let Some(callback) = on_progress.as_mut() {
            (**callback)(index + 1, total_files);
        }
    }

    Ok(())
}

fn should_preserve_existing_runtime_file(relative_path: &Path) -> bool {
    relative_path.components().count() == 1
        && relative_path
            .file_name()
            .and_then(|value| value.to_str())
            .is_some_and(|value| value.eq_ignore_ascii_case("build_sources.json"))
}

fn collect_relative_directory_paths(source_root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut directories = Vec::new();
    let mut unused_files = Vec::new();
    collect_relative_entries(
        source_root,
        source_root,
        &mut directories,
        &mut unused_files,
    )?;
    directories.sort();
    Ok(directories)
}

fn collect_relative_file_paths(source_root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    let mut unused_directories = Vec::new();
    collect_relative_entries(
        source_root,
        source_root,
        &mut unused_directories,
        &mut files,
    )?;
    files.sort();
    Ok(files)
}

fn collect_relative_entries(
    source_root: &Path,
    current_dir: &Path,
    directories: &mut Vec<PathBuf>,
    files: &mut Vec<PathBuf>,
) -> Result<(), String> {
    let entries = fs::read_dir(current_dir).map_err(|error| {
        format!(
            "Failed to read directory {}: {error}",
            current_dir.display()
        )
    })?;

    for entry in entries {
        let entry = entry.map_err(|error| {
            format!(
                "Failed to read directory entry in {}: {error}",
                current_dir.display()
            )
        })?;
        let path = entry.path();
        let relative_path = path.strip_prefix(source_root).map_err(|error| {
            format!(
                "Failed to resolve the relative path for {}: {error}",
                path.display()
            )
        })?;

        if path.is_dir() {
            directories.push(relative_path.to_path_buf());
            collect_relative_entries(source_root, &path, directories, files)?;
            continue;
        }

        files.push(relative_path.to_path_buf());
    }

    Ok(())
}

fn should_skip_zip_install_entry(entry_name: &str) -> bool {
    let normalized = entry_name.trim_start_matches('/');
    normalized.is_empty()
        || normalized.starts_with("__MACOSX/")
        || normalized.eq_ignore_ascii_case(".ds_store")
}

fn resolve_extracted_build_root(extract_root: &Path) -> Result<PathBuf, String> {
    let entries = fs::read_dir(extract_root).map_err(|error| {
        format!(
            "Failed to read directory {}: {error}",
            extract_root.display()
        )
    })?;
    let mut useful_entries = Vec::new();

    for entry in entries {
        let entry = entry.map_err(|error| {
            format!(
                "Failed to read directory entry in {}: {error}",
                extract_root.display()
            )
        })?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        if name == "__MACOSX" {
            continue;
        }

        useful_entries.push(path);
    }

    if useful_entries.is_empty() {
        return Err("The downloaded archive is empty.".to_string());
    }

    if useful_entries.len() == 1 && useful_entries[0].is_dir() {
        return Ok(useful_entries.remove(0));
    }

    Ok(extract_root.to_path_buf())
}

fn move_installed_build_into_versions(source_root: &Path, target_dir: &Path) -> Result<(), String> {
    if source_root == target_dir {
        return Ok(());
    }

    if source_root.is_dir() {
        if fs::rename(source_root, target_dir).is_ok() {
            return Ok(());
        }
    }

    ensure_directory_ready(target_dir)?;
    move_directory_contents(source_root, target_dir)
}

fn move_directory_contents(source_root: &Path, target_dir: &Path) -> Result<(), String> {
    let entries = fs::read_dir(source_root).map_err(|error| {
        format!(
            "Failed to read directory {}: {error}",
            source_root.display()
        )
    })?;

    for entry in entries {
        let entry = entry.map_err(|error| {
            format!(
                "Failed to read directory entry in {}: {error}",
                source_root.display()
            )
        })?;
        let source_path = entry.path();
        let target_path = target_dir.join(entry.file_name());

        if target_path.exists() {
            return Err(format!(
                "The target path {} already exists.",
                target_path.display()
            ));
        }

        fs::rename(&source_path, &target_path).map_err(|error| {
            format!(
                "Failed to move {} into {}: {error}",
                source_path.display(),
                target_path.display()
            )
        })?;
    }

    Ok(())
}

fn spawn_game_process(
    plan: &LaunchPlan,
    settings: &settings::LauncherSettings,
    nickname: &str,
    selected_skin_url: Option<&str>,
    launcher_skin_records: &[LauncherSkinRecord],
    cancel_state: Option<&SharedInstallCancel>,
) -> Result<u32, String> {
    ensure_download_not_cancelled(cancel_state)?;
    let selected_skin_source = selected_skin_url
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let selected_skin_path =
        resolve_selected_skin_file_path(plan, selected_skin_source, cancel_state);
    let skin_cdn_base_url =
        resolve_effective_skin_cdn_base_url(selected_skin_source, launcher_skin_records);
    let skin_exchange_dir = resolve_skin_exchange_dir(plan);
    if let Some(source_skin_path) = selected_skin_path.as_deref() {
        eprintln!("Resolved launch skin file: {}", source_skin_path.display());
    } else if let Some(source_value) = selected_skin_source {
        eprintln!("Launch skin source did not resolve to file: {source_value}");
    } else {
        eprintln!("No launch skin source was provided.");
    }
    eprintln!("Skin exchange directory: {}", skin_exchange_dir.display());
    if let Some(value) = skin_cdn_base_url.as_deref() {
        eprintln!("Skin CDN base URL: {value}");
    }
    if let Err(error) = ensure_directory_ready(&skin_exchange_dir) {
        eprintln!(
            "Failed to prepare skin exchange directory {}: {error}",
            skin_exchange_dir.display()
        );
    }
    ensure_launch_paths_writable(plan)?;

    if let Err(error) = sync_launcher_player_skins_into_game(
        plan,
        nickname,
        selected_skin_source,
        selected_skin_path.as_deref(),
        launcher_skin_records,
        skin_exchange_dir.as_path(),
        skin_cdn_base_url.as_deref(),
        cancel_state,
    ) {
        eprintln!("Failed to prepare launcher player skins for launch: {error}");
    }

    let mut launch_args = build_java_arguments(
        plan,
        settings,
        nickname,
        selected_skin_source,
        selected_skin_path.as_deref(),
        skin_exchange_dir.as_path(),
        skin_cdn_base_url.as_deref(),
    )?;
    launch_args.extend(build_game_arguments(plan, nickname));

    #[cfg(windows)]
    if settings.show_logs {
        let mut command = Command::new("powershell.exe");
        configure_process_spawn(&mut command, true);
        apply_selected_skin_environment(
            &mut command,
            selected_skin_source,
            selected_skin_path.as_deref(),
            nickname,
            skin_exchange_dir.as_path(),
            skin_cdn_base_url.as_deref(),
        );
        command.current_dir(&plan.working_dir);
        command.arg("-NoLogo");
        command.arg("-NoExit");
        command.arg("-Command");
        command.arg(build_debug_powershell_script(
            plan.java_executable.as_os_str(),
            &launch_args,
        ));

        ensure_download_not_cancelled(cancel_state)?;
        let child = command
            .spawn()
            .map_err(|error| format!("Failed to start the Java process: {error}"))?;

        return Ok(child.id());
    }

    let mut command = Command::new(&plan.java_executable);
    configure_process_spawn(&mut command, settings.show_logs);
    apply_selected_skin_environment(
        &mut command,
        selected_skin_source,
        selected_skin_path.as_deref(),
        nickname,
        skin_exchange_dir.as_path(),
        skin_cdn_base_url.as_deref(),
    );
    command.current_dir(&plan.working_dir);

    let launch_log_path = plan.working_dir.join("logs").join("tbw-java-launch.log");
    if !settings.show_logs {
        ensure_directory_ready(&plan.working_dir.join("logs"))?;

        let stdout_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&launch_log_path)
            .map_err(|error| {
                format!(
                    "Failed to open launch log file {}: {error}",
                    launch_log_path.display()
                )
            })?;
        let stderr_file = stdout_file
            .try_clone()
            .map_err(|error| format!("Failed to clone launch log file handle: {error}"))?;

        command.stdout(Stdio::from(stdout_file));
        command.stderr(Stdio::from(stderr_file));
    }

    for arg in launch_args {
        command.arg(arg);
    }

    ensure_download_not_cancelled(cancel_state)?;
    let child = command
        .spawn()
        .map_err(|error| format!("Failed to start the Java process: {error}"))?;
    let pid = child.id();

    if !settings.show_logs && process_exited_early(pid, Duration::from_secs(4)) {
        let mut message = format!(
            "The game process exited during startup. See {} for details.",
            launch_log_path.display()
        );

        if let Some(tail) = read_text_file_tail(&launch_log_path, 60) {
            message.push_str("\n\n=== Last launch log lines ===\n");
            message.push_str(&tail);
        }

        return Err(message);
    }

    Ok(pid)
}

fn resolve_selected_skin_file_path(
    plan: &LaunchPlan,
    selected_skin_url: Option<&str>,
    cancel_state: Option<&SharedInstallCancel>,
) -> Option<PathBuf> {
    let raw_value = selected_skin_url
        .map(str::trim)
        .filter(|value| !value.is_empty())?;

    if let Some(decoded_data_url_path) = try_decode_data_url_skin(plan, raw_value) {
        return Some(decoded_data_url_path);
    }

    let path = PathBuf::from(raw_value);

    if path.is_file() {
        return Some(path);
    }

    if let Some(recovered_path) = recover_missing_local_skin_path(plan, raw_value) {
        eprintln!(
            "Recovered missing selected skin file from launcher_skins: {}",
            recovered_path.display()
        );
        return Some(recovered_path);
    }

    if let Some(parsed_file_path) = try_resolve_file_url_path(raw_value) {
        if parsed_file_path.is_file() {
            return Some(parsed_file_path);
        }
    }

    if raw_value.starts_with("https://") || raw_value.starts_with("http://") {
        match download_selected_skin_url(plan, raw_value, cancel_state) {
            Ok(downloaded_path) => return Some(downloaded_path),
            Err(error) => {
                eprintln!("Failed to download selected skin from URL {raw_value}: {error}");
            }
        }
    }

    None
}

fn try_decode_data_url_skin(plan: &LaunchPlan, value: &str) -> Option<PathBuf> {
    let trimmed = value.trim();
    let lowered = trimmed.to_ascii_lowercase();
    if !lowered.starts_with("data:image/png;base64,") {
        return None;
    }

    let comma_index = trimmed.find(',')?;
    let encoded = trimmed.get(comma_index + 1..)?.trim();
    if encoded.is_empty() {
        return None;
    }

    let decoded = base64::engine::general_purpose::STANDARD
        .decode(encoded.as_bytes())
        .ok()?;
    if decoded.len() < 8 || &decoded[..8] != b"\x89PNG\r\n\x1a\n" {
        return None;
    }

    let cache_dir = plan.working_dir.join("launcher_skins").join("resolved");
    ensure_directory_ready(&cache_dir).ok()?;

    let mut hasher = Sha1::new();
    hasher.update(trimmed.as_bytes());
    let hash = format!("{:x}", hasher.finalize());
    let target_path = cache_dir.join(format!("selected-inline-{hash}.png"));

    fs::write(&target_path, decoded).ok()?;
    target_path.is_file().then_some(target_path)
}

fn is_inline_data_url(value: &str) -> bool {
    value
        .trim()
        .to_ascii_lowercase()
        .starts_with("data:image/png;base64,")
}

fn recover_missing_local_skin_path(plan: &LaunchPlan, raw_value: &str) -> Option<PathBuf> {
    let launcher_skins_dir = plan.working_dir.join("launcher_skins");
    if !launcher_skins_dir.is_dir() {
        return None;
    }

    let raw_path = PathBuf::from(raw_value);
    let raw_file_name = raw_path.file_name().and_then(|value| value.to_str())?;
    let uuid_prefix = extract_uuid_prefix_from_file_name(raw_file_name);

    let mut candidates = Vec::<(PathBuf, std::time::SystemTime)>::new();
    let entries = fs::read_dir(&launcher_skins_dir).ok()?;
    for entry in entries {
        let entry = entry.ok()?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let is_png = path
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|value| value.eq_ignore_ascii_case("png"));
        if !is_png {
            continue;
        }

        if let Some(prefix) = uuid_prefix.as_deref() {
            let file_name = entry.file_name();
            let file_name = file_name.to_string_lossy().to_ascii_lowercase();
            let expected_prefix = format!("{prefix}-");
            if !file_name.starts_with(&expected_prefix) {
                continue;
            }
        }

        let modified_time = entry.metadata().ok()?.modified().ok()?;
        candidates.push((path, modified_time));
    }

    if candidates.is_empty() {
        return None;
    }

    candidates.sort_by(|a, b| b.1.cmp(&a.1));
    candidates.into_iter().next().map(|entry| entry.0)
}

fn extract_uuid_prefix_from_file_name(file_name: &str) -> Option<String> {
    if file_name.len() < 36 {
        return None;
    }

    let prefix = &file_name[..36];
    Uuid::parse_str(prefix).ok()?;
    Some(prefix.to_ascii_lowercase())
}

fn try_resolve_file_url_path(value: &str) -> Option<PathBuf> {
    let stripped = value.strip_prefix("file://")?;
    let decoded = stripped
        .replace("%20", " ")
        .replace("%5C", "\\")
        .replace("%2F", "/");

    #[cfg(windows)]
    let normalized = {
        let without_leading_slash =
            if decoded.starts_with('/') && decoded.as_bytes().get(2) == Some(&b':') {
                &decoded[1..]
            } else {
                decoded.as_str()
            };

        without_leading_slash.replace('/', "\\")
    };

    #[cfg(not(windows))]
    let normalized = decoded;

    let path = PathBuf::from(normalized);
    path.is_file().then_some(path)
}

fn download_selected_skin_url(
    plan: &LaunchPlan,
    url: &str,
    cancel_state: Option<&SharedInstallCancel>,
) -> Result<PathBuf, String> {
    let cache_dir = plan.working_dir.join("launcher_skins").join("resolved");
    ensure_directory_ready(&cache_dir)?;

    let mut hasher = Sha1::new();
    hasher.update(url.as_bytes());
    let hash = format!("{:x}", hasher.finalize());
    let target_path = cache_dir.join(format!("selected-{hash}.png"));

    if target_path.is_file() {
        let _ = fs::remove_file(&target_path);
    }

    let http_client = build_http_client()?;
    download_to_path(&http_client, url, &target_path, None, None, cancel_state)?;

    if !target_path.is_file() {
        return Err(format!(
            "Downloaded skin file {} was not created.",
            target_path.display()
        ));
    }

    Ok(target_path)
}

fn resolve_skin_exchange_dir(plan: &LaunchPlan) -> PathBuf {
    if let Some(configured) = std::env::var_os("TBW_SKINS_DIR")
        .map(PathBuf::from)
        .filter(|value| !value.as_os_str().is_empty())
    {
        return configured;
    }

    plan.game_dir.join("TBWLauncherSkins").join("skins")
}

fn resolve_skin_cdn_base_url() -> Option<String> {
    let raw_value = crate::env_var_with_embedded_fallback("SKIN_CDN_BASE_URL")?;
    let normalized = raw_value.trim().trim_end_matches('/').to_string();
    if normalized.is_empty() {
        return None;
    }

    let lowercase = normalized.to_ascii_lowercase();
    if !lowercase.starts_with("http://") && !lowercase.starts_with("https://") {
        return None;
    }

    Some(normalized)
}

fn resolve_effective_skin_cdn_base_url(
    selected_skin_source: Option<&str>,
    launcher_skin_records: &[LauncherSkinRecord],
) -> Option<String> {
    if let Some(configured) = resolve_skin_cdn_base_url() {
        return Some(configured);
    }

    if let Some(source) = selected_skin_source {
        if let Some(derived) = derive_skin_cdn_base_url_from_url(source) {
            return Some(derived);
        }
    }

    for record in launcher_skin_records {
        if let Some(derived) = derive_skin_cdn_base_url_from_url(record.skin_url.as_str()) {
            return Some(derived);
        }
    }

    None
}

fn derive_skin_cdn_base_url_from_url(value: &str) -> Option<String> {
    let normalized = value
        .trim()
        .split('#')
        .next()
        .unwrap_or("")
        .split('?')
        .next()
        .unwrap_or("")
        .trim_end_matches('/');
    if normalized.is_empty() {
        return None;
    }

    let lowered = normalized.to_ascii_lowercase();
    if !lowered.starts_with("http://") && !lowered.starts_with("https://") {
        return None;
    }

    let slash_index = normalized.rfind('/')?;
    let file_name = normalized.get(slash_index + 1..)?.trim();
    if file_name.is_empty() {
        return None;
    }

    if !file_name.to_ascii_lowercase().ends_with(".png") {
        return None;
    }

    let mut base = normalized[..slash_index].trim_end_matches('/').to_string();
    let base_lower = base.to_ascii_lowercase();
    if base_lower.ends_with("/skins-upload") {
        let prefix = &base[..base.len() - "/skins-upload".len()];
        base = format!("{prefix}/skins");
    } else if !base_lower.ends_with("/skins") {
        return None;
    }

    if base.is_empty() {
        return None;
    }

    Some(base)
}

fn build_nickname_cdn_skin_url(base_url: &str, nickname: &str) -> Option<String> {
    let normalized_nickname = sanitize_cdn_skin_file_stem(nickname);
    if normalized_nickname.is_empty() {
        return None;
    }

    Some(format!("{base_url}/{normalized_nickname}.png"))
}

fn sanitize_cdn_skin_file_stem(value: &str) -> String {
    value
        .trim()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>()
}

fn apply_selected_skin_environment(
    command: &mut Command,
    selected_skin_source: Option<&str>,
    selected_skin_path: Option<&Path>,
    nickname: &str,
    skin_exchange_dir: &Path,
    skin_cdn_base_url: Option<&str>,
) {
    if let Some(source) = selected_skin_source.filter(|value| !is_inline_data_url(value)) {
        command.env("TBW_SKIN_SOURCE", source);
    }

    if let Some(path) = selected_skin_path {
        command.env("TBW_SKIN_PATH", path);
    }

    command.env("TBW_SKINS_DIR", skin_exchange_dir);
    if let Some(value) = skin_cdn_base_url {
        command.env("TBW_SKIN_CDN_BASE", value);
    }

    let trimmed_nickname = nickname.trim();
    if !trimmed_nickname.is_empty() {
        command.env("TBW_PLAYER_NAME", trimmed_nickname);
    }
}

fn ensure_launch_paths_writable(plan: &LaunchPlan) -> Result<(), String> {
    let launch_paths = [
        plan.working_dir.join("logs"),
        plan.game_dir.join("config"),
        plan.game_dir.join("logs"),
    ];

    for path in launch_paths {
        if !path.exists() {
            continue;
        }

        clear_readonly_flags_recursive(&path)?;
    }

    Ok(())
}

fn clear_readonly_flags_recursive(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }

    let metadata = fs::metadata(path)
        .map_err(|error| format!("Failed to read metadata for {}: {error}", path.display()))?;
    let mut permissions = metadata.permissions();
    if permissions.readonly() {
        permissions.set_readonly(false);
        fs::set_permissions(path, permissions).map_err(|error| {
            format!(
                "Failed to remove read-only flag from {}: {error}",
                path.display()
            )
        })?;
    }

    if !metadata.is_dir() {
        return Ok(());
    }

    let entries = fs::read_dir(path)
        .map_err(|error| format!("Failed to read directory {}: {error}", path.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| {
            format!(
                "Failed to read directory entry in {}: {error}",
                path.display()
            )
        })?;
        clear_readonly_flags_recursive(&entry.path())?;
    }

    Ok(())
}

fn process_exited_early(pid: u32, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if !process_is_running(pid) {
            return true;
        }

        thread::sleep(Duration::from_millis(120));
    }

    false
}

fn read_text_file_tail(path: &Path, max_lines: usize) -> Option<String> {
    if max_lines == 0 {
        return Some(String::new());
    }

    let contents = fs::read_to_string(path).ok()?;
    let mut lines = contents.lines().rev().take(max_lines).collect::<Vec<_>>();
    lines.reverse();

    Some(lines.join("\n"))
}

fn sync_launcher_player_skins_into_game(
    plan: &LaunchPlan,
    current_nickname: &str,
    current_selected_skin_source: Option<&str>,
    current_selected_skin_path: Option<&Path>,
    launcher_skin_records: &[LauncherSkinRecord],
    skin_exchange_dir: &Path,
    skin_cdn_base_url: Option<&str>,
    cancel_state: Option<&SharedInstallCancel>,
) -> Result<(), String> {
    let mut synced_nickname_keys = HashSet::new();
    let current_nickname_key = current_nickname.trim().to_ascii_lowercase();
    let mut synced_players_count = 0usize;

    if skin_cdn_base_url.is_some() {
        if let Some(source_skin_path) = current_selected_skin_path {
            sync_selected_skin_into_game(
                plan,
                current_nickname,
                source_skin_path,
                skin_exchange_dir,
            )?;
            eprintln!(
                "Prepared selected launcher skin for current player {current_nickname} (CDN mode)."
            );
            return Ok(());
        }

        if let Some(source) = current_selected_skin_source.filter(|value| !value.trim().is_empty())
        {
            if let Some(source_skin_path) =
                resolve_selected_skin_file_path(plan, Some(source), cancel_state)
            {
                sync_selected_skin_into_game(
                    plan,
                    current_nickname,
                    source_skin_path.as_path(),
                    skin_exchange_dir,
                )?;
                eprintln!(
                    "Prepared selected launcher skin for current player {current_nickname} using source fallback (CDN mode)."
                );
                return Ok(());
            }
        }

        eprintln!(
            "CDN mode is enabled. Skipping bulk local skin sync and relying on runtime CDN fetch for other players."
        );
        return Ok(());
    }

    if let Some(source_skin_path) = current_selected_skin_path {
        sync_selected_skin_into_game(plan, current_nickname, source_skin_path, skin_exchange_dir)?;
        if !current_nickname_key.is_empty() {
            synced_nickname_keys.insert(current_nickname_key.clone());
        }
        synced_players_count += 1;
    }

    let mut resolved_source_cache: HashMap<String, Option<PathBuf>> = HashMap::new();

    for record in launcher_skin_records {
        let nickname = record.nickname.trim();
        if nickname.is_empty() {
            continue;
        }

        let nickname_key = nickname.to_ascii_lowercase();
        if synced_nickname_keys.contains(&nickname_key) {
            continue;
        }

        ensure_download_not_cancelled(cancel_state)?;

        let skin_source = record.skin_url.trim();
        if skin_source.is_empty() {
            continue;
        }

        let mut resolved_skin_path = if let Some(cached) = resolved_source_cache.get(skin_source) {
            cached.clone()
        } else {
            let resolved = resolve_selected_skin_file_path(plan, Some(skin_source), cancel_state);
            resolved_source_cache.insert(skin_source.to_string(), resolved.clone());
            resolved
        };

        if resolved_skin_path.is_none() {
            if let Some(cdn_url) = skin_cdn_base_url
                .as_deref()
                .and_then(|base_url| build_nickname_cdn_skin_url(base_url, nickname))
            {
                resolved_skin_path = if let Some(cached) = resolved_source_cache.get(&cdn_url) {
                    cached.clone()
                } else {
                    let resolved =
                        resolve_selected_skin_file_path(plan, Some(cdn_url.as_str()), cancel_state);
                    resolved_source_cache.insert(cdn_url.clone(), resolved.clone());
                    resolved
                };

                if resolved_skin_path.is_some() {
                    eprintln!(
                        "Resolved launcher skin for player {nickname} using CDN fallback: {cdn_url}"
                    );
                }
            }
        }

        let Some(source_skin_path) = resolved_skin_path else {
            eprintln!(
                "Failed to resolve launcher skin source for player {nickname}: {skin_source}"
            );
            continue;
        };

        if let Err(error) = sync_selected_skin_into_game(
            plan,
            nickname,
            source_skin_path.as_path(),
            skin_exchange_dir,
        ) {
            eprintln!("Failed to sync launcher skin for player {nickname}: {error}");
            continue;
        }

        synced_nickname_keys.insert(nickname_key);
        synced_players_count += 1;
    }

    if current_selected_skin_path.is_none() {
        if let Some(source) = current_selected_skin_source.filter(|value| !value.trim().is_empty())
        {
            if let Some(source_skin_path) =
                resolve_selected_skin_file_path(plan, Some(source), cancel_state)
            {
                if let Err(error) = sync_selected_skin_into_game(
                    plan,
                    current_nickname,
                    source_skin_path.as_path(),
                    skin_exchange_dir,
                ) {
                    eprintln!(
                        "Failed to sync selected launcher skin for {current_nickname}: {error}"
                    );
                } else if !current_nickname_key.is_empty() {
                    synced_nickname_keys.insert(current_nickname_key.clone());
                    synced_players_count += 1;
                }
            }
        }
    }

    eprintln!("Prepared launcher skins for {synced_players_count} player(s) before game launch.");

    Ok(())
}

fn sync_selected_skin_into_game(
    plan: &LaunchPlan,
    nickname: &str,
    source_skin_path: &Path,
    skin_exchange_dir: &Path,
) -> Result<(), String> {
    if !source_skin_path.is_file() {
        return Err(format!(
            "Selected skin file {} was not found.",
            source_skin_path.display()
        ));
    }

    let player_file_names = skin_player_file_name_candidates(nickname);
    let target_roots = skin_sync_target_roots(plan, skin_exchange_dir);

    for target_root in target_roots {
        fs::create_dir_all(&target_root).map_err(|error| {
            format!(
                "Failed to create directory {}: {error}",
                target_root.display()
            )
        })?;

        for player_file_name in &player_file_names {
            let target_path = target_root.join(format!("{player_file_name}.png"));
            fs::copy(source_skin_path, &target_path).map_err(|error| {
                format!(
                    "Failed to copy selected skin from {} to {}: {error}",
                    source_skin_path.display(),
                    target_path.display()
                )
            })?;
        }
    }

    Ok(())
}

fn skin_sync_target_roots(plan: &LaunchPlan, skin_exchange_dir: &Path) -> Vec<PathBuf> {
    let mut roots = Vec::with_capacity(2);
    roots.push(skin_exchange_dir.to_path_buf());
    roots.push(plan.game_dir.join("TBWLauncherSkins").join("skins"));

    let mut seen = HashSet::new();
    roots.retain(|root| seen.insert(root.clone()));
    roots
}

fn skin_player_file_name_candidates(nickname: &str) -> Vec<String> {
    let trimmed_nickname = nickname.trim();
    let sanitized_nickname = sanitize_player_file_name(trimmed_nickname);
    let offline_uuid = offline_uuid_for_player(trimmed_nickname);
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    let candidates = [
        trimmed_nickname.to_string(),
        trimmed_nickname.to_ascii_lowercase(),
        sanitized_nickname.clone(),
        sanitized_nickname.to_ascii_lowercase(),
        offline_uuid.clone(),
        offline_uuid.replace('-', ""),
    ];

    for candidate in candidates {
        let safe_candidate = sanitize_player_file_name(candidate.as_str());
        if safe_candidate.is_empty() {
            continue;
        }

        if seen.insert(safe_candidate.clone()) {
            result.push(safe_candidate);
        }
    }

    if result.is_empty() {
        result.push("Player".to_string());
    }

    result
}

fn sanitize_player_file_name(value: &str) -> String {
    let sanitized = value
        .trim()
        .chars()
        .map(|ch| {
            if matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') || ch.is_control()
            {
                '_'
            } else {
                ch
            }
        })
        .collect::<String>();

    if sanitized.is_empty() {
        "Player".to_string()
    } else {
        sanitized
    }
}

#[cfg(windows)]
fn build_debug_powershell_script(java_executable: &OsStr, args: &[OsString]) -> String {
    let mut parts = Vec::with_capacity(args.len() + 1);
    parts.push(quote_powershell_argument(java_executable));

    for arg in args {
        parts.push(quote_powershell_argument(arg.as_os_str()));
    }

    format!(
    "$ErrorActionPreference = 'Continue'; Write-Host '===== Java Output ====='; Write-Host ''; & {}; Write-Host ''; Write-Host ('Process exited with code ' + $LASTEXITCODE + '. This console will stay open for inspection.');",
    parts.join(" ")
  )
}

#[cfg(windows)]
fn quote_powershell_argument(value: &OsStr) -> String {
    let escaped = value.to_string_lossy().replace('\'', "''");
    format!("'{escaped}'")
}

fn build_java_arguments(
    plan: &LaunchPlan,
    settings: &settings::LauncherSettings,
    nickname: &str,
    selected_skin_source: Option<&str>,
    selected_skin_path: Option<&Path>,
    skin_exchange_dir: &Path,
    skin_cdn_base_url: Option<&str>,
) -> Result<Vec<OsString>, String> {
    let mut args = split_command_line(&settings.java_args)?
        .into_iter()
        .filter(|arg| {
            let lowercase = arg.to_ascii_lowercase();
            !lowercase.starts_with("-xmx") && !lowercase.starts_with("-xms")
        })
        .map(OsString::from)
        .collect::<Vec<_>>();

    let ram_mb = settings.ram_mb.max(1024);

    args.push(OsString::from(format!("-Xms{ram_mb}M")));
    args.push(OsString::from(format!("-Xmx{ram_mb}M")));

    if plan.jvm_argument_tokens.is_empty() {
        args.push(OsString::from(format!(
            "-Djava.library.path={}",
            plan.natives_dir.display()
        )));
        args.push(OsString::from(format!(
            "-Dminecraft.client.jar={}",
            plan.client_jar.display()
        )));
        args.push(OsString::from(
            "-Dfml.ignoreInvalidMinecraftCertificates=true",
        ));
        args.push(OsString::from("-Dfml.ignorePatchDiscrepancies=true"));
        if let Some(logging_argument) = &plan.logging_argument {
            args.push(logging_argument.clone());
        }
        args.push(OsString::from("-cp"));
        args.push(plan.classpath.clone());
    } else {
        args.extend(
            plan.jvm_argument_tokens
                .iter()
                .map(|token| OsString::from(apply_argument_replacements(token, plan, None))),
        );

        if let Some(logging_argument) = &plan.logging_argument {
            args.push(logging_argument.clone());
        }
    }

    if let Some(path) = selected_skin_path {
        args.push(OsString::from(format!(
            "-Dtbw.skin.path={}",
            path.display()
        )));
    }

    if let Some(source) = selected_skin_source.filter(|value| !is_inline_data_url(value)) {
        args.push(OsString::from(format!("-Dtbw.skin.source={source}")));
    }

    args.push(OsString::from(format!(
        "-Dtbw.skins.dir={}",
        skin_exchange_dir.display()
    )));
    if let Some(value) = skin_cdn_base_url {
        args.push(OsString::from(format!("-Dtbw.skin.cdn.base={value}")));
    }

    let trimmed_nickname = nickname.trim();
    if !trimmed_nickname.is_empty() {
        args.push(OsString::from(format!(
            "-Dtbw.player.name={trimmed_nickname}"
        )));
    }

    args.push(OsString::from(&plan.main_class));

    Ok(args)
}

fn build_game_arguments(plan: &LaunchPlan, nickname: &str) -> Vec<OsString> {
    plan.game_argument_tokens
        .iter()
        .map(|token| OsString::from(apply_argument_replacements(token, plan, Some(nickname))))
        .collect()
}

fn apply_argument_replacements(token: &str, plan: &LaunchPlan, nickname: Option<&str>) -> String {
    let player_name = nickname.unwrap_or("Player");
    let uuid = offline_uuid_for_player(player_name);
    let game_dir = plan.game_dir.display().to_string();
    let assets_dir = plan.assets_dir.display().to_string();
    let libraries_dir = plan.libraries_dir.display().to_string();
    let natives_dir = plan.natives_dir.display().to_string();
    let classpath = plan.classpath.to_string_lossy().to_string();
    let classpath_separator = classpath_separator();
    let replacements = [
        ("${auth_player_name}", player_name),
        ("${version_name}", plan.version_id.as_str()),
        ("${game_directory}", game_dir.as_str()),
        ("${assets_root}", assets_dir.as_str()),
        ("${assets_index_name}", plan.asset_index_name.as_str()),
        ("${auth_uuid}", uuid.as_str()),
        ("${auth_access_token}", "0"),
        ("${clientid}", "0"),
        ("${auth_xuid}", "0"),
        ("${user_properties}", "{}"),
        ("${user_type}", "legacy"),
        ("${version_type}", "TBW Launcher"),
        ("${natives_directory}", natives_dir.as_str()),
        ("${launcher_name}", "TBWLauncher"),
        ("${launcher_version}", "0.1"),
        ("${classpath}", classpath.as_str()),
        ("${classpath_separator}", classpath_separator),
        ("${library_directory}", libraries_dir.as_str()),
    ];

    let mut resolved = token.to_string();
    for (placeholder, value) in replacements {
        resolved = resolved.replace(placeholder, value);
    }

    resolved
}

fn classpath_separator() -> &'static str {
    #[cfg(windows)]
    {
        ";"
    }

    #[cfg(not(windows))]
    {
        ":"
    }
}

fn resolve_manifest_arguments(
    manifest: &serde_json::Value,
    inherited_manifest: Option<&serde_json::Value>,
    kind: &str,
) -> Result<Vec<String>, String> {
    let mut tokens = Vec::new();
    let mut has_new_style_arguments = false;

    if let Some(parent) = inherited_manifest {
        if let Some(argument_list) = parent["arguments"][kind].as_array() {
            has_new_style_arguments = true;
            append_manifest_argument_entries(&mut tokens, argument_list);
        }
    }

    if let Some(argument_list) = manifest["arguments"][kind].as_array() {
        has_new_style_arguments = true;
        append_manifest_argument_entries(&mut tokens, argument_list);
    }

    if has_new_style_arguments {
        return Ok(tokens);
    }

    if kind == "game" {
        if let Some(value) = manifest["minecraftArguments"].as_str() {
            return Ok(value.split_whitespace().map(str::to_string).collect());
        }

        if let Some(parent) = inherited_manifest {
            if let Some(value) = parent["minecraftArguments"].as_str() {
                return Ok(value.split_whitespace().map(str::to_string).collect());
            }
        }

        return Err("The selected launch profile does not contain launch arguments.".to_string());
    }

    Ok(Vec::new())
}

fn append_manifest_argument_entries(tokens: &mut Vec<String>, argument_list: &[serde_json::Value]) {
    for entry in argument_list {
        if !manifest_argument_entry_is_allowed(entry) {
            continue;
        }

        if let Some(value) = entry.as_str() {
            tokens.push(value.to_string());
            continue;
        }

        if let Some(values) = entry["value"].as_array() {
            tokens.extend(
                values
                    .iter()
                    .filter_map(|value| value.as_str())
                    .map(str::to_string),
            );
            continue;
        }

        if let Some(value) = entry["value"].as_str() {
            tokens.push(value.to_string());
        }
    }
}

fn manifest_argument_entry_is_allowed(entry: &serde_json::Value) -> bool {
    let Some(rules) = entry["rules"].as_array() else {
        return true;
    };

    let mut allowed = false;

    for rule in rules {
        if !rule_matches_current_os(rule) || !rule_matches_features(rule) {
            continue;
        }

        allowed = rule["action"].as_str() == Some("allow");
    }

    allowed
}

fn rule_matches_features(rule: &serde_json::Value) -> bool {
    let Some(features) = rule["features"].as_object() else {
        return true;
    };

    features
        .iter()
        .all(|(_, value)| value.as_bool().is_none_or(|flag| !flag))
}

fn resolve_launch_plan(
    mode_name: &str,
    preferred_game_version: Option<&str>,
    cancel_state: Option<&SharedInstallCancel>,
) -> Result<LaunchPlan, String> {
    ensure_download_not_cancelled(cancel_state)?;
    let tbw_root = find_tbw_root()?;
    let http_client = build_http_client()?;
    let versions_dir = tbw_root.join("versions");
    let game_dir = resolve_game_dir(&tbw_root, &versions_dir, mode_name)?;
    let launch_version =
        resolve_launch_version_manifest(&versions_dir, &game_dir, preferred_game_version)?;
    let launch_manifest = &launch_version.manifest;
    let inherited_version = ensure_inherited_manifest_available(
        &tbw_root,
        &versions_dir,
        launch_manifest,
        &http_client,
        cancel_state,
    )?;
    let inherited_manifest = inherited_version.as_ref().map(|item| &item.manifest);
    let base_version_id = launch_manifest["inheritsFrom"]
        .as_str()
        .or_else(|| launch_manifest["jar"].as_str())
        .unwrap_or(launch_version.version_name.as_str());

    let main_class = launch_manifest["mainClass"]
        .as_str()
        .ok_or_else(|| "The selected launch profile does not contain mainClass.".to_string())?
        .to_string();
    let version_id = launch_manifest["id"]
        .as_str()
        .unwrap_or(launch_version.version_name.as_str())
        .to_string();
    let game_argument_tokens =
        resolve_manifest_arguments(launch_manifest, inherited_manifest, "game")?;
    let jvm_argument_tokens =
        resolve_manifest_arguments(launch_manifest, inherited_manifest, "jvm").unwrap_or_default();

    let client_version_name = inherited_version
        .as_ref()
        .map(|item| item.version_name.as_str())
        .unwrap_or(base_version_id);
    let client_manifest = inherited_manifest.unwrap_or(launch_manifest);
    let client_jar = ensure_client_jar_available(
        &versions_dir,
        client_version_name,
        client_manifest,
        &http_client,
        cancel_state,
    )?;

    let libraries_dir = tbw_root.join("libraries");
    ensure_directory_ready(&libraries_dir)?;

    let natives_root = tbw_root.join("natives");
    ensure_directory_ready(&natives_root)?;
    let natives_dir = ensure_natives_ready(
        &libraries_dir,
        &natives_root,
        launch_version.version_name.as_str(),
        &version_id,
        client_version_name,
        launch_manifest,
        inherited_manifest,
        &http_client,
        cancel_state,
    )?;

    let assets_dir = tbw_root.join("assets");
    ensure_directory_ready(&assets_dir)?;

    let asset_index_name =
        ensure_asset_index_and_objects(&assets_dir, client_manifest, &http_client, cancel_state)?;
    let logging_argument =
        ensure_logging_config_argument(&assets_dir, client_manifest, &http_client, cancel_state)?;
    let library_jars = resolve_manifest_library_paths(
        &libraries_dir,
        launch_manifest,
        inherited_manifest,
        &http_client,
        cancel_state,
    )?;
    let classpath = build_classpath(
        &library_jars,
        &client_jar,
        should_include_client_jar_in_classpath(
            &version_id,
            &main_class,
            &jvm_argument_tokens,
            &library_jars,
        ),
    )?;
    let required_java_major = required_java_major_version(client_manifest);
    let java_executable = resolve_java_executable(&tbw_root, required_java_major)?;

    Ok(LaunchPlan {
        java_executable,
        working_dir: tbw_root,
        game_dir,
        assets_dir,
        libraries_dir,
        natives_dir,
        client_jar,
        classpath,
        main_class,
        version_id,
        jvm_argument_tokens,
        game_argument_tokens,
        asset_index_name,
        logging_argument,
    })
}

fn build_classpath(
    library_jars: &[PathBuf],
    client_jar: &Path,
    include_client_jar: bool,
) -> Result<OsString, String> {
    let mut entries = library_jars.to_vec();
    if include_client_jar {
        entries.push(client_jar.to_path_buf());
    }

    std::env::join_paths(entries).map_err(|error| format!("Failed to build the classpath: {error}"))
}

fn should_include_client_jar_in_classpath(
    version_id: &str,
    main_class: &str,
    jvm_argument_tokens: &[String],
    library_jars: &[PathBuf],
) -> bool {
    if has_minecraft_srg_client_jar(library_jars) {
        return false;
    }

    let is_forge_like = version_id.to_ascii_lowercase().contains("forge");
    if !is_forge_like {
        return true;
    }

    if main_class == "cpw.mods.bootstraplauncher.BootstrapLauncher" {
        return false;
    }

    let has_ignore_list_hint = jvm_argument_tokens.iter().any(|token| {
        let lower = token.to_ascii_lowercase();
        lower.contains("ignorelist=") && lower.contains("${version_name}.jar")
    });

    !has_ignore_list_hint
}

fn has_minecraft_srg_client_jar(library_jars: &[PathBuf]) -> bool {
    library_jars.iter().any(|path| {
        let normalized = path
            .to_string_lossy()
            .replace('\\', "/")
            .to_ascii_lowercase();
        normalized.contains("/net/minecraft/client/") && normalized.ends_with("-srg.jar")
    })
}

fn required_java_major_version(manifest: &serde_json::Value) -> u32 {
    manifest["javaVersion"]["majorVersion"]
        .as_u64()
        .and_then(|value| u32::try_from(value).ok())
        .filter(|value| *value > 0)
        .unwrap_or(8)
}

fn resolve_manifest_library_paths(
    libraries_dir: &Path,
    manifest: &serde_json::Value,
    inherited_manifest: Option<&serde_json::Value>,
    http_client: &Client,
    cancel_state: Option<&SharedInstallCancel>,
) -> Result<Vec<PathBuf>, String> {
    let mut merged = Vec::<(String, PathBuf)>::new();

    if let Some(parent) = inherited_manifest {
        collect_manifest_library_paths(
            libraries_dir,
            parent,
            &mut merged,
            http_client,
            cancel_state,
        )?;
    }

    collect_manifest_library_paths(
        libraries_dir,
        manifest,
        &mut merged,
        http_client,
        cancel_state,
    )?;

    let paths = merged.into_iter().map(|(_, path)| path).collect::<Vec<_>>();

    if paths.is_empty() {
        return Err("No launch libraries were resolved from the manifests.".to_string());
    }

    Ok(paths)
}

fn collect_manifest_library_paths(
    libraries_dir: &Path,
    manifest: &serde_json::Value,
    out: &mut Vec<(String, PathBuf)>,
    http_client: &Client,
    cancel_state: Option<&SharedInstallCancel>,
) -> Result<(), String> {
    let Some(libraries) = manifest["libraries"].as_array() else {
        return Ok(());
    };

    for library in libraries {
        ensure_download_not_cancelled(cancel_state)?;
        if !library_is_allowed(library) {
            continue;
        }

        if library["include_in_classpath"].as_bool() == Some(false) {
            continue;
        }

        let Some(relative_path) = library["downloads"]["artifact"]["path"].as_str() else {
            continue;
        };

        let jar_path = libraries_dir.join(relative_path);
        let library_name = library["name"].as_str().unwrap_or(relative_path);

        ensure_library_artifact(
            library,
            library_name,
            relative_path,
            &jar_path,
            http_client,
            cancel_state,
        )?;

        let key = library_merge_key(library).unwrap_or_else(|| relative_path.to_string());

        if let Some(index) = out
            .iter()
            .position(|(existing_key, _)| *existing_key == key)
        {
            out[index] = (key, jar_path);
        } else {
            out.push((key, jar_path));
        }
    }

    Ok(())
}

fn ensure_library_artifact(
    library: &serde_json::Value,
    library_name: &str,
    relative_path: &str,
    jar_path: &Path,
    http_client: &Client,
    cancel_state: Option<&SharedInstallCancel>,
) -> Result<(), String> {
    ensure_download_not_cancelled(cancel_state)?;
    if jar_path.is_file() {
        return Ok(());
    }

    let primary_url = library["downloads"]["artifact"]["url"]
        .as_str()
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            library["name"]
                .as_str()
                .filter(|name| name.starts_with("net.minecraftforge:"))
                .map(|_| format!("https://maven.minecraftforge.net/{relative_path}"))
        });

    let expected_sha1 = library["downloads"]["artifact"]["sha1"].as_str();
    let expected_size = library["downloads"]["artifact"]["size"].as_u64();
    let mut candidate_urls = Vec::new();

    if let Some(url) = primary_url {
        candidate_urls.push(url);
    }

    if library["name"]
        .as_str()
        .is_some_and(|name| name.starts_with("net.minecraftforge:forge:"))
    {
        if let Some(universal_relative_path) = forge_universal_relative_path(relative_path) {
            let universal_url =
                format!("https://maven.minecraftforge.net/{universal_relative_path}");
            if !candidate_urls.iter().any(|item| item == &universal_url) {
                candidate_urls.push(universal_url);
            }
        }
    }

    if candidate_urls.is_empty() {
        return Err(format!("Missing download URL for library: {library_name}"));
    }

    let mut last_error = None;

    for url in candidate_urls {
        ensure_download_not_cancelled(cancel_state)?;
        match download_to_path(
            http_client,
            &url,
            jar_path,
            expected_sha1,
            expected_size,
            cancel_state,
        ) {
            Ok(()) => return Ok(()),
            Err(error) => last_error = Some(format!("{url} failed: {error}")),
        }
    }

    Err(format!(
        "Failed to download library {library_name}: {}",
        last_error.unwrap_or_else(|| "unknown error".to_string())
    ))
}

fn forge_universal_relative_path(relative_path: &str) -> Option<String> {
    let extension_index = relative_path.rfind(".jar")?;
    let (prefix, suffix) = relative_path.split_at(extension_index);

    Some(format!("{prefix}-universal{suffix}"))
}

fn ensure_asset_index_and_objects(
    assets_dir: &Path,
    manifest: &serde_json::Value,
    http_client: &Client,
    cancel_state: Option<&SharedInstallCancel>,
) -> Result<String, String> {
    ensure_download_not_cancelled(cancel_state)?;
    let indexes_dir = assets_dir.join("indexes");
    let objects_dir = assets_dir.join("objects");
    ensure_directory_ready(&indexes_dir)?;
    ensure_directory_ready(&objects_dir)?;

    let (asset_index_name, asset_index_url, asset_index_sha1, asset_index_size) =
        resolve_asset_index_metadata(manifest)?;
    let asset_index_path = indexes_dir.join(format!("{asset_index_name}.json"));

    if !asset_index_path.is_file() {
        download_to_path(
            http_client,
            &asset_index_url,
            &asset_index_path,
            asset_index_sha1.as_deref(),
            asset_index_size,
            cancel_state,
        )
        .map_err(|error| format!("Failed to download asset index {asset_index_name}: {error}"))?;
    }

    ensure_asset_objects(&asset_index_path, &objects_dir, http_client, cancel_state)?;

    Ok(asset_index_name)
}

fn ensure_logging_config_argument(
    assets_dir: &Path,
    manifest: &serde_json::Value,
    http_client: &Client,
    cancel_state: Option<&SharedInstallCancel>,
) -> Result<Option<OsString>, String> {
    ensure_download_not_cancelled(cancel_state)?;
    let Some(client_logging) = manifest["logging"]["client"].as_object() else {
        return Ok(None);
    };
    let Some(argument_template) = client_logging
        .get("argument")
        .and_then(|value| value.as_str())
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };
    let Some(file_meta) = client_logging.get("file") else {
        return Ok(None);
    };
    let Some(file_id) = file_meta["id"].as_str().filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let Some(file_url) = file_meta["url"].as_str().filter(|value| !value.is_empty()) else {
        return Ok(None);
    };

    let log_configs_dir = assets_dir.join("log_configs");
    ensure_directory_ready(&log_configs_dir)?;
    let target_path = log_configs_dir.join(file_id);

    if !target_path.is_file() {
        download_to_path(
            http_client,
            file_url,
            &target_path,
            file_meta["sha1"].as_str(),
            file_meta["size"].as_u64(),
            cancel_state,
        )
        .map_err(|error| format!("Failed to download logging config {file_id}: {error}"))?;
    }

    Ok(Some(OsString::from(
        argument_template.replace("${path}", &target_path.display().to_string()),
    )))
}

fn ensure_asset_objects(
    asset_index_path: &Path,
    objects_dir: &Path,
    http_client: &Client,
    cancel_state: Option<&SharedInstallCancel>,
) -> Result<(), String> {
    ensure_download_not_cancelled(cancel_state)?;
    let index_raw = fs::read_to_string(asset_index_path).map_err(|error| {
        format!(
            "Failed to read asset index {}: {error}",
            asset_index_path.display()
        )
    })?;
    let index_json: serde_json::Value = serde_json::from_str(&index_raw).map_err(|error| {
        format!(
            "Asset index {} is invalid: {error}",
            asset_index_path.display()
        )
    })?;
    let objects = index_json["objects"].as_object().ok_or_else(|| {
        format!(
            "Asset index {} does not contain objects.",
            asset_index_path.display()
        )
    })?;

    for (_, entry) in objects {
        ensure_download_not_cancelled(cancel_state)?;
        let Some(hash) = entry["hash"].as_str() else {
            continue;
        };
        let prefix = hash
            .get(..2)
            .ok_or_else(|| format!("Asset hash is invalid: {hash}"))?;
        let target_path = objects_dir.join(prefix).join(hash);

        if target_path.is_file() {
            continue;
        }

        let download_url = format!("https://resources.download.minecraft.net/{prefix}/{hash}");
        let expected_size = entry["size"].as_u64();

        download_to_path(
            http_client,
            &download_url,
            &target_path,
            Some(hash),
            expected_size,
            cancel_state,
        )
        .map_err(|error| format!("Failed to download asset object {hash}: {error}"))?;
    }

    Ok(())
}

fn resolve_asset_index_metadata(
    manifest: &serde_json::Value,
) -> Result<(String, String, Option<String>, Option<u64>), String> {
    let asset_index_name = manifest_asset_index_name(manifest)
        .ok_or_else(|| "No asset index was found in the version manifest.".to_string())?;
    let asset_index_url = manifest["assetIndex"]["url"]
    .as_str()
    .filter(|value| !value.is_empty())
    .ok_or_else(|| format!("The version manifest does not contain a download URL for asset index {asset_index_name}."))?
    .to_string();
    let asset_index_sha1 = manifest["assetIndex"]["sha1"].as_str().map(str::to_string);
    let asset_index_size = manifest["assetIndex"]["size"].as_u64();

    Ok((
        asset_index_name,
        asset_index_url,
        asset_index_sha1,
        asset_index_size,
    ))
}

fn ensure_inherited_manifest_available(
    tbw_root: &Path,
    versions_dir: &Path,
    manifest: &serde_json::Value,
    http_client: &Client,
    cancel_state: Option<&SharedInstallCancel>,
) -> Result<Option<ResolvedVersionManifest>, String> {
    let Some(parent_version_id) = manifest["inheritsFrom"]
        .as_str()
        .or_else(|| manifest["jar"].as_str())
    else {
        return Ok(None);
    };

    ensure_version_manifest_available(
        tbw_root,
        versions_dir,
        parent_version_id,
        http_client,
        cancel_state,
    )
    .map(Some)
}

fn ensure_version_manifest_available(
    tbw_root: &Path,
    versions_dir: &Path,
    version_id: &str,
    http_client: &Client,
    cancel_state: Option<&SharedInstallCancel>,
) -> Result<ResolvedVersionManifest, String> {
    ensure_download_not_cancelled(cancel_state)?;
    if let Some(manifest) = read_exact_version_manifest(versions_dir, version_id)? {
        return Ok(manifest);
    }

    let global_manifest = read_global_version_manifest(tbw_root)?;
    let manifest_url = find_version_manifest_url(&global_manifest, version_id)?;
    let manifest_path = versions_dir
        .join(version_id)
        .join(format!("{version_id}.json"));

    download_to_path(
        http_client,
        &manifest_url,
        &manifest_path,
        None,
        None,
        cancel_state,
    )
    .map_err(|error| format!("Failed to download version manifest {version_id}: {error}"))?;

    read_exact_version_manifest(versions_dir, version_id)?.ok_or_else(|| {
        format!("Version manifest {version_id} was downloaded but could not be read.")
    })
}

fn ensure_client_jar_available(
    versions_dir: &Path,
    version_name: &str,
    manifest: &serde_json::Value,
    http_client: &Client,
    cancel_state: Option<&SharedInstallCancel>,
) -> Result<PathBuf, String> {
    ensure_download_not_cancelled(cancel_state)?;
    let client_jar = versions_dir
        .join(version_name)
        .join(format!("{version_name}.jar"));

    if client_jar.is_file() {
        return Ok(client_jar);
    }

    let client_url = manifest["downloads"]["client"]["url"]
        .as_str()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            format!("The version manifest {version_name} does not contain a client jar URL.")
        })?;
    let client_sha1 = manifest["downloads"]["client"]["sha1"].as_str();
    let client_size = manifest["downloads"]["client"]["size"].as_u64();

    download_to_path(
        http_client,
        client_url,
        &client_jar,
        client_sha1,
        client_size,
        cancel_state,
    )
    .map_err(|error| format!("Failed to download client jar {version_name}: {error}"))?;

    Ok(client_jar)
}

fn resolve_launch_version_manifest(
    versions_dir: &Path,
    game_dir: &Path,
    preferred_game_version: Option<&str>,
) -> Result<ResolvedVersionManifest, String> {
    if let Some(game_dir_name) = game_dir.file_name().and_then(|value| value.to_str()) {
        let local_manifest_path = game_dir.join(format!("{game_dir_name}.json"));
        if local_manifest_path.is_file() {
            let manifest = read_manifest_file(&local_manifest_path, game_dir_name)?;
            return Ok(ResolvedVersionManifest {
                version_name: game_dir_name.to_string(),
                manifest,
            });
        }
    }

    let entries = fs::read_dir(versions_dir).map_err(|error| {
        format!(
            "Failed to read directory {}: {error}",
            versions_dir.display()
        )
    })?;
    let mut best_match: Option<(usize, ResolvedVersionManifest)> = None;

    for entry in entries {
        let entry = entry.map_err(|error| {
            format!(
                "Failed to read directory entry in {}: {error}",
                versions_dir.display()
            )
        })?;
        let version_dir = entry.path();
        if !version_dir.is_dir() {
            continue;
        }

        let directory_name = entry.file_name().to_string_lossy().to_string();
        let manifest_path = version_dir.join(format!("{directory_name}.json"));
        if !manifest_path.is_file() {
            continue;
        }

        let manifest = match read_manifest_file(&manifest_path, &directory_name) {
            Ok(value) => value,
            Err(_) => continue,
        };

        if manifest["mainClass"].as_str().is_none() {
            continue;
        }

        let score = launch_profile_priority(&directory_name, &manifest, preferred_game_version);
        let candidate = ResolvedVersionManifest {
            version_name: directory_name,
            manifest,
        };

        match &best_match {
            Some((best_score, _)) if *best_score <= score => {}
            _ => best_match = Some((score, candidate)),
        }
    }

    best_match.map(|(_, value)| value).ok_or_else(|| {
        "No launch profile was found in .tbw/versions. Add a version manifest with mainClass first."
            .to_string()
    })
}

fn launch_profile_priority(
    directory_name: &str,
    manifest: &serde_json::Value,
    preferred_game_version: Option<&str>,
) -> usize {
    let manifest_id = manifest["id"].as_str().unwrap_or_default();
    let base_version = manifest["inheritsFrom"]
        .as_str()
        .or_else(|| manifest["jar"].as_str())
        .unwrap_or_default();
    let joined = format!(
        "{} {} {}",
        directory_name.to_ascii_lowercase(),
        manifest_id.to_ascii_lowercase(),
        base_version.to_ascii_lowercase()
    );
    let is_loader_profile = joined.contains("forge")
        || joined.contains("fabric")
        || joined.contains("quilt")
        || joined.contains("neoforge")
        || manifest["inheritsFrom"].as_str().is_some();

    let mut score: usize = if is_loader_profile { 0 } else { 1000 };

    if let Some(preferred_version) = preferred_game_version
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let version_key = preferred_version.to_ascii_lowercase();
        let exact_match = directory_name.eq_ignore_ascii_case(preferred_version)
            || manifest_id.eq_ignore_ascii_case(preferred_version)
            || base_version.eq_ignore_ascii_case(preferred_version);

        if exact_match {
            score += 0;
        } else if joined.contains(&version_key) {
            score += 150;
        } else {
            score += 1500;
        }
    }

    score + directory_name.len()
}

fn ensure_natives_ready(
    libraries_dir: &Path,
    natives_root: &Path,
    launch_version_name: &str,
    version_id: &str,
    base_version_id: &str,
    manifest: &serde_json::Value,
    inherited_manifest: Option<&serde_json::Value>,
    http_client: &Client,
    cancel_state: Option<&SharedInstallCancel>,
) -> Result<PathBuf, String> {
    ensure_download_not_cancelled(cancel_state)?;
    let target_dir = resolve_natives_dir(
        natives_root,
        launch_version_name,
        version_id,
        base_version_id,
    )?;
    ensure_directory_ready(&target_dir)?;

    if let Some(parent) = inherited_manifest {
        extract_manifest_natives(
            libraries_dir,
            &target_dir,
            parent,
            http_client,
            cancel_state,
        )?;
    }

    extract_manifest_natives(
        libraries_dir,
        &target_dir,
        manifest,
        http_client,
        cancel_state,
    )?;

    Ok(target_dir)
}

fn resolve_natives_dir(
    natives_root: &Path,
    launch_version_name: &str,
    version_id: &str,
    base_version_id: &str,
) -> Result<PathBuf, String> {
    let candidates = natives_dir_candidates(launch_version_name, version_id, base_version_id);

    for candidate in &candidates {
        let candidate_path = natives_root.join(candidate);
        if candidate_path.is_dir() {
            return Ok(candidate_path);
        }
    }

    if let Some(matched_existing) = find_matching_natives_dir(natives_root, &candidates)? {
        return Ok(matched_existing);
    }

    let selected_name = candidates
        .into_iter()
        .find(|item| !item.is_empty())
        .unwrap_or_else(|| "default".to_string());

    Ok(natives_root.join(selected_name))
}

fn natives_dir_candidates(
    launch_version_name: &str,
    version_id: &str,
    base_version_id: &str,
) -> Vec<String> {
    let mut candidates = Vec::new();
    let stripped_launch_version = strip_loader_marker(launch_version_name);
    let stripped_version_id = strip_loader_marker(version_id);

    for value in [
        launch_version_name,
        version_id,
        stripped_launch_version.as_str(),
        stripped_version_id.as_str(),
        base_version_id,
    ] {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            continue;
        }

        if !candidates.iter().any(|existing| existing == trimmed) {
            candidates.push(trimmed.to_string());
        }
    }

    candidates
}

fn strip_loader_marker(value: &str) -> String {
    value
        .replace("-forge-", "-")
        .replace("-fabric-", "-")
        .replace("-quilt-", "-")
        .replace("-neoforge-", "-")
}

fn find_matching_natives_dir(
    natives_root: &Path,
    candidate_names: &[String],
) -> Result<Option<PathBuf>, String> {
    let candidate_keys = candidate_names
        .iter()
        .map(|item| normalize_mode_name(item))
        .filter(|item| !item.is_empty())
        .collect::<Vec<_>>();

    if candidate_keys.is_empty() {
        return Ok(None);
    }

    let entries = fs::read_dir(natives_root).map_err(|error| {
        format!(
            "Failed to read directory {}: {error}",
            natives_root.display()
        )
    })?;
    let mut best_match: Option<(usize, PathBuf)> = None;

    for entry in entries {
        let entry = entry.map_err(|error| {
            format!(
                "Failed to read directory entry in {}: {error}",
                natives_root.display()
            )
        })?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let name = entry.file_name().to_string_lossy().to_string();
        let name_key = normalize_mode_name(&name);
        if name_key.is_empty() {
            continue;
        }

        let score = candidate_keys
            .iter()
            .filter_map(|candidate| {
                if name_key == *candidate {
                    Some(0usize)
                } else if name_key.contains(candidate) || candidate.contains(&name_key) {
                    Some(name_key.len().abs_diff(candidate.len()) + 5)
                } else {
                    None
                }
            })
            .min();

        let Some(score) = score else {
            continue;
        };

        match &best_match {
            Some((best_score, _)) if *best_score <= score => {}
            _ => best_match = Some((score, path)),
        }
    }

    Ok(best_match.map(|(_, path)| path))
}

fn extract_manifest_natives(
    libraries_dir: &Path,
    target_dir: &Path,
    manifest: &serde_json::Value,
    http_client: &Client,
    cancel_state: Option<&SharedInstallCancel>,
) -> Result<(), String> {
    ensure_download_not_cancelled(cancel_state)?;
    let mut merged = Vec::<(String, serde_json::Value)>::new();
    let Some(libraries) = manifest["libraries"].as_array() else {
        return Ok(());
    };

    for library in libraries {
        if !library_is_allowed(library) {
            continue;
        }

        if !library_has_natives_for_current_os(library) {
            continue;
        }

        let key = library_merge_key(library)
            .unwrap_or_else(|| library["name"].as_str().unwrap_or_default().to_string());

        if let Some(index) = merged
            .iter()
            .position(|(existing_key, _)| *existing_key == key)
        {
            merged[index] = (key, library.clone());
        } else {
            merged.push((key, library.clone()));
        }
    }

    for (_, library) in merged {
        ensure_native_classifier_extracted(
            libraries_dir,
            target_dir,
            &library,
            http_client,
            cancel_state,
        )?;
    }

    Ok(())
}

fn library_has_natives_for_current_os(library: &serde_json::Value) -> bool {
    let os_name = current_minecraft_os_name();
    library["natives"]
        .get(os_name)
        .and_then(|value| value.as_str())
        .is_some()
}

fn ensure_native_classifier_extracted(
    libraries_dir: &Path,
    target_dir: &Path,
    library: &serde_json::Value,
    http_client: &Client,
    cancel_state: Option<&SharedInstallCancel>,
) -> Result<(), String> {
    ensure_download_not_cancelled(cancel_state)?;
    let os_name = current_minecraft_os_name();
    let classifier_key = library["natives"][os_name].as_str().ok_or_else(|| {
        "The native library entry is missing the current OS classifier.".to_string()
    })?;
    let classifier_meta = &library["downloads"]["classifiers"][classifier_key];
    let relative_path = classifier_meta["path"]
        .as_str()
        .ok_or_else(|| format!("The native classifier {classifier_key} is missing its path."))?;
    let classifier_url = classifier_meta["url"]
        .as_str()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("The native classifier {classifier_key} is missing its URL."))?;
    let classifier_path = libraries_dir.join(relative_path);

    if !classifier_path.is_file() {
        download_to_path(
            http_client,
            classifier_url,
            &classifier_path,
            classifier_meta["sha1"].as_str(),
            classifier_meta["size"].as_u64(),
            cancel_state,
        )
        .map_err(|error| {
            format!("Failed to download native classifier {classifier_key}: {error}")
        })?;
    }

    extract_native_archive(&classifier_path, target_dir, library, cancel_state)
}

fn extract_native_archive(
    archive_path: &Path,
    target_dir: &Path,
    library: &serde_json::Value,
    cancel_state: Option<&SharedInstallCancel>,
) -> Result<(), String> {
    ensure_download_not_cancelled(cancel_state)?;
    let archive_bytes = fs::read(archive_path).map_err(|error| {
        format!(
            "Failed to read native archive {}: {error}",
            archive_path.display()
        )
    })?;
    let cursor = Cursor::new(archive_bytes);
    let mut archive = ZipArchive::new(cursor).map_err(|error| {
        format!(
            "Failed to open native archive {}: {error}",
            archive_path.display()
        )
    })?;
    let excludes = library["extract"]["exclude"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|value| value.as_str().map(str::to_string))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    for index in 0..archive.len() {
        ensure_download_not_cancelled(cancel_state)?;
        let mut file = archive.by_index(index).map_err(|error| {
            format!(
                "Failed to read native archive entry in {}: {error}",
                archive_path.display()
            )
        })?;
        let entry_name = file.name().replace('\\', "/");

        if file.is_dir() || should_skip_extract_entry(&entry_name, &excludes) {
            continue;
        }

        let output_path = target_dir.join(&entry_name);
        if let Some(parent) = output_path.parent() {
            ensure_directory_ready(parent)?;
        }

        if output_path.is_file() {
            continue;
        }

        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer).map_err(|error| {
            format!(
                "Failed to extract native archive entry {} from {}: {error}",
                entry_name,
                archive_path.display()
            )
        })?;

        fs::write(&output_path, buffer).map_err(|error| {
            format!(
                "Failed to write extracted native file {}: {error}",
                output_path.display()
            )
        })?;
    }

    Ok(())
}

fn should_skip_extract_entry(entry_name: &str, excludes: &[String]) -> bool {
    excludes
        .iter()
        .any(|prefix| entry_name.starts_with(&prefix.replace('\\', "/")))
}

fn ensure_directory_ready(path: &Path) -> Result<(), String> {
    fs::create_dir_all(path)
        .map_err(|error| format!("Failed to create directory {}: {error}", path.display()))
}

fn build_http_client() -> Result<Client, String> {
    Client::builder()
        .user_agent("TBWLauncher/0.1")
        .build()
        .map_err(|error| format!("Failed to create the HTTP client: {error}"))
}

fn read_global_version_manifest(tbw_root: &Path) -> Result<serde_json::Value, String> {
    let manifest_path = tbw_root.join("version_manifest.json");
    let raw = fs::read_to_string(&manifest_path)
        .map_err(|error| format!("Failed to read {}: {error}", manifest_path.display()))?;

    serde_json::from_str(&raw)
        .map_err(|error| format!("The file {} is invalid: {error}", manifest_path.display()))
}

fn find_version_manifest_url(
    global_manifest: &serde_json::Value,
    version_id: &str,
) -> Result<String, String> {
    let versions = global_manifest["versions"].as_array().ok_or_else(|| {
        "The global version manifest does not contain a versions list.".to_string()
    })?;

    versions
        .iter()
        .find_map(|entry| {
            (entry["id"].as_str() == Some(version_id))
                .then(|| entry["url"].as_str())
                .flatten()
        })
        .filter(|url| !url.is_empty())
        .map(str::to_string)
        .ok_or_else(|| format!("Version {version_id} was not found in .tbw/version_manifest.json."))
}

fn download_to_path(
    http_client: &Client,
    url: &str,
    destination: &Path,
    expected_sha1: Option<&str>,
    expected_size: Option<u64>,
    cancel_state: Option<&SharedInstallCancel>,
) -> Result<(), String> {
    ensure_download_not_cancelled(cancel_state)?;
    if destination.is_file() {
        if let Some(size) = expected_size {
            if fs::metadata(destination)
                .map(|meta| meta.len() == size)
                .unwrap_or(false)
            {
                return Ok(());
            }
        } else {
            return Ok(());
        }
    }

    let bytes = download_bytes(http_client, url, cancel_state)?;
    verify_downloaded_bytes(&bytes, expected_sha1, expected_size)?;
    ensure_download_not_cancelled(cancel_state)?;

    if let Some(parent) = destination.parent() {
        ensure_directory_ready(parent)?;
    }

    let temp_path = destination.with_extension("part");
    fs::write(&temp_path, &bytes)
        .map_err(|error| format!("Failed to write {}: {error}", temp_path.display()))?;
    fs::rename(&temp_path, destination).map_err(|error| {
        format!(
            "Failed to move downloaded file into place {}: {error}",
            destination.display()
        )
    })
}

fn download_bytes(
    http_client: &Client,
    url: &str,
    cancel_state: Option<&SharedInstallCancel>,
) -> Result<Vec<u8>, String> {
    download_bytes_with_progress(
        http_client,
        url,
        cancel_state,
        Option::<fn(u64, Option<u64>)>::None,
    )
}

fn download_bytes_with_progress<F>(
    http_client: &Client,
    url: &str,
    cancel_state: Option<&SharedInstallCancel>,
    mut on_progress: Option<F>,
) -> Result<Vec<u8>, String>
where
    F: FnMut(u64, Option<u64>),
{
    ensure_download_not_cancelled(cancel_state)?;
    let mut response = http_client
        .get(url)
        .header("Accept-Encoding", "identity")
        .send()
        .map_err(|error| format!("Request failed for {url}: {error}"))?;

    if !response.status().is_success() {
        return Err(format!(
            "Request for {url} failed with status {}.",
            response.status()
        ));
    }

    let total_bytes = response.content_length();
    let mut buffer = Vec::with_capacity(total_bytes.unwrap_or(0) as usize);
    let mut chunk = [0u8; 64 * 1024];
    let mut downloaded_bytes = 0u64;

    loop {
        ensure_download_not_cancelled(cancel_state)?;
        let bytes_read = response
            .read(&mut chunk)
            .map_err(|error| format!("Failed to read response body from {url}: {error}"))?;

        if bytes_read == 0 {
            break;
        }

        buffer.extend_from_slice(&chunk[..bytes_read]);
        downloaded_bytes = downloaded_bytes.saturating_add(bytes_read as u64);

        if let Some(callback) = on_progress.as_mut() {
            callback(downloaded_bytes, total_bytes);
        }
    }

    Ok(buffer)
}

fn verify_downloaded_bytes(
    bytes: &[u8],
    expected_sha1: Option<&str>,
    expected_size: Option<u64>,
) -> Result<(), String> {
    if let Some(size) = expected_size {
        if bytes.len() as u64 != size {
            return Err(format!(
                "Downloaded file size mismatch: expected {size} bytes, got {} bytes.",
                bytes.len()
            ));
        }
    }

    if let Some(sha1) = expected_sha1 {
        let actual_sha1 = format!("{:x}", Sha1::digest(bytes));
        if !actual_sha1.eq_ignore_ascii_case(sha1) {
            return Err(format!(
                "Downloaded file checksum mismatch: expected {sha1}, got {actual_sha1}."
            ));
        }
    }

    Ok(())
}

fn read_exact_version_manifest(
    versions_dir: &Path,
    version_id: &str,
) -> Result<Option<ResolvedVersionManifest>, String> {
    let exact_version_dir = versions_dir.join(version_id);
    let exact_manifest_path = exact_version_dir.join(format!("{version_id}.json"));

    if !exact_manifest_path.is_file() {
        return Ok(None);
    }

    let manifest = read_manifest_file(&exact_manifest_path, version_id)?;
    Ok(Some(ResolvedVersionManifest {
        version_name: version_id.to_string(),
        manifest,
    }))
}

fn read_manifest_file(
    manifest_path: &Path,
    requested_version_id: &str,
) -> Result<serde_json::Value, String> {
    let manifest_raw = fs::read_to_string(manifest_path).map_err(|error| {
        format!("Failed to read version manifest {requested_version_id}: {error}")
    })?;

    serde_json::from_str(&manifest_raw)
        .map_err(|error| format!("Version manifest {requested_version_id} is invalid: {error}"))
}

fn manifest_asset_index_name(manifest: &serde_json::Value) -> Option<String> {
    manifest["assetIndex"]["id"]
        .as_str()
        .or_else(|| manifest["assets"].as_str())
        .map(str::to_string)
}

fn library_is_allowed(library: &serde_json::Value) -> bool {
    if !library_classifier_matches_current_arch(library) {
        return false;
    }

    let Some(rules) = library["rules"].as_array() else {
        return true;
    };

    let mut allowed = false;

    for rule in rules {
        if !rule_matches_current_os(rule) {
            continue;
        }

        allowed = rule["action"].as_str() == Some("allow");
    }

    allowed
}

fn rule_matches_current_os(rule: &serde_json::Value) -> bool {
    let Some(os_rule) = rule["os"].as_object() else {
        return true;
    };

    let current_os = current_minecraft_os_name();
    let current_arch = current_minecraft_arch_name();

    let name_matches = os_rule
        .get("name")
        .and_then(|value| value.as_str())
        .is_none_or(|name| name == current_os);
    let arch_matches = os_rule
        .get("arch")
        .and_then(|value| value.as_str())
        .is_none_or(|arch| arch == current_arch);

    name_matches && arch_matches
}

fn current_minecraft_os_name() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        "windows"
    }

    #[cfg(target_os = "macos")]
    {
        "osx"
    }

    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        "linux"
    }
}

fn current_minecraft_arch_name() -> &'static str {
    #[cfg(target_arch = "x86")]
    {
        "x86"
    }

    #[cfg(target_arch = "x86_64")]
    {
        "x86_64"
    }

    #[cfg(target_arch = "aarch64")]
    {
        "aarch64"
    }

    #[cfg(all(
        not(target_arch = "x86"),
        not(target_arch = "x86_64"),
        not(target_arch = "aarch64")
    ))]
    {
        std::env::consts::ARCH
    }
}

fn library_classifier_matches_current_arch(library: &serde_json::Value) -> bool {
    let Some(name) = library["name"].as_str() else {
        return true;
    };

    let mut segments = name.split(':');
    let _group = segments.next();
    let _artifact = segments.next();
    let _version = segments.next();
    let Some(classifier) = segments.next() else {
        return true;
    };

    classifier_matches_current_arch(classifier)
}

fn classifier_matches_current_arch(classifier: &str) -> bool {
    if !classifier.starts_with("natives-") {
        return true;
    }

    let expected_prefix = match current_minecraft_os_name() {
        "windows" => "natives-windows",
        "osx" => "natives-macos",
        "linux" => "natives-linux",
        _ => return true,
    };

    if !classifier.starts_with(expected_prefix) {
        return false;
    }

    let suffix = &classifier[expected_prefix.len()..];

    #[cfg(target_os = "windows")]
    {
        match suffix {
            "" => cfg!(target_arch = "x86_64"),
            "-x86" => cfg!(target_arch = "x86"),
            "-arm64" => cfg!(target_arch = "aarch64"),
            _ => false,
        }
    }

    #[cfg(target_os = "macos")]
    {
        match suffix {
            "" => !cfg!(target_arch = "aarch64"),
            "-arm64" => cfg!(target_arch = "aarch64"),
            _ => false,
        }
    }

    #[cfg(target_os = "linux")]
    {
        match suffix {
            "" => cfg!(target_arch = "x86_64"),
            "-x86" => cfg!(target_arch = "x86"),
            "-arm64" => cfg!(target_arch = "aarch64"),
            "-arm32" => false,
            "-riscv64" => cfg!(target_arch = "riscv64"),
            "-ppc64le" => cfg!(target_arch = "powerpc64"),
            _ => false,
        }
    }

    #[cfg(all(
        not(target_os = "windows"),
        not(target_os = "macos"),
        not(target_os = "linux")
    ))]
    {
        suffix.is_empty()
    }
}

fn library_merge_key(library: &serde_json::Value) -> Option<String> {
    let name = library["name"].as_str()?;
    let mut segments = name.split(':');
    let group = segments.next()?;
    let artifact = segments.next()?;
    let _version = segments.next()?;
    let classifier = segments.next();

    Some(match classifier {
        Some(value) if !value.is_empty() => format!("{group}:{artifact}:{value}"),
        _ => format!("{group}:{artifact}"),
    })
}

fn resolve_game_dir(
    tbw_root: &Path,
    versions_dir: &Path,
    mode_name: &str,
) -> Result<PathBuf, String> {
    ensure_directory_exists(versions_dir, "The .tbw/versions directory is missing.")?;

    if let Some(source) = try_find_build_source(tbw_root, mode_name)? {
        let target_dir = versions_dir.join(source_target_folder(&source));
        if build_directory_has_payload(&target_dir)? {
            return Ok(target_dir);
        }
    }

    let target = normalize_mode_name(mode_name);
    let entries = fs::read_dir(versions_dir).map_err(|error| {
        format!(
            "Failed to read directory {}: {error}",
            versions_dir.display()
        )
    })?;

    for entry in entries {
        let entry = entry.map_err(|error| {
            format!(
                "Failed to read directory entry in {}: {error}",
                versions_dir.display()
            )
        })?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let directory_name = entry.file_name();
        let directory_name = directory_name.to_string_lossy();
        if normalize_mode_name(&directory_name) == target {
            return Ok(path);
        }
    }

    Err(format!(
        "Build \"{mode_name}\" was not found in .tbw/versions."
    ))
}

fn resolve_java_executable(tbw_root: &Path, required_major: u32) -> Result<OsString, String> {
    let mut candidates = Vec::new();
    let bundled_java_roots = [tbw_root.join("runtime"), tbw_root.join("java_versions")];

    for runtime_dir in bundled_java_roots {
        if runtime_dir.exists() {
            collect_named_files(&runtime_dir, "java.exe", &mut candidates)?;
        }
    }

    collect_path_java_candidates(&mut candidates);
    collect_windows_java_install_candidates(&mut candidates);
    candidates.push(PathBuf::from("java"));

    let exact_major_preferred = prefers_exact_java_major(required_major);
    let mut exact_match: Option<PathBuf> = None;
    let mut best_compatible: Option<(u32, PathBuf)> = None;

    for candidate in candidates {
        let Some(version) = detect_java_major_version(&candidate) else {
            continue;
        };

        let is_compatible = if required_major <= 8 {
            version == 8
        } else {
            version >= required_major
        };

        if !is_compatible {
            continue;
        }

        if required_major <= 8 {
            return Ok(candidate.as_os_str().to_os_string());
        }

        if exact_major_preferred && version == required_major && exact_match.is_none() {
            exact_match = Some(candidate.clone());
        }

        match &best_compatible {
            Some((best_version, _)) if *best_version <= version => {}
            _ => best_compatible = Some((version, candidate)),
        }
    }

    if let Some(candidate) = exact_match {
        return Ok(candidate.as_os_str().to_os_string());
    }

    if let Some((_, candidate)) = best_compatible {
        return Ok(candidate.as_os_str().to_os_string());
    }

    if required_major <= 8 {
        return Err(
      "This build requires Java 8. Install Java 8 and make sure java.exe is available in PATH."
        .to_string(),
    );
    }

    Err(format!(
        "This build requires Java {required_major}+ but no compatible java.exe was found."
    ))
}

fn prefers_exact_java_major(required_major: u32) -> bool {
    matches!(required_major, 16 | 17)
}

fn collect_path_java_candidates(out: &mut Vec<PathBuf>) {
    let Some(path_value) = std::env::var_os("PATH") else {
        return;
    };

    for entry in std::env::split_paths(&path_value) {
        let candidate = entry.join("java.exe");
        if candidate.is_file() {
            out.push(candidate);
        }
    }
}

#[cfg(windows)]
fn collect_windows_java_install_candidates(out: &mut Vec<PathBuf>) {
    let mut roots = Vec::<PathBuf>::new();

    for env_key in ["ProgramFiles", "ProgramFiles(x86)"] {
        if let Some(base_dir) = std::env::var_os(env_key).map(PathBuf::from) {
            roots.push(base_dir.join("Java"));
            roots.push(base_dir.join("Eclipse Adoptium"));
            roots.push(base_dir.join("Microsoft"));
            roots.push(base_dir.join("Zulu"));
            roots.push(base_dir.join("BellSoft"));
        }
    }

    for root in roots {
        if !root.is_dir() {
            continue;
        }

        let Ok(entries) = fs::read_dir(&root) else {
            continue;
        };

        for entry in entries.flatten() {
            let java_candidate = entry.path().join("bin").join("java.exe");
            if java_candidate.is_file() {
                out.push(java_candidate);
            }
        }
    }
}

#[cfg(not(windows))]
fn collect_windows_java_install_candidates(_: &mut Vec<PathBuf>) {}

fn collect_named_files(dir: &Path, file_name: &str, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = fs::read_dir(dir)
        .map_err(|error| format!("Не удалось прочитать папку {}: {error}", dir.display()))?;

    for entry in entries {
        let entry = entry.map_err(|error| {
            format!(
                "Не удалось прочитать элемент папки {}: {error}",
                dir.display()
            )
        })?;
        let path = entry.path();

        if path.is_dir() {
            collect_named_files(&path, file_name, out)?;
            continue;
        }

        let matches_name = path
            .file_name()
            .and_then(|value| value.to_str())
            .is_some_and(|value| value.eq_ignore_ascii_case(file_name));

        if matches_name {
            out.push(path);
        }
    }

    Ok(())
}

fn detect_java_major_version(executable: &Path) -> Option<u32> {
    let output = Command::new(executable)
        .arg("-version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .ok()?;

    let mut combined = String::from_utf8_lossy(&output.stdout).to_string();
    combined.push_str(&String::from_utf8_lossy(&output.stderr));

    let version = combined.split('"').nth(1).map(str::to_string).or_else(|| {
        combined
            .split_whitespace()
            .find(|part| part.chars().next().is_some_and(|ch| ch.is_ascii_digit()))
            .map(str::to_string)
    })?;

    parse_java_major_version(&version)
}

fn parse_java_major_version(raw: &str) -> Option<u32> {
    let mut parts = raw
        .split(|ch: char| !(ch.is_ascii_digit() || ch == '.'))
        .find(|part| !part.is_empty())?
        .split('.');

    let first = parts.next()?.parse::<u32>().ok()?;
    if first == 1 {
        return parts.next()?.parse::<u32>().ok();
    }

    Some(first)
}

fn split_command_line(input: &str) -> Result<Vec<String>, String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut active_quote: Option<char> = None;
    let mut escaped = false;

    for ch in input.chars() {
        if escaped {
            current.push(ch);
            escaped = false;
            continue;
        }

        match ch {
            '\\' if active_quote.is_some() => escaped = true,
            '\'' | '"' => match active_quote {
                Some(quote) if quote == ch => active_quote = None,
                Some(_) => current.push(ch),
                None => active_quote = Some(ch),
            },
            _ if ch.is_whitespace() && active_quote.is_none() => {
                if !current.is_empty() {
                    args.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(ch),
        }
    }

    if escaped {
        current.push('\\');
    }

    if active_quote.is_some() {
        return Err("JAVA params contain an err".to_string());
    }

    if !current.is_empty() {
        args.push(current);
    }

    Ok(args)
}

fn normalize_mode_name(value: &str) -> String {
    value
        .chars()
        .flat_map(|ch| ch.to_lowercase())
        .filter(|ch| ch.is_alphanumeric())
        .collect()
}

fn offline_uuid_for_player(nickname: &str) -> String {
    let seed = format!("OfflinePlayer:{nickname}");
    let first = fnv1a64(seed.as_bytes(), 0xcbf2_9ce4_8422_2325);
    let second = fnv1a64(seed.as_bytes(), 0x8422_2325_cbf2_9ce4);

    let mut bytes = [0u8; 16];
    bytes[..8].copy_from_slice(&first.to_be_bytes());
    bytes[8..].copy_from_slice(&second.to_be_bytes());

    bytes[6] = (bytes[6] & 0x0f) | 0x30;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;

    format!(
    "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
    bytes[0],
    bytes[1],
    bytes[2],
    bytes[3],
    bytes[4],
    bytes[5],
    bytes[6],
    bytes[7],
    bytes[8],
    bytes[9],
    bytes[10],
    bytes[11],
    bytes[12],
    bytes[13],
    bytes[14],
    bytes[15]
  )
}

fn fnv1a64(bytes: &[u8], offset_basis: u64) -> u64 {
    let mut hash = offset_basis;

    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }

    hash
}

fn stop_process_tree(pid: u32) -> Result<(), String> {
    let mut command = Command::new("taskkill");
    command.arg("/PID").arg(pid.to_string()).arg("/T").arg("/F");

    configure_hidden_process(&mut command);

    command
        .status()
        .map(|_| ())
        .map_err(|error| format!("Не удалось закрыть игру: {error}"))
}

fn configure_process_spawn(command: &mut Command, show_logs: bool) {
    #[cfg(windows)]
    {
        let flags = if show_logs {
            CREATE_NEW_CONSOLE
        } else {
            CREATE_NO_WINDOW
        };

        command.creation_flags(flags);
    }

    if !show_logs {
        command.stdin(Stdio::null());
    }
}

fn configure_hidden_process(command: &mut Command) {
    #[cfg(windows)]
    {
        command.creation_flags(CREATE_NO_WINDOW);
    }

    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
}

fn ensure_directory_exists(path: &Path, message: &str) -> Result<(), String> {
    if path.is_dir() {
        return Ok(());
    }

    Err(message.to_string())
}

pub(crate) fn find_tbw_root() -> Result<PathBuf, String> {
    #[cfg(windows)]
    {
        if let Some(app_data) = std::env::var_os("APPDATA") {
            return Ok(PathBuf::from(app_data).join(".tbw"));
        }
    }

    let mut search_roots = Vec::new();

    if let Ok(current_dir) = std::env::current_dir() {
        search_roots.push(current_dir);
    }

    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(exe_dir) = current_exe.parent() {
            search_roots.push(exe_dir.to_path_buf());
        }
    }

    for root in search_roots {
        for candidate in root.ancestors() {
            let tbw_dir = candidate.join(".tbw");
            if tbw_dir.is_dir() {
                return Ok(tbw_dir);
            }
        }
    }

    Err("Не удалось определить папку .tbw".to_string())
}
