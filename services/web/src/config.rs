use std::env;

/// Variable de entorno obligatoria; el error nombra la variable. Carga `.env` la primera vez.
pub fn required(key: &str) -> anyhow::Result<String> {
    let _ = dotenvy::dotenv();
    env::var(key).map_err(|_| anyhow::anyhow!("falta la variable de entorno {key}"))
}

pub fn or_default(key: &str, default: &str) -> String {
    let _ = dotenvy::dotenv();
    env::var(key).unwrap_or_else(|_| default.to_string())
}
