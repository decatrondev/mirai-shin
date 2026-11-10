use crate::{config, limiter::LoginLimiter};
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::{path::PathBuf, sync::Arc};

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub jwt_secret: String,
    pub docs_root: PathBuf,
    pub limiter: Arc<LoginLimiter>,
    /// Envíos de la encuesta pública por IP (mismo tope y ventana que los logins fallidos).
    pub survey_limiter: Arc<LoginLimiter>,
}

impl AppState {
    pub async fn init() -> anyhow::Result<Self> {
        let db = connect().await?;
        let docs_root = PathBuf::from(config::required("DOCS_ROOT")?).canonicalize()?;
        anyhow::ensure!(docs_root.is_dir(), "DOCS_ROOT no es una carpeta");
        let jwt_secret = config::required("JWT_SECRET")?;
        anyhow::ensure!(
            jwt_secret.len() >= 32,
            "JWT_SECRET debe tener al menos 32 caracteres"
        );
        Ok(Self {
            db,
            jwt_secret,
            docs_root,
            limiter: Arc::new(LoginLimiter::default()),
            survey_limiter: Arc::new(LoginLimiter::default()),
        })
    }
}

/// Conecta y aplica las migraciones pendientes (son idempotentes).
pub async fn connect() -> anyhow::Result<PgPool> {
    let db = PgPoolOptions::new()
        .max_connections(5)
        .connect(&config::required("DATABASE_URL")?)
        .await?;
    sqlx::migrate!("./migrations").run(&db).await?;
    Ok(db)
}
