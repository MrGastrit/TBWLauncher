use crate::auth::models::{
  AuthResult, ChangePasswordPayload, LoginPayload, RegisterPayload, UpdateAccountPayload,
};
use crate::auth::service;
use crate::AppState;

#[tauri::command]
pub async fn register(
  state: tauri::State<'_, AppState>,
  payload: RegisterPayload,
) -> Result<AuthResult, String> {
  service::register(&state.pool, payload)
    .await
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn login(
  state: tauri::State<'_, AppState>,
  payload: LoginPayload,
) -> Result<AuthResult, String> {
  service::login(&state.pool, payload)
    .await
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn update_account(
  state: tauri::State<'_, AppState>,
  payload: UpdateAccountPayload,
) -> Result<(), String> {
  service::update_account(&state.pool, payload)
    .await
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn change_password(
  state: tauri::State<'_, AppState>,
  payload: ChangePasswordPayload,
) -> Result<(), String> {
  service::change_password(&state.pool, payload)
    .await
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn upload_skin(
  state: tauri::State<'_, AppState>,
  file_path: String,
) -> Result<String, String> {
  service::upload_skin(&state.pool, file_path)
    .await
    .map_err(|error| error.to_string())
}
