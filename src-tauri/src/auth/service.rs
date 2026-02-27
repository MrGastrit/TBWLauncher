use crate::auth::errors::AuthError;
use crate::auth::models::{
  AuthResult, AuthTokens, AuthUser, ChangePasswordPayload, LoginPayload, RegisterPayload, UpdateAccountPayload,
};
use crate::auth::repository;
use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use sqlx::PgPool;
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

pub async fn upload_skin(_pool: &PgPool, _file_path: String) -> Result<String, AuthError> {
  Err(AuthError::NotImplemented("upload_skin service is not implemented yet"))
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
