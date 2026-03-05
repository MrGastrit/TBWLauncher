use crate::auth::errors::AuthError;
use crate::auth::models::{
  AuthResult, AuthTokens, AuthUser, ChangePasswordPayload, LoginPayload, RegisterPayload, UpdateAccountPayload,
};
use crate::auth::repository;
use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine as _;
use sqlx::PgPool;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub async fn register(pool: &PgPool, payload: RegisterPayload) -> Result<AuthResult, AuthError> {
  if payload.password != payload.repeat_password {
    return Err(AuthError::Validation("Пароли не совпадают.".into()));
  }

  let existing_email = repository::find_user_by_identity(pool, &payload.email)
    .await
    .map_err(|error| AuthError::Internal(error.to_string()))?;

  if existing_email.is_some() {
    return Err(AuthError::Validation("Пользователь с таким email уже существует.".into()));
  }

  let existing_nickname = repository::find_user_by_identity(pool, &payload.nickname)
    .await
    .map_err(|error| AuthError::Internal(error.to_string()))?;

  if existing_nickname.is_some() {
    return Err(AuthError::Validation("Ник уже занят.".into()));
  }

  let password_hash = hash_password(&payload.password)?;

  let user = repository::create_user(pool, &payload, &password_hash)
    .await
    .map_err(|error| AuthError::Internal(error.to_string()))?;

  Ok(AuthResult {
    user: AuthUser {
      id: user.id,
      nickname: user.nickname,
      email: user.email,
      skin_url: user.skin_url,
      role: user.role,
    },
    tokens: AuthTokens {
      access_token: Uuid::new_v4().to_string(),
      refresh_token: None,
    },
  })
}

pub async fn login(pool: &PgPool, payload: LoginPayload) -> Result<AuthResult, AuthError> {
  let user = repository::find_user_by_identity(pool, &payload.identity)
    .await
    .map_err(|error| AuthError::Internal(error.to_string()))?
    .ok_or_else(|| AuthError::Validation("Неверный логин или пароль.".into()))?;

  verify_password(&payload.password, &user.password_hash)
    .map_err(|_| AuthError::Validation("Неверный логин или пароль.".into()))?;

  Ok(AuthResult {
    user: AuthUser {
      id: user.id,
      nickname: user.nickname,
      email: user.email,
      skin_url: user.skin_url,
      role: user.role,
    },
    tokens: AuthTokens {
      access_token: Uuid::new_v4().to_string(),
      refresh_token: None,
    },
  })
}

pub async fn update_account(_pool: &PgPool, _payload: UpdateAccountPayload) -> Result<(), AuthError> {
  Err(AuthError::NotImplemented("update_account service is not implemented yet"))
}

pub async fn change_password(_pool: &PgPool, _payload: ChangePasswordPayload) -> Result<(), AuthError> {
  Err(AuthError::NotImplemented("change_password service is not implemented yet"))
}

pub async fn set_skin_url(
  pool: &PgPool,
  user_id: Option<String>,
  identity: Option<String>,
  skin_url: String,
) -> Result<(), AuthError> {
  let resolved_user_id = resolve_user_id(pool, user_id.as_deref(), identity.as_deref()).await?;
  let normalized_skin_url = skin_url.trim();
  if normalized_skin_url.is_empty() {
    return Err(AuthError::Validation("Skin URL is required.".into()));
  }

  repository::update_skin_url(pool, resolved_user_id.as_str(), Some(normalized_skin_url))
    .await
    .map_err(|error| AuthError::Internal(error.to_string()))
}

pub async fn upload_skin(
  pool: &PgPool,
  user_id: Option<String>,
  identity: Option<String>,
  file_path: String,
) -> Result<String, AuthError> {
  let resolved_user_id = resolve_user_id(pool, user_id.as_deref(), identity.as_deref()).await?;

  let source_path = PathBuf::from(file_path.trim());
  if !source_path.is_file() {
    return Err(AuthError::Validation("Skin file was not found.".into()));
  }

  let extension = source_path
    .extension()
    .and_then(|value| value.to_str())
    .map(|value| value.to_ascii_lowercase())
    .unwrap_or_default();
  if extension != "png" {
    return Err(AuthError::Validation("Only PNG skin files are supported.".into()));
  }

  persist_skin_from_file(pool, resolved_user_id.as_str(), &source_path).await
}

pub async fn upload_skin_data(
  pool: &PgPool,
  user_id: Option<String>,
  identity: Option<String>,
  skin_name: Option<String>,
  skin_data_url: String,
) -> Result<String, AuthError> {
  let resolved_user_id = resolve_user_id(pool, user_id.as_deref(), identity.as_deref()).await?;
  let image_bytes = decode_skin_data_url(skin_data_url.as_str())?;

  persist_skin_from_data(
    pool,
    resolved_user_id.as_str(),
    skin_name.as_deref(),
    image_bytes.as_slice(),
  )
  .await
}

