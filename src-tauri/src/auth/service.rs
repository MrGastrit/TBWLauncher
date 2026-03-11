use crate::auth::errors::AuthError;
use crate::auth::models::{
    AccountChangeStatus, AuthResult, AuthTokens, AuthUser, ChangePasswordPayload,
    DbAccountChangeStatus, LoginPayload, RegisterPayload, UpdateAccountPayload,
};
use crate::auth::repository;
use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine as _;
use reqwest::blocking::Client as BlockingHttpClient;
use reqwest::header::CONTENT_TYPE;
use sqlx::PgPool;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
use uuid::Uuid;

const MIN_NICKNAME_LEN: usize = 3;
const MAX_NICKNAME_LEN: usize = 24;
const CHANGE_COOLDOWN_DAYS: i64 = 30;
const SKIN_CDN_BASE_URL_ENV: &str = "SKIN_CDN_BASE_URL";
const SKIN_CDN_UPLOAD_URL_ENV: &str = "SKIN_CDN_UPLOAD_URL";
const SKIN_CDN_BASIC_USER_ENV: &str = "SKIN_CDN_BASIC_USER";
const SKIN_CDN_BASIC_PASSWORD_ENV: &str = "SKIN_CDN_BASIC_PASSWORD";
const SKIN_CDN_AUTH_HEADER_NAME_ENV: &str = "SKIN_CDN_AUTH_HEADER_NAME";
const SKIN_CDN_AUTH_HEADER_VALUE_ENV: &str = "SKIN_CDN_AUTH_HEADER_VALUE";
const SKIN_CDN_TIMEOUT_SECONDS_ENV: &str = "SKIN_CDN_TIMEOUT_SECONDS";
const SKIN_CDN_DEFAULT_TIMEOUT_SECONDS: u64 = 20;

#[derive(Debug, Clone)]
struct SkinCdnConfig {
    public_base_url: String,
    upload_base_url: String,
    basic_user: Option<String>,
    basic_password: Option<String>,
    auth_header_name: Option<String>,
    auth_header_value: Option<String>,
    timeout_seconds: u64,
}

