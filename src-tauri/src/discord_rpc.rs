use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

const DISCORD_IPC_PIPE_COUNT: u8 = 10;
const DISCORD_OP_HANDSHAKE: i32 = 0;
const DISCORD_OP_FRAME: i32 = 1;
const DEFAULT_APPLICATION_ID: &str = "1477724606919086232";
const DEFAULT_LARGE_IMAGE_KEY: &str = "tbw_logo";
const BROWSING_STATE: &str = "Просматривает лаунчер";

static DISCORD_RPC_SESSION: OnceLock<Mutex<Option<DiscordRpcSession>>> = OnceLock::new();

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscordPresencePayload {
    pub active_mode_name: Option<String>,
    pub nickname: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DiscordRpcConfig {
    application_id: String,
    #[serde(default = "default_large_image_key")]
    large_image_key: String,
}

struct DiscordRpcSession {
    application_id: String,
    pipe: File,
}

fn default_large_image_key() -> String {
    DEFAULT_LARGE_IMAGE_KEY.to_string()
}

#[tauri::command]
pub async fn update_discord_presence(payload: DiscordPresencePayload) -> Result<(), String> {
    let tbw_root = crate::game::find_tbw_root()?;

    tauri::async_runtime::spawn_blocking(move || update_presence(&tbw_root, payload))
        .await
        .map_err(|error| format!("Failed to join the Discord RPC task: {error}"))?
}

pub fn update_presence(tbw_root: &Path, payload: DiscordPresencePayload) -> Result<(), String> {
    let config = read_discord_rpc_config(tbw_root)?;
    let state = payload
        .active_mode_name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| format!("Играет в {value}"))
        .unwrap_or_else(|| BROWSING_STATE.to_string());
    let details = payload
        .nickname
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("TBW Launcher");

    let session_lock = DISCORD_RPC_SESSION.get_or_init(|| Mutex::new(None));
    let mut session = session_lock
        .lock()
        .map_err(|_| "Failed to lock the Discord RPC session.".to_string())?;

    let needs_reconnect = session
        .as_ref()
        .map(|active_session| active_session.application_id != config.application_id)
        .unwrap_or(true);

    if needs_reconnect {
        *session = connect_discord_session(&config.application_id)?;
    }

    let Some(active_session) = session.as_mut() else {
        return Ok(());
    };

    if let Err(first_error) =
        send_activity(active_session, &config.large_image_key, details, &state)
    {
        *session = connect_discord_session(&config.application_id)?;

        let Some(reconnected_session) = session.as_mut() else {
            return Ok(());
        };

        if let Err(second_error) = send_activity(
            reconnected_session,
            &config.large_image_key,
            details,
            &state,
        ) {
            *session = None;
            return Err(format!(
        "Failed to update Discord RPC activity: {first_error}. Retry failed: {second_error}"
      ));
        }
    }

    Ok(())
}

fn connect_discord_session(application_id: &str) -> Result<Option<DiscordRpcSession>, String> {
    let Some(mut pipe) = open_discord_ipc_pipe() else {
        return Ok(None);
    };

    send_frame(
        &mut pipe,
        DISCORD_OP_HANDSHAKE,
        &json!({
          "v": 1,
          "client_id": application_id,
        }),
    )?;

    Ok(Some(DiscordRpcSession {
        application_id: application_id.to_string(),
        pipe,
    }))
}

fn send_activity(
    session: &mut DiscordRpcSession,
    large_image_key: &str,
    details: &str,
    state: &str,
) -> Result<(), String> {
    send_frame(
        &mut session.pipe,
        DISCORD_OP_FRAME,
        &json!({
          "cmd": "SET_ACTIVITY",
          "args": {
            "pid": std::process::id(),
            "activity": {
              "details": details,
              "state": state,
              "assets": {
                "large_image": large_image_key,
                "large_text": "TBW Launcher"
              }
            }
          },
          "nonce": uuid::Uuid::new_v4().to_string(),
        }),
    )
}

