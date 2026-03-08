use crate::auth::models::{DbUser, RegisterPayload};
use sqlx::PgPool;

pub async fn find_user_by_identity(pool: &PgPool, identity: &str) -> Result<Option<DbUser>, sqlx::Error> {
  sqlx::query_as::<_, DbUser>(
    r#"
    SELECT id::text AS id, email, nickname, password_hash, skin_url, role
    FROM users
    WHERE email = $1 OR nickname = $1
    LIMIT 1
    "#,
  )
  .bind(identity)
  .fetch_optional(pool)
  .await
}

pub async fn find_user_by_nickname_case_insensitive(
  pool: &PgPool,
  nickname: &str,
) -> Result<Option<DbUser>, sqlx::Error> {
  sqlx::query_as::<_, DbUser>(
    r#"
    SELECT id::text AS id, email, nickname, password_hash, skin_url, role
    FROM users
    WHERE lower(nickname) = lower($1)
    LIMIT 1
    "#,
  )
  .bind(nickname)
  .fetch_optional(pool)
  .await
}

pub async fn create_user(pool: &PgPool, payload: &RegisterPayload, password_hash: &str) -> Result<DbUser, sqlx::Error> {
  sqlx::query_as::<_, DbUser>(
    r#"
    INSERT INTO users (email, nickname, password_hash, skin_url, role)
    VALUES ($1, $2, $3, NULL, 'user')
    RETURNING id::text AS id, email, nickname, password_hash, skin_url, role
    "#,
  )
  .bind(&payload.email)
  .bind(&payload.nickname)
  .bind(password_hash)
  .fetch_one(pool)
  .await
}

pub async fn update_nickname(pool: &PgPool, user_id: &str, nickname: &str) -> Result<(), sqlx::Error> {
  sqlx::query("UPDATE users SET nickname = $1 WHERE id::text = $2")
    .bind(nickname)
    .bind(user_id)
    .execute(pool)
    .await?;
  Ok(())
}

pub async fn update_password_hash(pool: &PgPool, user_id: &str, password_hash: &str) -> Result<(), sqlx::Error> {
  sqlx::query("UPDATE users SET password_hash = $1 WHERE id::text = $2")
    .bind(password_hash)
    .bind(user_id)
    .execute(pool)
    .await?;
  Ok(())
}

pub async fn update_skin_url(
  pool: &PgPool,
  user_id: &str,
  skin_url: Option<&str>,
) -> Result<(), sqlx::Error> {
  sqlx::query("UPDATE users SET skin_url = $1 WHERE id::text = $2")
    .bind(skin_url)
    .bind(user_id)
    .execute(pool)
    .await?;
  Ok(())
}
