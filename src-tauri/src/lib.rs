mod auth;
mod settings;

use sqlx::postgres::PgPoolOptions;
use std::time::Duration;

pub struct AppState {
  pub pool: sqlx::PgPool,
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
    .manage(AppState { pool })
    .invoke_handler(tauri::generate_handler![
      auth::commands::register,
      auth::commands::login,
      auth::commands::update_account,
      auth::commands::change_password,
      auth::commands::upload_skin,
      settings::load_launcher_settings,
      settings::save_launcher_settings,
      settings::get_total_ram_mb,
    ])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