pub async fn register(pool: &PgPool, payload: RegisterPayload) -> Result<AuthResult, AuthError> {
    let normalized_email = payload.email.trim();
    if normalized_email.is_empty() {
        return Err(AuthError::Validation("Email обязателен.".into()));
    }

    let normalized_nickname = payload.nickname.trim();
    if normalized_nickname.len() < MIN_NICKNAME_LEN {
        return Err(AuthError::Validation(
            "Ник должен содержать минимум 3 символа.".into(),
        ));
    }
    if normalized_nickname.len() > MAX_NICKNAME_LEN {
        return Err(AuthError::Validation(
            "Ник не должен превышать 24 символа.".into(),
        ));
    }
    if !is_valid_registration_nickname(normalized_nickname) {
        return Err(AuthError::Validation(
            "Ник может содержать только английские буквы, цифры и нижнее подчеркивание (_).".into(),
        ));
    }

    if payload.password != payload.repeat_password {
        return Err(AuthError::Validation("Пароли не совпадают.".into()));
    }

    let existing_email = repository::find_user_by_identity(pool, normalized_email)
        .await
        .map_err(|error| AuthError::Internal(error.to_string()))?;

    if existing_email.is_some() {
        return Err(AuthError::Validation(
            "Пользователь с таким email уже существует.".into(),
        ));
    }

    let existing_nickname = repository::find_user_by_identity(pool, normalized_nickname)
        .await
        .map_err(|error| AuthError::Internal(error.to_string()))?;

    if existing_nickname.is_some() {
        return Err(AuthError::Validation("Ник уже занят.".into()));
    }

    let normalized_payload = RegisterPayload {
        email: normalized_email.to_string(),
        nickname: normalized_nickname.to_string(),
        password: payload.password,
        repeat_password: payload.repeat_password,
    };

    let password_hash = hash_password(&normalized_payload.password)?;

    let user = repository::create_user(pool, &normalized_payload, &password_hash)
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

pub async fn update_account(
    pool: &PgPool,
    user_id: Option<String>,
    identity: Option<String>,
    payload: UpdateAccountPayload,
) -> Result<(), AuthError> {
    let resolved_user_id = resolve_user_id(pool, user_id.as_deref(), identity.as_deref()).await?;

    let current_user = repository::find_user_by_id(pool, resolved_user_id.as_str())
        .await
        .map_err(|error| AuthError::Internal(error.to_string()))?
        .ok_or_else(|| AuthError::Validation("Пользователь не найден.".into()))?;

    if let Some(raw_nickname) = payload.nickname.as_deref() {
        let normalized_nickname = raw_nickname.trim();
        if normalized_nickname.is_empty() {
            return Err(AuthError::Validation("Ник не может быть пустым.".into()));
        }

        if normalized_nickname != current_user.nickname {
            if normalized_nickname.len() < MIN_NICKNAME_LEN {
                return Err(AuthError::Validation(
                    "Ник должен содержать минимум 3 символа.".into(),
                ));
            }
            if normalized_nickname.len() > MAX_NICKNAME_LEN {
                return Err(AuthError::Validation(
                    "Ник не должен превышать 24 символа.".into(),
                ));
            }
            if !is_valid_registration_nickname(normalized_nickname) {
                return Err(AuthError::Validation(
          "Ник может содержать только английские буквы, цифры и нижнее подчеркивание (_).".into(),
        ));
            }

            let existing_nickname =
                repository::find_user_by_nickname_case_insensitive(pool, normalized_nickname)
                    .await
                    .map_err(|error| AuthError::Internal(error.to_string()))?;
            if let Some(user_with_nickname) = existing_nickname {
                if user_with_nickname.id != current_user.id {
                    return Err(AuthError::Validation("Ник уже занят.".into()));
                }
            }

            let account_change_status =
                repository::find_account_change_status(pool, resolved_user_id.as_str())
                    .await
                    .map_err(|error| AuthError::Internal(error.to_string()))?
                    .ok_or_else(|| AuthError::Validation("Пользователь не найден.".into()))?;
            if !account_change_status.can_change_nickname {
                let nickname_cooldown_days =
                    i64::from(account_change_status.nickname_cooldown_days);
                let effective_days = if nickname_cooldown_days > 0 {
                    nickname_cooldown_days
                } else {
                    CHANGE_COOLDOWN_DAYS
                };

                return Err(AuthError::Validation(format!(
                    "Ник можно менять не чаще одного раза в {effective_days} дней."
                )));
            }

            repository::update_nickname(pool, resolved_user_id.as_str(), normalized_nickname)
                .await
                .map_err(|error| AuthError::Internal(error.to_string()))?;
        }
    }

    if let Some(raw_skin_path) = payload.skin_path.as_deref() {
        let normalized_skin_path = raw_skin_path.trim();
        if !normalized_skin_path.is_empty() {
            repository::update_skin_url(
                pool,
                resolved_user_id.as_str(),
                Some(normalized_skin_path),
            )
            .await
            .map_err(|error| AuthError::Internal(error.to_string()))?;
        }
    }

    Ok(())
}

pub async fn change_password(
    pool: &PgPool,
    user_id: Option<String>,
    identity: Option<String>,
    payload: ChangePasswordPayload,
) -> Result<(), AuthError> {
    let resolved_user_id = resolve_user_id(pool, user_id.as_deref(), identity.as_deref()).await?;

    let current_password = payload.current_password.as_str();
    if current_password.is_empty() {
        return Err(AuthError::Validation("Введите текущий пароль.".into()));
    }

    let next_password = payload.next_password.as_str();
    if next_password.is_empty() {
        return Err(AuthError::Validation("Введите новый пароль.".into()));
    }

    let current_user = repository::find_user_by_id(pool, resolved_user_id.as_str())
        .await
        .map_err(|error| AuthError::Internal(error.to_string()))?
        .ok_or_else(|| AuthError::Validation("Пользователь не найден.".into()))?;

    verify_password(current_password, &current_user.password_hash)
        .map_err(|_| AuthError::Validation("Текущий пароль введён неверно.".into()))?;

    let account_change_status =
        repository::find_account_change_status(pool, resolved_user_id.as_str())
            .await
            .map_err(|error| AuthError::Internal(error.to_string()))?
            .ok_or_else(|| AuthError::Validation("Пользователь не найден.".into()))?;
    if !account_change_status.can_change_password {
        let password_cooldown_days = i64::from(account_change_status.password_cooldown_days);
        let effective_days = if password_cooldown_days > 0 {
            password_cooldown_days
        } else {
            CHANGE_COOLDOWN_DAYS
        };

        return Err(AuthError::Validation(format!(
            "Пароль можно менять не чаще одного раза в {effective_days} дней."
        )));
    }

    let next_password_hash = hash_password(next_password)?;

    repository::update_password_hash(pool, resolved_user_id.as_str(), next_password_hash.as_str())
        .await
        .map_err(|error| AuthError::Internal(error.to_string()))
}

pub async fn get_account_change_status(
    pool: &PgPool,
    user_id: Option<String>,
    identity: Option<String>,
) -> Result<AccountChangeStatus, AuthError> {
    let resolved_user_id = resolve_user_id(pool, user_id.as_deref(), identity.as_deref()).await?;

    let status = repository::find_account_change_status(pool, resolved_user_id.as_str())
        .await
        .map_err(|error| AuthError::Internal(error.to_string()))?
        .ok_or_else(|| AuthError::Validation("Пользователь не найден.".into()))?;

    Ok(map_account_change_status(status))
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
        return Err(AuthError::Validation(
            "Only PNG skin files are supported.".into(),
        ));
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
            .ok_or_else(|| {
                AuthError::Validation("User was not found for the provided identity.".into())
            })?;

        return Ok(user.id);
    }

    Err(AuthError::Validation(
        "User id or identity is required.".into(),
    ))
}

