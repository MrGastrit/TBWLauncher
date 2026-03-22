use crate::auth::models::{
    AccountChangeStatus, AdminUserSummary, AuthResult, ChangePasswordPayload, LoginPayload,
    RegisterPayload, UpdateAccountPayload,
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
    user_id: Option<String>,
    identity: Option<String>,
    payload: UpdateAccountPayload,
) -> Result<(), String> {
    service::update_account(&state.pool, user_id, identity, payload)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn change_password(
    state: tauri::State<'_, AppState>,
    user_id: Option<String>,
    identity: Option<String>,
    payload: ChangePasswordPayload,
) -> Result<(), String> {
    service::change_password(&state.pool, user_id, identity, payload)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn get_account_change_status(
    state: tauri::State<'_, AppState>,
    user_id: Option<String>,
    identity: Option<String>,
) -> Result<AccountChangeStatus, String> {
    service::get_account_change_status(&state.pool, user_id, identity)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn upload_skin(
    state: tauri::State<'_, AppState>,
    user_id: Option<String>,
    identity: Option<String>,
    file_path: String,
) -> Result<String, String> {
    service::upload_skin(&state.pool, user_id, identity, file_path)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn upload_skin_data(
    state: tauri::State<'_, AppState>,
    user_id: Option<String>,
    identity: Option<String>,
    skin_name: Option<String>,
    skin_data_url: String,
) -> Result<String, String> {
    service::upload_skin_data(&state.pool, user_id, identity, skin_name, skin_data_url)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn set_skin_url(
    state: tauri::State<'_, AppState>,
    user_id: Option<String>,
    identity: Option<String>,
    skin_url: String,
) -> Result<(), String> {
    service::set_skin_url(&state.pool, user_id, identity, skin_url)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn admin_list_users(
    state: tauri::State<'_, AppState>,
    actor_user_id: Option<String>,
    actor_identity: Option<String>,
    search: Option<String>,
) -> Result<Vec<AdminUserSummary>, String> {
    service::admin_list_users(&state.pool, actor_user_id, actor_identity, search)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn admin_set_user_role(
    state: tauri::State<'_, AppState>,
    actor_user_id: Option<String>,
    actor_identity: Option<String>,
    target_nickname: String,
    role: String,
) -> Result<(), String> {
    service::admin_set_user_role(
        &state.pool,
        actor_user_id,
        actor_identity,
        target_nickname,
        role,
    )
    .await
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn admin_set_user_banned(
    state: tauri::State<'_, AppState>,
    actor_user_id: Option<String>,
    actor_identity: Option<String>,
    target_nickname: String,
    banned: bool,
) -> Result<(), String> {
    service::admin_set_user_banned(
        &state.pool,
        actor_user_id,
        actor_identity,
        target_nickname,
        banned,
    )
    .await
    .map_err(|error| error.to_string())
}
