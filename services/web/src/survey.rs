//! Encuestas públicas de la landing: la de la marca (ronda 1, `/marca/ronda-1`),
//! la votación del logo (ronda 2, `/marca`) y la de la interfaz de la app (`/interfaz`). Cada una tiene sus preguntas y su
//! validación; comparten la tabla `survey_responses` (columna `survey`).
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

/// Encuesta según la dirección (`/api/survey/{kind}`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    Marca,
    /// Ronda 2 de la marca: votación entre las propuestas de logo.
    Logo,
    Interfaz,
}

impl Kind {
    fn parse(path: &str) -> ApiResult<Self> {
        match path {
            "marca" => Ok(Self::Marca),
            "logo" => Ok(Self::Logo),
            "interfaz" => Ok(Self::Interfaz),
            _ => Err(ApiError::NotFound),
        }
    }

    /// Valor de la columna `survey`. Una ronda nueva tendrá su propio valor.
    fn id(self) -> &'static str {
        match self {
            Self::Marca => "marca-1",
            Self::Logo => "marca-2",
            Self::Interfaz => "interfaz-1",
        }
    }

    /// Lee y valida las respuestas de esta encuesta; devuelve el JSON que se guarda.
    pub fn check(self, raw: serde_json::Value) -> Result<serde_json::Value, &'static str> {
        fn typed<T: serde::de::DeserializeOwned + Serialize>(
            raw: serde_json::Value,
            validate: fn(&T) -> Result<(), &'static str>,
        ) -> Result<serde_json::Value, &'static str> {
            let answers: T = serde_json::from_value(raw).map_err(|_| "invalid_answers")?;
            validate(&answers)?;
            serde_json::to_value(answers).map_err(|_| "invalid_answers")
        }
        match self {
            Self::Marca => typed(raw, Answers::validate),
            Self::Logo => typed(raw, LogoAnswers::validate),
            Self::Interfaz => typed(raw, UiAnswers::validate),
        }
    }
}

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

/// Versiones de la interfaz que se comparan (`/direcciones/app/`).
pub const VERSIONS: &[&str] = &["a", "b", "c", "d", "e"];
pub const MAX_CHANGE: usize = 500;
pub const MAX_MIX: usize = 300;

/// Propuestas de logo de la ronda 2 (`/direcciones/logo/`) y las paletas en que se muestran.
pub const PROPOSALS: &[&str] = &["sello", "enso", "mecha", "kitsune"];
pub const LOGO_PALETTES: &[&str] = &["actual", "hora_azul", "sello"];

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/{kind}", post(create))
        .route("/{kind}/{id}", put(update))
        .route("/{kind}/responses", get(list))
        .route("/{kind}/responses/{id}", delete(remove))
}

/// Respuestas de la marca. Todo es opcional salvo el nombre (va aparte): se puede saltar una pregunta.
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

/// Respuestas de la interfaz. Solo la elegida es obligatoria.
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct UiAnswers {
    /// La versión que elegiría.
    pick: Option<String>,
    /// Segunda opción, distinta de la elegida.
    second: Option<String>,
    /// Qué cambiaría de la elegida.
    change: String,
    /// Mezcla: de qué otra versión tomaría algo, y qué parte.
    mix_from: Option<String>,
    mix_part: String,
    /// Notas opcionales por versión, de 1 a 5.
    ratings: BTreeMap<String, u8>,
}

impl UiAnswers {
    pub fn validate(&self) -> Result<(), &'static str> {
        let version = |v: &Option<String>| v.as_deref().is_none_or(|v| VERSIONS.contains(&v));
        let Some(pick) = self.pick.as_deref() else {
            return Err("invalid_answers");
        };
        if !VERSIONS.contains(&pick) || !version(&self.second) || !version(&self.mix_from) {
            return Err("invalid_answers");
        }
        if self.second.as_deref() == Some(pick) || self.mix_from.as_deref() == Some(pick) {
            return Err("invalid_answers");
        }
        if self
            .ratings
            .iter()
            .any(|(k, v)| !VERSIONS.contains(&k.as_str()) || !(1..=5).contains(v))
        {
            return Err("invalid_answers");
        }
        if self.change.chars().count() > MAX_CHANGE || self.mix_part.chars().count() > MAX_MIX {
            return Err("text_too_long");
        }
        Ok(())
    }
}

/// Votación del logo. Obligatorias: la propuesta elegida y la paleta.
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct LogoAnswers {
    pick: Option<String>,
    /// Segunda opción, distinta de la elegida.
    second: Option<String>,
    palette: Option<String>,
    /// Notas opcionales por propuesta, de 1 a 5.
    ratings: BTreeMap<String, u8>,
    comment: String,
}

