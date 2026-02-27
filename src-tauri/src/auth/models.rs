use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthUser {
  pub id: String,
  pub nickname: String,
  pub email: String,
  pub skin_url: Option<String>,
  pub role: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthTokens {
  pub access_token: String,
  pub refresh_token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthResult {
  pub user: AuthUser,
  pub tokens: AuthTokens,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterPayload {
  pub email: String,
  pub nickname: String,
  pub password: String,
  pub repeat_password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginPayload {
  pub identity: String,
  pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAccountPayload {
  pub nickname: Option<String>,
  pub skin_path: Option<String>
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangePasswordPayload {
  pub current_password: String,
  pub next_password: String,
}

#[derive(Debug, Clone, FromRow)]
pub struct DbUser {
  pub id: String,
  pub email: String,
  pub nickname: String,
  pub password_hash: String,
  pub skin_url: Option<String>,
  pub role: String,
}