async fn persist_skin_from_file(
    pool: &PgPool,
    user_id: &str,
    source_path: &Path,
) -> Result<String, AuthError> {
    let image_bytes = fs::read(source_path).map_err(|error| {
        AuthError::Internal(format!(
            "Failed to read skin file {}: {error}",
            source_path.display()
        ))
    })?;

    persist_skin_bytes(pool, user_id, None, image_bytes.as_slice()).await
}

async fn persist_skin_from_data(
    pool: &PgPool,
    user_id: &str,
    skin_name: Option<&str>,
    image_bytes: &[u8],
) -> Result<String, AuthError> {
    persist_skin_bytes(pool, user_id, skin_name, image_bytes).await
}

async fn persist_skin_bytes(
    pool: &PgPool,
    user_id: &str,
    skin_name: Option<&str>,
    image_bytes: &[u8],
) -> Result<String, AuthError> {
    let skin_file_name = resolve_skin_file_name(pool, user_id, skin_name).await?;
    if let Some(cdn_config) = load_skin_cdn_config() {
        let stored_skin_url =
            upload_skin_bytes_to_cdn(&cdn_config, skin_file_name.as_str(), image_bytes)?;
        repository::update_skin_url(pool, user_id, Some(stored_skin_url.as_str()))
            .await
            .map_err(|error| AuthError::Internal(error.to_string()))?;
        return Ok(stored_skin_url);
    }

    let tbw_root = crate::game::find_tbw_root().map_err(AuthError::Internal)?;
    let skins_dir = tbw_root.join("launcher_skins");
    fs::create_dir_all(&skins_dir).map_err(|error| {
        AuthError::Internal(format!(
            "Failed to create skins directory {}: {error}",
            skins_dir.display()
        ))
    })?;

    let safe_user_id = sanitize_path_component(user_id);
    let target_file_name = format!("{safe_user_id}-{}.png", Uuid::new_v4());
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

async fn resolve_skin_file_name(
    pool: &PgPool,
    user_id: &str,
    skin_name: Option<&str>,
) -> Result<String, AuthError> {
    let user = repository::find_user_by_id(pool, user_id)
        .await
        .map_err(|error| AuthError::Internal(error.to_string()))?
        .ok_or_else(|| AuthError::Validation("Пользователь не найден.".into()))?;

    let fallback_name = skin_name
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("skin");
    let raw_name = if user.nickname.trim().is_empty() {
        fallback_name
    } else {
        user.nickname.as_str()
    };
    let normalized_name = sanitize_path_component(raw_name).to_ascii_lowercase();
    let resolved_name = if normalized_name.is_empty() {
        "skin".to_string()
    } else {
        normalized_name
    };

    Ok(format!("{resolved_name}.png"))
}

fn load_skin_cdn_config() -> Option<SkinCdnConfig> {
    let public_base_url = crate::env_var_with_embedded_fallback(SKIN_CDN_BASE_URL_ENV)?;
    let public_base_url = normalize_url_base(public_base_url.as_str())?;
    let upload_base_url = crate::env_var_with_embedded_fallback(SKIN_CDN_UPLOAD_URL_ENV)
        .and_then(|value| normalize_url_base(value.as_str()))
        .unwrap_or_else(|| public_base_url.clone());
    let basic_user = crate::env_var_with_embedded_fallback(SKIN_CDN_BASIC_USER_ENV)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let basic_password = crate::env_var_with_embedded_fallback(SKIN_CDN_BASIC_PASSWORD_ENV)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let auth_header_name = crate::env_var_with_embedded_fallback(SKIN_CDN_AUTH_HEADER_NAME_ENV)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let auth_header_value = crate::env_var_with_embedded_fallback(SKIN_CDN_AUTH_HEADER_VALUE_ENV)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let timeout_seconds = crate::env_var_with_embedded_fallback(SKIN_CDN_TIMEOUT_SECONDS_ENV)
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(SKIN_CDN_DEFAULT_TIMEOUT_SECONDS);

    Some(SkinCdnConfig {
        public_base_url,
        upload_base_url,
        basic_user,
        basic_password,
        auth_header_name,
        auth_header_value,
        timeout_seconds,
    })
}

fn upload_skin_bytes_to_cdn(
    config: &SkinCdnConfig,
    file_name: &str,
    image_bytes: &[u8],
) -> Result<String, AuthError> {
    let public_url = join_url(config.public_base_url.as_str(), file_name);
    let upload_url = join_url(config.upload_base_url.as_str(), file_name);
    let http_client = BlockingHttpClient::builder()
        .timeout(Duration::from_secs(config.timeout_seconds))
        .build()
        .map_err(|error| {
            AuthError::Internal(format!(
                "Failed to create HTTP client for skin CDN: {error}"
            ))
        })?;

    let mut request = http_client
        .put(upload_url.as_str())
        .header(CONTENT_TYPE, "image/png")
        .body(image_bytes.to_vec());

    if let Some(user) = config.basic_user.as_ref() {
        request = request.basic_auth(user.as_str(), config.basic_password.clone());
    }

    if let (Some(header_name), Some(header_value)) = (
        config.auth_header_name.as_deref(),
        config.auth_header_value.as_deref(),
    ) {
        request = request.header(header_name, header_value);
    }

    let response = request.send().map_err(|error| {
        AuthError::Internal(format!(
            "Failed to upload skin to CDN {upload_url}: {error}"
        ))
    })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(AuthError::Internal(format!(
            "Failed to upload skin to CDN {upload_url}: HTTP {} {}",
            status.as_u16(),
            truncate_error_text(body.as_str(), 200)
        )));
    }

    Ok(public_url)
}

