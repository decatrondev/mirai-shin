use crate::{
    error::{ApiError, ApiResult},
    state::AppState,
};
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use axum::{
    extract::{FromRequestParts, State},
    http::{request::Parts, HeaderMap},
    routing::{get, post},
    Json, Router,
};
use chrono::{DateTime, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::time::Instant;
use uuid::Uuid;

pub const MIN_PASSWORD: usize = 12;
const SESSION_HOURS: i64 = 12;

/// Roles ordenados por privilegio: `role >= Role::Admin` se lee «al menos admin».
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Role {
    Viewer,
    Admin,
    #[serde(rename = "SUPERADMIN")]
    SuperAdmin,
}

impl Role {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "VIEWER" => Some(Self::Viewer),
            "ADMIN" => Some(Self::Admin),
            "SUPERADMIN" => Some(Self::SuperAdmin),
            _ => None,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Viewer => "VIEWER",
            Self::Admin => "ADMIN",
            Self::SuperAdmin => "SUPERADMIN",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserInfo {
    pub id: Uuid,
    pub email: String,
    pub role: Role,
    pub must_change_password: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: Uuid,
    /// Momento del último cambio de clave: si no coincide con la base, el token ya no vale.
    pwv: i64,
    exp: usize,
}

/// Sesión válida, aunque todavía tenga la clave temporal (solo para `/me` y `/password`).
pub struct AuthUser(pub UserInfo);

/// Sesión válida y con la clave temporal ya cambiada: la que usa todo lo demás.
pub struct ReadyUser(pub UserInfo);

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, ApiError> {
        let token = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .ok_or(ApiError::Unauthorized)?;
        let claims = decode::<Claims>(
            token,
            &DecodingKey::from_secret(state.jwt_secret.as_bytes()),
            &Validation::default(),
        )
        .map_err(|_| ApiError::Unauthorized)?
        .claims;

        // El rol y el estado se leen de la base en cada pedido: borrar a alguien
        // o cambiar su clave corta su sesión al instante.
        let row: Option<(String, String, bool, DateTime<Utc>)> = sqlx::query_as(
            "SELECT email, role, must_change_password, password_changed_at FROM admin_users WHERE id = $1",
        )
        .bind(claims.sub)
        .fetch_optional(&state.db)
        .await?;
        let (email, role, must_change_password, changed_at) = row.ok_or(ApiError::Unauthorized)?;
        if changed_at.timestamp_micros() != claims.pwv {
            return Err(ApiError::Unauthorized);
        }
        let role = Role::parse(&role).ok_or_else(|| anyhow::anyhow!("rol inválido en la base"))?;
        Ok(AuthUser(UserInfo {
            id: claims.sub,
            email,
            role,
            must_change_password,
        }))
    }
}

impl FromRequestParts<AppState> for ReadyUser {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, ApiError> {
        let AuthUser(user) = AuthUser::from_request_parts(parts, state).await?;
        if user.must_change_password {
            return Err(ApiError::PasswordChangeRequired);
        }
        Ok(ReadyUser(user))
    }
}

impl ReadyUser {
    pub fn require(&self, min: Role) -> ApiResult<()> {
        if self.0.role >= min {
            Ok(())
        } else {
            Err(ApiError::Forbidden)
        }
    }
}

pub fn hash_password(plain: &str) -> anyhow::Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    Ok(Argon2::default()
        .hash_password(plain.as_bytes(), &salt)
        .map_err(|e| anyhow::anyhow!(e))?
        .to_string())
}

fn verify_password(plain: &str, hash: &str) -> bool {
    PasswordHash::new(hash)
        .map(|h| {
            Argon2::default()
                .verify_password(plain.as_bytes(), &h)
                .is_ok()
        })
        .unwrap_or(false)
}

/// Clave temporal de 16 caracteres, sin los que se confunden al copiarlos (0/O, 1/l/I).
pub fn temporary_password() -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnpqrstuvwxyz23456789";
    let mut rng = rand::thread_rng();
    (0..16)
        .map(|_| ALPHABET[rng.gen_range(0..ALPHABET.len())] as char)
        .collect()
}

/// Correo normalizado y con forma válida; lo que escribe una persona se sanea.
pub fn normalize_email(raw: &str) -> Option<String> {
    let email = raw.trim().to_lowercase();
    let (local, domain) = email.split_once('@')?;
    let ok = !local.is_empty()
        && email.len() <= 254
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && email
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "@._+-".contains(c));
    ok.then_some(email)
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/login", post(login))
        .route("/me", get(me))
        .route("/password", post(change_password))
}

#[derive(Deserialize)]
struct LoginBody {
    email: String,
    password: String,
}

#[derive(Serialize)]
struct SessionResponse {
    token: String,
    user: UserInfo,
}

fn issue_token(state: &AppState, id: Uuid, changed_at: DateTime<Utc>) -> ApiResult<String> {
    let exp = (Utc::now() + chrono::Duration::hours(SESSION_HOURS)).timestamp() as usize;
    let claims = Claims {
        sub: id,
        pwv: changed_at.timestamp_micros(),
        exp,
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(state.jwt_secret.as_bytes()),
    )
    .map_err(|e| ApiError::Internal(anyhow::anyhow!(e)))
}