impl LogoAnswers {
    pub fn validate(&self) -> Result<(), &'static str> {
        let (Some(pick), Some(palette)) = (self.pick.as_deref(), self.palette.as_deref()) else {
            return Err("invalid_answers");
        };
        if !PROPOSALS.contains(&pick) || !LOGO_PALETTES.contains(&palette) {
            return Err("invalid_answers");
        }
        if self
            .second
            .as_deref()
            .is_some_and(|v| v == pick || !PROPOSALS.contains(&v))
        {
            return Err("invalid_answers");
        }
        if self
            .ratings
            .iter()
            .any(|(k, v)| !PROPOSALS.contains(&k.as_str()) || !(1..=5).contains(v))
        {
            return Err("invalid_answers");
        }
        if self.comment.chars().count() > MAX_COMMENT {
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
    answers: serde_json::Value,
    /// Campo trampa: la página lo deja vacío y oculto; un bot que rellena todo lo llena.
    #[serde(default)]
    website: String,
}

impl Body {
    fn checked(self, kind: Kind) -> ApiResult<(String, String, serde_json::Value)> {
        let name = clean_name(&self.name).ok_or(ApiError::BadRequest("invalid_name"))?;
        let answers = kind.check(self.answers).map_err(ApiError::BadRequest)?;
        let lang = if LANGS.contains(&self.lang.as_str()) {
            self.lang
        } else {
            "es".into()
        };
        Ok((name, lang, answers))
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
    Path(kind): Path<String>,
    headers: HeaderMap,
    Json(body): Json<Body>,
) -> ApiResult<Json<Created>> {
    let kind = Kind::parse(&kind)?;
    count_send(&state, &headers)?;
    // Al bot se le responde como si todo fuera bien, sin guardar nada.
    if !body.website.trim().is_empty() {
        tracing::info!("encuesta: envío descartado por el campo trampa");
        return Ok(Json(Created {
            id: Uuid::new_v4(),
            token: new_token(),
        }));
    }
    let (name, lang, answers) = body.checked(kind)?;
    let (id, token) = (Uuid::new_v4(), new_token());
    sqlx::query(
        "INSERT INTO survey_responses (id, survey, name, lang, answers, token_hash) VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(id)
    .bind(kind.id())
    .bind(&name)
    .bind(&lang)
    .bind(sqlx::types::Json(&answers))
    .bind(hash_token(&token))
    .execute(&state.db)
    .await?;
    tracing::info!(%id, survey = kind.id(), "encuesta: respuesta nueva");
    Ok(Json(Created { id, token }))
}

async fn update(
    State(state): State<AppState>,
    Path((kind, id)): Path<(String, Uuid)>,
    headers: HeaderMap,
    Json(body): Json<Body>,
) -> ApiResult<Json<serde_json::Value>> {
    let kind = Kind::parse(&kind)?;
    count_send(&state, &headers)?;
    let token = headers
        .get("x-edit-token")
        .and_then(|v| v.to_str().ok())
        .ok_or(ApiError::Unauthorized)?;
    let (name, lang, answers) = body.checked(kind)?;
    let done = sqlx::query(
        "UPDATE survey_responses SET name = $1, lang = $2, answers = $3, updated_at = now()
         WHERE id = $4 AND survey = $5 AND token_hash = $6",
    )
    .bind(&name)
    .bind(&lang)
    .bind(sqlx::types::Json(&answers))
    .bind(id)
    .bind(kind.id())
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

async fn list(
    State(state): State<AppState>,
    Path(kind): Path<String>,
    user: ReadyUser,
) -> ApiResult<Json<Vec<Row>>> {
    user.require(Role::Admin)?;
    let kind = Kind::parse(&kind)?;
    let rows = sqlx::query_as::<_, Row>(
        "SELECT id, name, lang, answers, created_at, updated_at FROM survey_responses
         WHERE survey = $1 ORDER BY created_at",
    )
    .bind(kind.id())
    .fetch_all(&state.db)
    .await?;
    Ok(Json(rows))
}

/// Para quitar envíos de prueba o spam.
async fn remove(
    State(state): State<AppState>,
    user: ReadyUser,
    Path((kind, id)): Path<(String, Uuid)>,
) -> ApiResult<Json<serde_json::Value>> {
    user.require(Role::Admin)?;
    let kind = Kind::parse(&kind)?;
    let done = sqlx::query("DELETE FROM survey_responses WHERE id = $1 AND survey = $2")
        .bind(id)
        .bind(kind.id())
        .execute(&state.db)
        .await?;
    if done.rows_affected() == 0 {
        return Err(ApiError::NotFound);
    }
    tracing::info!(%id, survey = kind.id(), by = %user.0.email, "encuesta: respuesta borrada");
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

    fn ui(json: &str) -> Result<serde_json::Value, &'static str> {
        Kind::Interfaz.check(serde_json::from_str(json).unwrap())
    }

    #[test]
    fn interfaz_pide_la_elegida() {
        assert!(ui(r#"{}"#).is_err());
        assert!(ui(r#"{"pick":"c"}"#).is_ok());
        assert!(ui(r#"{"pick":"z"}"#).is_err());
    }

    #[test]
    fn interfaz_completa_es_valida() {
        let v = ui(
            r#"{"pick":"c","second":"e","change":"menos gris","mixFrom":"b","mixPart":"arrastrar",
                       "ratings":{"a":3,"c":5,"e":4}}"#,
        )
        .unwrap();
        assert_eq!(v["mixFrom"], "b");
        assert_eq!(v["ratings"]["c"], 5);
    }

    #[test]
    fn interfaz_rechaza_repetidas_y_notas_fuera_de_rango() {
        assert!(ui(r#"{"pick":"c","second":"c"}"#).is_err());
        assert!(ui(r#"{"pick":"c","mixFrom":"c"}"#).is_err());
        assert!(ui(r#"{"pick":"c","ratings":{"a":0}}"#).is_err());
        assert!(ui(r#"{"pick":"c","ratings":{"a":6}}"#).is_err());
        assert!(ui(r#"{"pick":"c","ratings":{"f":3}}"#).is_err());
        assert!(ui(r#"{"pick":"c","extra":1}"#).is_err());
    }

    #[test]
    fn interfaz_limita_el_largo_de_los_textos() {
        let long = "x".repeat(MAX_CHANGE + 1);
        assert_eq!(
            ui(&format!(r#"{{"pick":"a","change":"{long}"}}"#)),
            Err("text_too_long")
        );
        let ok = "ñ".repeat(MAX_MIX);
        assert!(ui(&format!(r#"{{"pick":"a","mixPart":"{ok}"}}"#)).is_ok());
    }

    #[test]
    fn cada_encuesta_valida_lo_suyo() {
        assert!(Kind::Marca.check(serde_json::json!({"pick":"a"})).is_err());
        assert!(Kind::Marca
            .check(serde_json::json!({"words":["vivo"]}))
            .is_ok());
        assert!(Kind::parse("otra").is_err());
        assert_eq!(Kind::parse("interfaz").unwrap().id(), "interfaz-1");
    }

    fn logo(json: &str) -> Result<serde_json::Value, &'static str> {
        Kind::Logo.check(serde_json::from_str(json).unwrap())
    }

    #[test]
    fn logo_pide_propuesta_y_paleta() {
        assert!(logo(r#"{}"#).is_err());
        assert!(logo(r#"{"pick":"mecha"}"#).is_err());
        assert!(logo(r#"{"palette":"actual"}"#).is_err());
        assert!(logo(r#"{"pick":"mecha","palette":"actual"}"#).is_ok());
        assert!(logo(r#"{"pick":"otra","palette":"actual"}"#).is_err());
        assert!(logo(r#"{"pick":"mecha","palette":"mono"}"#).is_err());
    }

    #[test]
    fn logo_completo_es_valido() {
        let v = logo(
            r#"{"pick":"sello","second":"kitsune","palette":"hora_azul","ratings":{"enso":2,"mecha":5},"comment":"la llama"}"#,
        )
        .unwrap();
        assert_eq!(v["second"], "kitsune");
        assert_eq!(v["ratings"]["mecha"], 5);
    }

    #[test]
    fn logo_rechaza_repetidas_notas_malas_y_textos_largos() {
        assert!(logo(r#"{"pick":"sello","second":"sello","palette":"sello"}"#).is_err());
        assert!(logo(r#"{"pick":"sello","palette":"sello","ratings":{"enso":6}}"#).is_err());
        assert!(logo(r#"{"pick":"sello","palette":"sello","ratings":{"a":3}}"#).is_err());
        assert!(logo(r#"{"pick":"sello","palette":"sello","extra":1}"#).is_err());
        let long = "x".repeat(MAX_COMMENT + 1);
        assert_eq!(
            logo(&format!(
                r#"{{"pick":"enso","palette":"actual","comment":"{long}"}}"#
            )),
            Err("text_too_long")
        );
        assert_eq!(Kind::parse("logo").unwrap().id(), "marca-2");
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
