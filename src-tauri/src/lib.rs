mod auth;
mod game;
mod settings;

use sqlx::postgres::PgPoolOptions;
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub struct AppState {
  pub pool: sqlx::PgPool,
  pub running_game: Mutex<Option<game::RunningGame>>,
  pub install_progress: game::SharedInstallProgress,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  dotenvy::dotenv().ok();

  let database_url = std::env::var("DATABASE_URL")
    .expect("DATABASE_URL is not set");

  let pool = tauri::async_runtime::block_on(async {
    PgPoolOptions::new()
      .max_connections(10)
      .acquire_timeout(Duration::from_secs(5))
      .connect(&database_url)
      .await
  })
  .expect("Failed to connect to Postgres");

  tauri::Builder::default()
    .manage(AppState {
      pool,
      running_game: Mutex::new(None),
      install_progress: Arc::new(Mutex::new(None)),
    })
    .invoke_handler(tauri::generate_handler![
      auth::commands::register,
      auth::commands::login,
      auth::commands::update_account,
      auth::commands::change_password,
      auth::commands::upload_skin,
      game::get_build_installation_states,
      game::get_game_runtime_state,
      game::get_install_progress_state,
      game::install_build,
      game::toggle_game_runtime,
      settings::load_launcher_settings,
      settings::save_launcher_settings,
      settings::get_total_ram_mb,
    ])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
