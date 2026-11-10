//! `miraishin-web`: API JSON del panel `/admin` de la landing.
//!
//! La landing es un build estático que sirve nginx; este binario solo responde
//! `/api/*`. Tiene las tablas `admin_users` y `survey_responses` (encuesta
//! pública de la marca) y lee los documentos internos en
//! Markdown desde `DOCS_ROOT`, filtrados por el rol de quien pide.
//!
//! Subcomandos para el servidor (claves temporales por la salida estándar):
//!   miraishin-web create-user <correo> <VIEWER|ADMIN|SUPERADMIN>
//!   miraishin-web reset-password <correo>

mod auth;
mod brand;
mod cli;
mod config;
mod docs;
mod error;
mod limiter;
mod state;
mod survey;
mod users;

use axum::{routing::get, Json, Router};
use state::AppState;
use tower_http::trace::TraceLayer;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if !args.is_empty() {
        return cli::run(&args).await;
    }

    init_tracing();
    let state = AppState::init().await?;

    let app = Router::new()
        .route("/api/health", get(|| async { "ok" }))
        .route("/api/brand", get(|| async { Json(brand::Brand::load()) }))
        .nest("/api/auth", auth::router())
        .nest("/api/docs", docs::router())
        .nest("/api/users", users::router())
        .nest("/api/survey", survey::router())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr = config::or_default("BIND_ADDR", "127.0.0.1:3034");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!(%addr, name = %brand::Brand::load().name, "escuchando");
    axum::serve(listener, app).await?;
    Ok(())
}

/// Texto plano a stdout: screen → /tmp/miraishin-web.log → Promtail → Loki.
fn init_tracing() {
    use tracing_subscriber::{fmt, EnvFilter};
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    fmt().with_env_filter(filter).with_target(false).init();
}