async fn resolve_user_id(
  pool: &PgPool,
  user_id: Option<&str>,
  identity: Option<&str>,
) -> Result<String, AuthError> {
  if let Some(value) = user_id.map(str::trim).filter(|value| !value.is_empty()) {
    return Ok(value.to_string());
  }

  if let Some(value) = identity.map(str::trim).filter(|value| !value.is_empty()) {
    let user = repository::find_user_by_identity(pool, value)
      .await
      .map_err(|error| AuthError::Internal(error.to_string()))?
      .ok_or_else(|| AuthError::Validation("User was not found for the provided identity.".into()))?;

    return Ok(user.id);
  }

  Err(AuthError::Validation("User id or identity is required.".into()))
}

async fn persist_skin_from_file(
  pool: &PgPool,
  user_id: &str,
  source_path: &Path,
) -> Result<String, AuthError> {
  let tbw_root = crate::game::find_tbw_root()
    .map_err(AuthError::Internal)?;
  let skins_dir = tbw_root.join("launcher_skins");
  fs::create_dir_all(&skins_dir)
    .map_err(|error| AuthError::Internal(format!("Failed to create skins directory {}: {error}", skins_dir.display())))?;

  let safe_user_id = sanitize_path_component(user_id);
  let target_file_name = format!("{safe_user_id}-{}.png", Uuid::new_v4());
  let target_path = skins_dir.join(target_file_name);

  fs::copy(source_path, &target_path).map_err(|error| {
    AuthError::Internal(format!(
      "Failed to copy skin file {} into {}: {error}",
      source_path.display(),
      target_path.display()
    ))
  })?;

  let stored_skin_url = target_path.to_string_lossy().to_string();
  repository::update_skin_url(pool, user_id, Some(stored_skin_url.as_str()))
    .await
    .map_err(|error| AuthError::Internal(error.to_string()))?;

  Ok(stored_skin_url)
}

async fn persist_skin_from_data(
  pool: &PgPool,
  user_id: &str,
  skin_name: Option<&str>,
  image_bytes: &[u8],
) -> Result<String, AuthError> {
  let tbw_root = crate::game::find_tbw_root()
    .map_err(AuthError::Internal)?;
  let skins_dir = tbw_root.join("launcher_skins");
  fs::create_dir_all(&skins_dir)
    .map_err(|error| AuthError::Internal(format!("Failed to create skins directory {}: {error}", skins_dir.display())))?;

  let safe_user_id = sanitize_path_component(user_id);
  let safe_name = skin_name
    .map(str::trim)
    .filter(|value| !value.is_empty())
    .map(sanitize_path_component)
    .unwrap_or_else(|| "skin".to_string());
  let target_file_name = format!("{safe_user_id}-{safe_name}-{}.png", Uuid::new_v4());
  let target_path = skins_dir.join(target_file_name);

  fs::write(&target_path, image_bytes).map_err(|error| {
    AuthError::Internal(format!(
      "Failed to write skin file {}: {error}",
      target_path.display()
    ))
  })?;

  let stored_skin_url = target_path.to_string_lossy().to_string();
  repository::update_skin_url(pool, user_id, Some(stored_skin_url.as_str()))
    .await
    .map_err(|error| AuthError::Internal(error.to_string()))?;

  Ok(stored_skin_url)
}

fn decode_skin_data_url(skin_data_url: &str) -> Result<Vec<u8>, AuthError> {
  let normalized = skin_data_url.trim();
  let (meta, payload) = normalized
    .split_once(',')
    .ok_or_else(|| AuthError::Validation("Invalid PNG data URL format.".into()))?;
  let normalized_meta = meta.to_ascii_lowercase();
  if !normalized_meta.starts_with("data:image/png;base64") {
    return Err(AuthError::Validation("Only PNG data URLs are supported.".into()));
  }

  BASE64_STANDARD
    .decode(payload)
    .map_err(|_| AuthError::Validation("Invalid base64 payload for the skin image.".into()))
}

fn hash_password(password: &str) -> Result<String, AuthError> {
  let salt = SaltString::generate(&mut OsRng);

  Argon2::default()
    .hash_password(password.as_bytes(), &salt)
    .map(|hash| hash.to_string())
    .map_err(|error| AuthError::Internal(error.to_string()))
}

fn verify_password(password: &str, stored_hash: &str) -> Result<(), AuthError> {
  let parsed_hash = PasswordHash::new(stored_hash)
    .map_err(|error| AuthError::Internal(error.to_string()))?;

  Argon2::default()
    .verify_password(password.as_bytes(), &parsed_hash)
    .map_err(|error| AuthError::Internal(error.to_string()))
}

fn sanitize_path_component(value: &str) -> String {
  let sanitized = value
    .chars()
    .map(|ch| {
      if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
        ch
      } else {
        '_'
      }
    })
    .collect::<String>();

  if sanitized.is_empty() {
    return "user".to_string();
  }

  sanitized
}
