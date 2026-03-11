mod auth;
mod discord_rpc;
mod game;
mod settings;

use sqlx::postgres::PgPoolOptions;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const EMBEDDED_ENV_FILE: &str = include_str!("../.env");

pub struct AppState {
    pub pool: sqlx::PgPool,
    pub running_game: Mutex<Option<game::RunningGame>>,
    pub install_progress: game::SharedInstallProgress,
    pub install_cancel: game::SharedInstallCancel,
}

pub(crate) fn env_var_with_embedded_fallback(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .or_else(|| read_embedded_env_value(key))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn read_embedded_env_value(key: &str) -> Option<String> {
    for line in EMBEDDED_ENV_FILE.lines() {
        let trimmed_line = line.trim();
        if trimmed_line.is_empty() || trimmed_line.starts_with('#') {
            continue;
        }

        let Some((line_key, line_value)) = trimmed_line.split_once('=') else {
            continue;
        };
        if line_key.trim() != key {
            continue;
        }

        let normalized_value = line_value.trim();
        if normalized_value.len() >= 2
            && ((normalized_value.starts_with('"') && normalized_value.ends_with('"'))
                || (normalized_value.starts_with('\'') && normalized_value.ends_with('\'')))
        {
            return Some(normalized_value[1..normalized_value.len() - 1].to_string());
        }

        return Some(normalized_value.to_string());
    }

    None
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    dotenvy::dotenv().ok();

    let mut updater_plugin_builder = tauri_plugin_updater::Builder::new();
    if let Some(updater_pubkey) = env_var_with_embedded_fallback("UPDATER_PUBLIC_KEY") {
        updater_plugin_builder = updater_plugin_builder.pubkey(updater_pubkey);
    }

    let database_url = env_var_with_embedded_fallback("DATABASE_URL")
        .expect("DATABASE_URL is not set in runtime environment or embedded .env");

    let pool = tauri::async_runtime::block_on(async {
        PgPoolOptions::new()
            .max_connections(10)
            .acquire_timeout(Duration::from_secs(5))
            .connect(&database_url)
            .await
    })
    .expect("Failed to connect to Postgres");

    tauri::Builder::default()
        .plugin(updater_plugin_builder.build())
        .plugin(tauri_plugin_process::init())
        .manage(AppState {
            pool,
            running_game: Mutex::new(None),
            install_progress: Arc::new(Mutex::new(None)),
            install_cancel: Arc::new(AtomicBool::new(false)),
        })
        .invoke_handler(tauri::generate_handler![
            auth::commands::register,
            auth::commands::login,
            auth::commands::update_account,
            auth::commands::change_password,
            auth::commands::get_account_change_status,
            auth::commands::upload_skin,
            auth::commands::upload_skin_data,
            auth::commands::set_skin_url,
            discord_rpc::update_discord_presence,
            game::get_build_installation_states,
            game::get_game_runtime_state,
            game::get_install_progress_state,
            game::cancel_active_downloads,
            game::install_build,
            game::toggle_game_runtime,
            settings::load_launcher_settings,
            settings::save_launcher_settings,
            settings::get_total_ram_mb,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
