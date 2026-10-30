//! Subcomandos para quien administra el servidor. Imprimen la clave temporal por
//! la salida estándar: se redirige a un archivo con permisos solo de root, nunca a un chat.

use crate::{auth::Role, error::ApiError, state, users};

pub async fn run(args: &[String]) -> anyhow::Result<()> {
    let db = state::connect().await?;
    match args {
        [cmd, email, role] if cmd == "create-user" => {
            let role = Role::parse(role).ok_or_else(|| anyhow::anyhow!("rol inválido: usa VIEWER, ADMIN o SUPERADMIN"))?;
            let (user, password) = users::insert_user(&db, email, role).await.map_err(explain)?;
            println!("{}\t{}\t{}", user.email(), role.as_str(), password);
        }
        [cmd, email] if cmd == "reset-password" => {
            let email = crate::auth::normalize_email(email).ok_or_else(|| anyhow::anyhow!("correo inválido"))?;
            let id: uuid::Uuid = sqlx::query_scalar("SELECT id FROM admin_users WHERE email = $1")
                .bind(&email)
                .fetch_optional(&db)
                .await?
                .ok_or_else(|| anyhow::anyhow!("no existe un usuario con ese correo"))?;
            let (user, password) = users::reset_password(&db, id).await.map_err(explain)?;
            println!("{}\t{}", user.email(), password);
        }
        _ => anyhow::bail!(
            "uso:\n  miraishin-web create-user <correo> <VIEWER|ADMIN|SUPERADMIN>\n  miraishin-web reset-password <correo>"
        ),
    }
    Ok(())
}

fn explain(e: ApiError) -> anyhow::Error {
    match e {
        ApiError::Conflict(code) | ApiError::BadRequest(code) => anyhow::anyhow!(code),
        other => anyhow::anyhow!(other.to_string()),
    }
}
