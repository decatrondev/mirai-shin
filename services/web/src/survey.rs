//! Encuesta pública de la marca (`/marca` en la landing).
//!
//! Cualquiera con el enlace responde con su nombre. Se guarda el nombre, las
//! respuestas, el idioma y las fechas; la IP solo se usa en memoria para el
//! límite de envíos. Al crear, se devuelve un código que el navegador guarda
//! para cambiar las respuestas después (en la base va su SHA-256).
//! Los resultados solo los ve el panel (ADMIN o más).

use crate::{
    auth::{client_ip, ReadyUser, Role},
    error::{ApiError, ApiResult},
    state::AppState,
};
use axum::{
    extract::{Path, State},
    http::HeaderMap,
    routing::{delete, get, post, put},
    Json, Router,
};
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, time::Instant};
use uuid::Uuid;

/// Ronda actual de la encuesta. La ronda 2 (votación) tendrá su propio valor.
const SURVEY: &str = "marca-1";

pub const AXES: &[&str] = &[
    "serio_jugueton",
    "tecnico_artistico",
    "simple_detallado",
    "tranquilo_energico",
    "japones_evidente_sutil",
];
pub const WORDS: &[&str] = &[
    "liviano",
    "nocturno",
    "vivo",
    "tranquilo",
    "preciso",
    "calido",
    "futurista",
    "artesanal",
    "libre",
    "elegante",
    "jugueton",
    "tecnico",
];
pub const MAX_WORDS: usize = 3;
pub const SYMBOLS: &[&str] = &["kanji", "mecha", "monitor", "onda", "abstracto", "letras"];
pub const MAX_SYMBOLS: usize = 2;
pub const LOGO_TYPES: &[&str] = &["simbolo", "simbolo_nombre", "nombre", "sello"];
pub const STYLE_PAIRS: &[&str] = &[
    "geometrico_pincel",
    "fina_rellena",
    "plano_volumen",
    "redondo_anguloso",
    "simetrico_organico",
];
pub const STYLE_CHOICES: &[&str] = &["a", "b", "igual"];
pub const PALETTES: &[&str] = &["actual", "hora_azul", "sello", "amanecer", "mono"];
pub const SMALL: &[&str] = &["simple", "medio", "detallado"];
pub const LANGS: &[&str] = &["es", "en"];
pub const MAX_AVOID: usize = 500;
pub const MAX_COMMENT: usize = 1000;
pub const NAME_MIN: usize = 2;
pub const NAME_MAX: usize = 60;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/marca", post(create))
        .route("/marca/{id}", put(update))
        .route("/marca/responses", get(list))
        .route("/marca/responses/{id}", delete(remove))
}

/// Respuestas. Todo es opcional salvo el nombre (va aparte): se puede saltar una pregunta.
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct Answers {
    /// Eje → posición de -2 (primer extremo) a 2 (segundo).
    personality: BTreeMap<String, i8>,
    words: Vec<String>,
    symbols: Vec<String>,
    logo_type: Option<String>,
    /// Par de estilos → "a", "b" o "igual".
    styles: BTreeMap<String, String>,
    palette: Option<String>,
    small: Option<String>,
    avoid: String,
    comment: String,
}

impl Answers {
    /// Comprueba listas cerradas y largos. El error es el código que ve el front.
    pub fn validate(&self) -> Result<(), &'static str> {
        let known = |list: &[&str], v: &str| list.contains(&v);
        if self
            .personality
            .iter()
            .any(|(k, v)| !known(AXES, k) || !(-2..=2).contains(v))
        {
            return Err("invalid_answers");
        }
        if self.words.len() > MAX_WORDS || self.symbols.len() > MAX_SYMBOLS {
            return Err("invalid_answers");
        }
        if !distinct_and_known(&self.words, WORDS) || !distinct_and_known(&self.symbols, SYMBOLS) {
            return Err("invalid_answers");
        }
        let opt_ok =
            |list: &[&str], v: &Option<String>| v.as_deref().is_none_or(|v| known(list, v));
        if !opt_ok(LOGO_TYPES, &self.logo_type)
            || !opt_ok(PALETTES, &self.palette)
            || !opt_ok(SMALL, &self.small)
        {
            return Err("invalid_answers");
        }
        if self
            .styles
            .iter()
            .any(|(k, v)| !known(STYLE_PAIRS, k) || !known(STYLE_CHOICES, v))
        {
            return Err("invalid_answers");
        }
        if self.avoid.chars().count() > MAX_AVOID || self.comment.chars().count() > MAX_COMMENT {
            return Err("text_too_long");
        }
        Ok(())
    }
}