/// IP real del visitante: nginx la pasa en `X-Real-IP` (sale de `CF-Connecting-IP`).
pub fn client_ip(headers: &HeaderMap) -> String {
    headers
        .get("x-real-ip")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("desconocida")
        .to_string()
}

async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<LoginBody>,
) -> ApiResult<Json<SessionResponse>> {
    let email = normalize_email(&body.email).unwrap_or_default();
    let ip = format!("ip:{}", client_ip(&headers));
    let who = format!("email:{email}");
    let keys = [ip.as_str(), who.as_str()];
    if state.limiter.is_blocked(&keys, Instant::now()) {
        return Err(ApiError::TooManyAttempts);
    }

    let row: Option<(Uuid, String, String, bool, DateTime<Utc>)> = sqlx::query_as(
        "SELECT id, password_hash, role, must_change_password, password_changed_at
         FROM admin_users WHERE email = $1",
    )
    .bind(&email)
    .fetch_optional(&state.db)
    .await?;
    // Con un correo desconocido se verifica contra un hash de relleno: el tiempo de respuesta no lo delata.
    let found = row.is_some();
    let (id, hash, role, must_change_password, changed_at) = row.unwrap_or_else(|| {
        (
            Uuid::nil(),
            DUMMY_HASH.to_string(),
            String::new(),
            true,
            Utc::now(),
        )
    });
    if !verify_password(&body.password, &hash) || !found {
        state.limiter.record_failure(&keys, Instant::now());
        return Err(ApiError::Unauthorized);
    }
    state.limiter.clear(&keys);
    let role = Role::parse(&role).ok_or_else(|| anyhow::anyhow!("rol inválido en la base"))?;

    sqlx::query("UPDATE admin_users SET last_login_at = now() WHERE id = $1")
        .bind(id)
        .execute(&state.db)
        .await?;

    Ok(Json(SessionResponse {
        token: issue_token(&state, id, changed_at)?,
        user: UserInfo {
            id,
            email,
            role,
            must_change_password,
        },
    }))
}

// Hash argon2id de un texto al azar; solo iguala el tiempo con correos desconocidos.
const DUMMY_HASH: &str =
    "$argon2id$v=19$m=19456,t=2,p=1$c29tZXNhbHRzb21lc2FsdA$Q2hhc2tpRHVtbXlIYXNoVmFsdWVGb3JUaW1pbmc";

async fn me(user: AuthUser) -> Json<UserInfo> {
    Json(user.0)
}

#[derive(Deserialize)]
struct PasswordBody {
    current: String,
    new: String,
}

/// Cambia la clave (también la temporal). Devuelve un token nuevo: los anteriores dejan de valer.
async fn change_password(
    State(state): State<AppState>,
    user: AuthUser,
    Json(body): Json<PasswordBody>,
) -> ApiResult<Json<SessionResponse>> {
    if body.new.chars().count() < MIN_PASSWORD {
        return Err(ApiError::BadRequest("password_too_short"));
    }
    if body.new == body.current {
        return Err(ApiError::BadRequest("password_same"));
    }
    let hash: String = sqlx::query_scalar("SELECT password_hash FROM admin_users WHERE id = $1")
        .bind(user.0.id)
        .fetch_one(&state.db)
        .await?;
    if !verify_password(&body.current, &hash) {
        return Err(ApiError::BadRequest("wrong_password"));
    }
    let changed_at: DateTime<Utc> = sqlx::query_scalar(
        "UPDATE admin_users
         SET password_hash = $1, must_change_password = FALSE, password_changed_at = now()
         WHERE id = $2 RETURNING password_changed_at",
    )
    .bind(hash_password(&body.new)?)
    .bind(user.0.id)
    .fetch_one(&state.db)
    .await?;
    let mut info = user.0;
    info.must_change_password = false;
    Ok(Json(SessionResponse {
        token: issue_token(&state, info.id, changed_at)?,
        user: info,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roles_ordenados_por_privilegio() {
        assert!(Role::SuperAdmin > Role::Admin && Role::Admin > Role::Viewer);
        for r in [Role::Viewer, Role::Admin, Role::SuperAdmin] {
            assert_eq!(Role::parse(r.as_str()), Some(r));
        }
        assert_eq!(
            serde_json::to_string(&Role::SuperAdmin).unwrap(),
            "\"SUPERADMIN\""
        );
    }

    #[test]
    fn correos_se_normalizan_y_validan() {
        assert_eq!(
            normalize_email("  Ana@Decatron.NET ").as_deref(),
            Some("ana@decatron.net")
        );
        for bad in [
            "",
            "sin-arroba",
            "@dominio.net",
            "a@sinpunto",
            "a@.net",
            "a b@x.net",
            "a@x.net.",
        ] {
            assert_eq!(normalize_email(bad), None, "{bad}");
        }
    }

    #[test]
    fn clave_temporal_larga_y_sin_ambiguos() {
        let p = temporary_password();
        assert_eq!(p.len(), 16);
        assert!(!p.contains(['0', 'O', '1', 'l', 'I']));
        assert_ne!(p, temporary_password());
    }

    #[test]
    fn hash_y_verificacion() {
        let h = hash_password("una clave larga").unwrap();
        assert!(verify_password("una clave larga", &h));
        assert!(!verify_password("otra", &h));
        assert!(!verify_password("x", DUMMY_HASH));
    }
}