fn normalize_url_base(value: &str) -> Option<String> {
    let normalized = value.trim().trim_end_matches('/').to_string();
    if normalized.is_empty() {
        return None;
    }

    let lowercase = normalized.to_ascii_lowercase();
    if !lowercase.starts_with("http://") && !lowercase.starts_with("https://") {
        return None;
    }

    Some(normalized)
}

fn join_url(base: &str, file_name: &str) -> String {
    format!(
        "{}/{}",
        base.trim_end_matches('/'),
        file_name.trim_start_matches('/')
    )
}

fn truncate_error_text(value: &str, max_chars: usize) -> String {
    if value.len() <= max_chars {
        return value.to_string();
    }

    let end_index = value
        .char_indices()
        .nth(max_chars)
        .map(|entry| entry.0)
        .unwrap_or(value.len());
    format!("{}...", &value[..end_index])
}

fn decode_skin_data_url(skin_data_url: &str) -> Result<Vec<u8>, AuthError> {
    let normalized = skin_data_url.trim();
    let (meta, payload) = normalized
        .split_once(',')
        .ok_or_else(|| AuthError::Validation("Invalid PNG data URL format.".into()))?;
    let normalized_meta = meta.to_ascii_lowercase();
    if !normalized_meta.starts_with("data:image/png;base64") {
        return Err(AuthError::Validation(
            "Only PNG data URLs are supported.".into(),
        ));
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
    let parsed_hash =
        PasswordHash::new(stored_hash).map_err(|error| AuthError::Internal(error.to_string()))?;

    Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .map_err(|error| AuthError::Internal(error.to_string()))
}

fn is_valid_registration_nickname(value: &str) -> bool {
    value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
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

fn map_account_change_status(value: DbAccountChangeStatus) -> AccountChangeStatus {
    AccountChangeStatus {
        role: value.role,
        nickname_change_date: value.nickname_change_date,
        password_change_date: value.password_change_date,
        nickname_cooldown_days: value.nickname_cooldown_days,
        password_cooldown_days: value.password_cooldown_days,
        nickname_remaining_seconds: value.nickname_remaining_seconds,
        password_remaining_seconds: value.password_remaining_seconds,
        can_change_nickname: value.can_change_nickname,
        can_change_password: value.can_change_password,
    }
}