fn distinct_and_known(values: &[String], list: &[&str]) -> bool {
    values
        .iter()
        .enumerate()
        .all(|(i, v)| list.contains(&v.as_str()) && !values[..i].contains(v))
}

/// Nombre limpio: sin espacios de más ni caracteres de control, entre 2 y 60 caracteres.
pub fn clean_name(raw: &str) -> Option<String> {
    let name = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    let len = name.chars().count();
    if !(NAME_MIN..=NAME_MAX).contains(&len) || name.chars().any(char::is_control) {
        return None;
    }
    Some(name)
}

fn hash_token(token: &str) -> String {
    Sha256::digest(token.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn new_token() -> String {
    let bytes: [u8; 24] = rand::thread_rng().gen();
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[derive(Deserialize)]
struct Body {
    name: String,
    #[serde(default)]
    lang: String,
    answers: Answers,
    /// Campo trampa: la página lo deja vacío y oculto; un bot que rellena todo lo llena.
    #[serde(default)]
    website: String,
}

impl Body {
    fn checked(self) -> ApiResult<(String, String, Answers)> {
        let name = clean_name(&self.name).ok_or(ApiError::BadRequest("invalid_name"))?;
        self.answers.validate().map_err(ApiError::BadRequest)?;
        let lang = if LANGS.contains(&self.lang.as_str()) {
            self.lang
        } else {
            "es".into()
        };
        Ok((name, lang, self.answers))
    }
}

#[derive(Serialize)]
struct Created {
    id: Uuid,
    token: String,
}

/// Cuenta el envío en el límite por IP; falla si la IP ya llegó al tope.
fn count_send(state: &AppState, headers: &HeaderMap) -> ApiResult<()> {
    let key = format!("ip:{}", client_ip(headers));
    let now = Instant::now();
    if state.survey_limiter.is_blocked(&[&key], now) {
        return Err(ApiError::TooManyAttempts);
    }
    state.survey_limiter.record_failure(&[&key], now);
    Ok(())
}

async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Body>,
) -> ApiResult<Json<Created>> {
    count_send(&state, &headers)?;
    // Al bot se le responde como si todo fuera bien, sin guardar nada.
    if !body.website.trim().is_empty() {
        tracing::info!("encuesta: envío descartado por el campo trampa");
        return Ok(Json(Created {
            id: Uuid::new_v4(),
            token: new_token(),
        }));
    }
    let (name, lang, answers) = body.checked()?;
    let (id, token) = (Uuid::new_v4(), new_token());
    sqlx::query(
        "INSERT INTO survey_responses (id, survey, name, lang, answers, token_hash) VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(id)
    .bind(SURVEY)
    .bind(&name)
    .bind(&lang)
    .bind(sqlx::types::Json(&answers))
    .bind(hash_token(&token))
    .execute(&state.db)
    .await?;
    tracing::info!(%id, "encuesta: respuesta nueva");
    Ok(Json(Created { id, token }))
}

async fn update(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(body): Json<Body>,
) -> ApiResult<Json<serde_json::Value>> {
    count_send(&state, &headers)?;
    let token = headers
        .get("x-edit-token")
        .and_then(|v| v.to_str().ok())
        .ok_or(ApiError::Unauthorized)?;
    let (name, lang, answers) = body.checked()?;
    let done = sqlx::query(
        "UPDATE survey_responses SET name = $1, lang = $2, answers = $3, updated_at = now()
         WHERE id = $4 AND survey = $5 AND token_hash = $6",
    )
    .bind(&name)
    .bind(&lang)
    .bind(sqlx::types::Json(&answers))
    .bind(id)
    .bind(SURVEY)
    .bind(hash_token(token))
    .execute(&state.db)
    .await?;
    if done.rows_affected() == 0 {
        // Id inexistente o código equivocado: no se dice cuál de las dos.
        return Err(ApiError::NotFound);
    }
    Ok(Json(serde_json::json!({ "ok": true })))
}

#[derive(Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
struct Row {
    id: Uuid,
    name: String,
    lang: String,
    answers: serde_json::Value,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
}

async fn list(State(state): State<AppState>, user: ReadyUser) -> ApiResult<Json<Vec<Row>>> {
    user.require(Role::Admin)?;
    let rows = sqlx::query_as::<_, Row>(
        "SELECT id, name, lang, answers, created_at, updated_at FROM survey_responses
         WHERE survey = $1 ORDER BY created_at",
    )
    .bind(SURVEY)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(rows))
}

/// Para quitar envíos de prueba o spam.
async fn remove(
    State(state): State<AppState>,
    user: ReadyUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    user.require(Role::Admin)?;
    let done = sqlx::query("DELETE FROM survey_responses WHERE id = $1 AND survey = $2")
        .bind(id)
        .bind(SURVEY)
        .execute(&state.db)
        .await?;
    if done.rows_affected() == 0 {
        return Err(ApiError::NotFound);
    }
    tracing::info!(%id, by = %user.0.email, "encuesta: respuesta borrada");
    Ok(Json(serde_json::json!({ "ok": true })))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(json: &str) -> Answers {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn respuestas_vacias_son_validas() {
        assert!(Answers::default().validate().is_ok());
    }

    #[test]
    fn respuestas_completas_son_validas() {
        let a = parse(
            r#"{"personality":{"serio_jugueton":-2,"japones_evidente_sutil":2},"words":["liviano","vivo","libre"],
                "symbols":["kanji","onda"],"logoType":"sello","styles":{"fina_rellena":"a","plano_volumen":"igual"},
                "palette":"actual","small":"simple","avoid":"degradados","comment":"me gusta"}"#,
        );
        assert_eq!(a.validate(), Ok(()));
    }

    #[test]
    fn rechaza_valores_fuera_de_lista_o_repetidos() {
        assert!(parse(r#"{"words":["liviano","liviano"]}"#)
            .validate()
            .is_err());
        assert!(parse(r#"{"words":["a","b","c","d"]}"#).validate().is_err());
        assert!(parse(r#"{"words":["inventada"]}"#).validate().is_err());
        assert!(parse(r#"{"personality":{"serio_jugueton":3}}"#)
            .validate()
            .is_err());
        assert!(parse(r#"{"personality":{"otro":0}}"#).validate().is_err());
        assert!(parse(r#"{"logoType":"otro"}"#).validate().is_err());
        assert!(parse(r#"{"styles":{"fina_rellena":"c"}}"#)
            .validate()
            .is_err());
        assert!(parse(r#"{"symbols":["kanji","onda","mecha"]}"#)
            .validate()
            .is_err());
    }

    #[test]
    fn rechaza_campos_desconocidos() {
        assert!(serde_json::from_str::<Answers>(r#"{"extra":1}"#).is_err());
    }

    #[test]
    fn limita_el_largo_de_los_textos() {
        let a = Answers {
            comment: "x".repeat(MAX_COMMENT + 1),
            ..Default::default()
        };
        assert_eq!(a.validate(), Err("text_too_long"));
        let a = Answers {
            avoid: "ñ".repeat(MAX_AVOID),
            ..Default::default()
        };
        assert!(a.validate().is_ok());
    }

    #[test]
    fn limpia_el_nombre() {
        assert_eq!(clean_name("  Ana   María "), Some("Ana María".into()));
        assert_eq!(clean_name("A"), None);
        assert_eq!(clean_name(&"x".repeat(NAME_MAX + 1)), None);
        assert_eq!(clean_name("Ana\u{0007}"), None);
        assert_eq!(clean_name("Pixi"), Some("Pixi".into()));
    }

    #[test]
    fn el_codigo_se_guarda_como_hash() {
        let t = new_token();
        assert_eq!(t.len(), 48);
        assert_eq!(hash_token(&t).len(), 64);
        assert_ne!(hash_token(&t), t);
        assert_eq!(hash_token(&t), hash_token(&t));
    }
}