fn read_discord_rpc_config(tbw_root: &Path) -> Result<DiscordRpcConfig, String> {
    let config_path = ensure_discord_rpc_config_exists(tbw_root)?;
    let raw = fs::read_to_string(&config_path)
        .map_err(|error| format!("Failed to read {}: {error}", config_path.display()))?;
    let config = serde_json::from_str::<DiscordRpcConfig>(&raw)
        .map_err(|error| format!("The file {} is invalid: {error}", config_path.display()))?;

    Ok(DiscordRpcConfig {
        application_id: if config.application_id.trim().is_empty() {
            DEFAULT_APPLICATION_ID.to_string()
        } else {
            config.application_id.trim().to_string()
        },
        large_image_key: if config.large_image_key.trim().is_empty() {
            DEFAULT_LARGE_IMAGE_KEY.to_string()
        } else {
            config.large_image_key.trim().to_string()
        },
    })
}

fn ensure_discord_rpc_config_exists(tbw_root: &Path) -> Result<PathBuf, String> {
    let config_path = resolve_discord_rpc_config_path(tbw_root);
    if config_path.is_file() {
        return Ok(config_path);
    }

    let template = DiscordRpcConfig {
        application_id: DEFAULT_APPLICATION_ID.to_string(),
        large_image_key: DEFAULT_LARGE_IMAGE_KEY.to_string(),
    };
    let serialized = serde_json::to_string_pretty(&template)
        .map_err(|error| format!("Failed to serialize {}: {error}", config_path.display()))?;

    if let Some(parent) = config_path.parent() {
        if let Err(error) = fs::create_dir_all(parent) {
            return write_discord_rpc_config_fallback(tbw_root, serialized, &config_path, error);
        }
    }

    match fs::write(&config_path, &serialized) {
        Ok(()) => Ok(config_path),
        Err(error) => write_discord_rpc_config_fallback(tbw_root, serialized, &config_path, error),
    }
}

fn resolve_discord_rpc_config_path(tbw_root: &Path) -> PathBuf {
    let mut candidates = Vec::new();

    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(exe_dir) = current_exe.parent() {
            candidates.push(exe_dir.join("discord_rpc.json"));
        }
    }

    if let Ok(current_dir) = std::env::current_dir() {
        let path = current_dir.join("discord_rpc.json");
        if !candidates.iter().any(|candidate| candidate == &path) {
            candidates.push(path);
        }
    }

    if let Some(existing) = candidates.iter().find(|path| path.is_file()) {
        return existing.clone();
    }

    let legacy_path = tbw_root.join("discord_rpc.json");
    if legacy_path.is_file() {
        return legacy_path;
    }

    candidates.into_iter().next().unwrap_or(legacy_path)
}

fn write_discord_rpc_config_fallback(
    tbw_root: &Path,
    serialized: String,
    attempted_path: &Path,
    original_error: std::io::Error,
) -> Result<PathBuf, String> {
    let fallback_path = tbw_root.join("discord_rpc.json");
    if let Some(parent) = fallback_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create {}: {error}", parent.display()))?;
    }

    fs::write(&fallback_path, serialized).map_err(|error| {
        format!(
            "Failed to write {} ({original_error}) and fallback {} ({error}).",
            attempted_path.display(),
            fallback_path.display()
        )
    })?;

    Ok(fallback_path)
}

fn open_discord_ipc_pipe() -> Option<File> {
    (0..DISCORD_IPC_PIPE_COUNT).find_map(|index| {
        let path = PathBuf::from(format!(r"\\?\pipe\discord-ipc-{index}"));
        OpenOptions::new().read(true).write(true).open(path).ok()
    })
}

fn send_frame(
    stream: &mut impl Write,
    opcode: i32,
    payload: &serde_json::Value,
) -> Result<(), String> {
    let body = serde_json::to_vec(payload)
        .map_err(|error| format!("Failed to encode Discord RPC payload: {error}"))?;
    let body_len =
        i32::try_from(body.len()).map_err(|_| "Discord RPC payload is too large.".to_string())?;

    stream
        .write_all(&opcode.to_le_bytes())
        .and_then(|_| stream.write_all(&body_len.to_le_bytes()))
        .and_then(|_| stream.write_all(&body))
        .map_err(|error| format!("Failed to write the Discord RPC frame: {error}"))
}
