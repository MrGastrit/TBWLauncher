use crate::auth::models::{DbAccountChangeStatus, DbUser, RegisterPayload};
use sqlx::PgPool;

pub async fn find_user_by_identity(
    pool: &PgPool,
    identity: &str,
) -> Result<Option<DbUser>, sqlx::Error> {
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

pub async fn find_user_by_id(pool: &PgPool, user_id: &str) -> Result<Option<DbUser>, sqlx::Error> {
    sqlx::query_as::<_, DbUser>(
        r#"
    SELECT id::text AS id, email, nickname, password_hash, skin_url, role
    FROM users
    WHERE id::text = $1
    LIMIT 1
    "#,
    )
    .bind(user_id)
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

pub async fn create_user(
    pool: &PgPool,
    payload: &RegisterPayload,
    password_hash: &str,
) -> Result<DbUser, sqlx::Error> {
    sqlx::query_as::<_, DbUser>(
    r#"
    INSERT INTO users (email, nickname, password_hash, skin_url, role, password_change_date, nickname_change_date)
    VALUES ($1, $2, $3, NULL, 'user', NOW(), NOW())
    RETURNING id::text AS id, email, nickname, password_hash, skin_url, role
    "#,
  )
  .bind(&payload.email)
  .bind(&payload.nickname)
  .bind(password_hash)
  .fetch_one(pool)
  .await
}

pub async fn update_nickname(
    pool: &PgPool,
    user_id: &str,
    nickname: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE users SET nickname = $1, nickname_change_date = NOW() WHERE id::text = $2")
        .bind(nickname)
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn update_password_hash(
    pool: &PgPool,
    user_id: &str,
    password_hash: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE users SET password_hash = $1, password_change_date = NOW() WHERE id::text = $2",
    )
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

pub async fn find_account_change_status(
    pool: &PgPool,
    user_id: &str,
) -> Result<Option<DbAccountChangeStatus>, sqlx::Error> {
    sqlx::query_as::<_, DbAccountChangeStatus>(
        r#"
    SELECT
      role,
      CASE
        WHEN nickname_change_date IS NULL THEN NULL
        ELSE to_char(nickname_change_date AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS"Z"')
      END AS nickname_change_date,
      CASE
        WHEN password_change_date IS NULL THEN NULL
        ELSE to_char(password_change_date AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS"Z"')
      END AS password_change_date,
      CASE
        WHEN lower(role) IN ('admin', 'tech') THEN 0
        WHEN lower(role) = 'vip' THEN 10
        ELSE 30
      END::int AS nickname_cooldown_days,
      CASE
        WHEN lower(role) IN ('admin', 'tech') THEN 0
        ELSE 30
      END::int AS password_cooldown_days,
      GREATEST(
        EXTRACT(EPOCH FROM (
          COALESCE(
            nickname_change_date + (
              CASE
                WHEN lower(role) IN ('admin', 'tech') THEN INTERVAL '0 days'
                WHEN lower(role) = 'vip' THEN INTERVAL '10 days'
                ELSE INTERVAL '30 days'
              END
            ),
            NOW()
          ) - NOW()
        )),
        0
      )::bigint AS nickname_remaining_seconds,
      GREATEST(
        EXTRACT(EPOCH FROM (
          COALESCE(
            password_change_date + (
              CASE
                WHEN lower(role) IN ('admin', 'tech') THEN INTERVAL '0 days'
                ELSE INTERVAL '30 days'
              END
            ),
            NOW()
          ) - NOW()
        )),
        0
      )::bigint AS password_remaining_seconds,
      COALESCE(
        nickname_change_date <= NOW() - (
          CASE
            WHEN lower(role) IN ('admin', 'tech') THEN INTERVAL '0 days'
            WHEN lower(role) = 'vip' THEN INTERVAL '10 days'
            ELSE INTERVAL '30 days'
          END
        ),
        TRUE
      ) AS can_change_nickname,
      COALESCE(
        password_change_date <= NOW() - (
          CASE
            WHEN lower(role) IN ('admin', 'tech') THEN INTERVAL '0 days'
            ELSE INTERVAL '30 days'
          END
        ),
        TRUE
      ) AS can_change_password
    FROM users
    WHERE id::text = $1
    LIMIT 1
    "#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
}
