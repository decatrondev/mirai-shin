//! Gestión de usuarios del panel. Solo SUPERADMIN.
//! Las claves nuevas son temporales: se muestran una sola vez y el panel obliga a cambiarlas.

use crate::{
    auth::{hash_password, normalize_email, temporary_password, ReadyUser, Role},
    error::{ApiError, ApiResult},
    state::AppState,
};
use axum::{
    extract::{Path, State},
    routing::{delete, get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list).post(create))
        .route("/{id}", delete(remove))
        .route("/{id}/reset", post(reset))
}

#[derive(Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct UserRow {
    id: Uuid,
    email: String,
    role: String,
    must_change_password: bool,
    created_at: chrono::DateTime<chrono::Utc>,
    last_login_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl UserRow {
    pub fn email(&self) -> &str {
        &self.email
    }
}

const COLUMNS: &str = "id, email, role, must_change_password, created_at, last_login_at";

async fn list(State(state): State<AppState>, user: ReadyUser) -> ApiResult<Json<Vec<UserRow>>> {
    user.require(Role::SuperAdmin)?;
    let rows = sqlx::query_as::<_, UserRow>(&format!(
        "SELECT {COLUMNS} FROM admin_users ORDER BY created_at"
    ))
    .fetch_all(&state.db)
    .await?;
    Ok(Json(rows))
}

#[derive(Deserialize)]
struct CreateBody {
    email: String,
    role: Role,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WithPassword {
    user: UserRow,
    temporary_password: String,
}

async fn create(
    State(state): State<AppState>,
    user: ReadyUser,
    Json(body): Json<CreateBody>,
) -> ApiResult<Json<WithPassword>> {
    user.require(Role::SuperAdmin)?;
    let (row, temporary_password) = insert_user(&state.db, &body.email, body.role).await?;
    Ok(Json(WithPassword {
        user: row,
        temporary_password,
    }))
}

/// Crea un usuario con clave temporal. Lo usa la API y el subcomando `create-user`.
pub async fn insert_user(db: &PgPool, email: &str, role: Role) -> ApiResult<(UserRow, String)> {
    let email = normalize_email(email).ok_or(ApiError::BadRequest("invalid_email"))?;
    let password = temporary_password();
    let res = sqlx::query_as::<_, UserRow>(&format!(
        "INSERT INTO admin_users (id, email, password_hash, role) VALUES ($1, $2, $3, $4) RETURNING {COLUMNS}"
    ))
    .bind(Uuid::new_v4())
    .bind(&email)
    .bind(hash_password(&password)?)
    .bind(role.as_str())
    .fetch_one(db)
    .await;
    match res {
        Ok(row) => Ok((row, password)),
        Err(sqlx::Error::Database(e)) if e.is_unique_violation() => {
            Err(ApiError::Conflict("email_taken"))
        }
        Err(e) => Err(e.into()),
    }
}

/// Nueva clave temporal: corta las sesiones abiertas de esa persona.
pub async fn reset_password(db: &PgPool, id: Uuid) -> ApiResult<(UserRow, String)> {
    let password = temporary_password();
    let row = sqlx::query_as::<_, UserRow>(&format!(
        "UPDATE admin_users
         SET password_hash = $1, must_change_password = TRUE, password_changed_at = now()
         WHERE id = $2 RETURNING {COLUMNS}"
    ))
    .bind(hash_password(&password)?)
    .bind(id)
    .fetch_one(db)
    .await?;
    Ok((row, password))
}

async fn reset(
    State(state): State<AppState>,
    user: ReadyUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<WithPassword>> {
    user.require(Role::SuperAdmin)?;
    if id == user.0.id {
        // La clave propia se cambia desde «Cuenta», sabiendo la actual.
        return Err(ApiError::BadRequest("cannot_reset_self"));
    }
    let (row, temporary_password) = reset_password(&state.db, id).await?;
    Ok(Json(WithPassword {
        user: row,
        temporary_password,
    }))
}

async fn remove(
    State(state): State<AppState>,
    user: ReadyUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    user.require(Role::SuperAdmin)?;
    if id == user.0.id {
        return Err(ApiError::BadRequest("cannot_delete_self"));
    }
    let n = sqlx::query("DELETE FROM admin_users WHERE id = $1")
        .bind(id)
        .execute(&state.db)
        .await?
        .rows_affected();
    if n == 0 {
        return Err(ApiError::NotFound);
    }
    Ok(Json(serde_json::json!({ "ok": true })))
}
