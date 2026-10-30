//! Documentos internos en Markdown, servidos por rol.
//!
//! `DOCS_ROOT` apunta a `.dev/plans/` del proyecto: el panel muestra los planes
//! tal como están y se actualiza solo. Estructura:
//!
//! ```text
//! DOCS_ROOT/
//!   REGLAS_DE_TRABAJO.md
//!   plan-general/
//!     _meta.json   { "title": "Plan general", "role": "VIEWER", "order": 1,
//!                    "files": ["PLAN.md", "ESTADO.md"] }
//!     PLAN.md
//! ```
//!
//! El `role` de una carpeta es el mínimo para verla a ella y a todo lo que tiene
//! dentro; una subcarpeta solo puede endurecerlo. `files` fija el orden de los
//! archivos (los que no aparecen van después, por nombre). Solo se sirven `.md`.

use crate::{
    auth::{ReadyUser, Role},
    error::{ApiError, ApiResult},
    state::AppState,
};
use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(tree))
        .route("/content", get(content))
        .route("/search", get(search))
}

#[derive(Debug, Deserialize, Default)]
struct Meta {
    title: Option<String>,
    role: Option<String>,
    order: Option<i32>,
    #[serde(default)]
    files: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Node {
    Folder {
        title: String,
        path: String,
        role: Role,
        children: Vec<Node>,
    },
    File {
        title: String,
        path: String,
    },
}

async fn tree(State(state): State<AppState>, user: ReadyUser) -> ApiResult<Json<Vec<Node>>> {
    let root = state.docs_root.clone();
    let viewer = user.0.role;
    let nodes = tokio::task::spawn_blocking(move || walk(&root, &root, Role::Viewer, viewer))
        .await
        .map_err(|e| anyhow::anyhow!(e))??;
    Ok(Json(nodes))
}

fn walk(root: &Path, dir: &Path, inherited: Role, viewer: Role) -> anyhow::Result<Vec<Node>> {
    let meta = read_meta(dir);
    let mut entries: Vec<(String, PathBuf, (i32, i32))> = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if hidden(&name) {
            continue;
        }
        let path = entry.path();
        let key = if path.is_dir() {
            (read_meta(&path).order.unwrap_or(i32::MAX), 1)
        } else if is_markdown(&path) {
            (0, position(&meta.files, &name))
        } else {
            continue;
        };
        entries.push((name, path, key));
    }
    // Archivos sueltos primero (en el orden de `files`), luego carpetas por `order`; empates por nombre.
    entries.sort_by(|a, b| a.2.cmp(&b.2).then_with(|| a.0.cmp(&b.0)));

    let mut nodes = Vec::new();
    for (name, path, _) in entries {
        let rel = path
            .strip_prefix(root)?
            .to_string_lossy()
            .replace('\\', "/");
        if path.is_dir() {
            let meta = read_meta(&path);
            let required = meta
                .role
                .as_deref()
                .and_then(Role::parse)
                .unwrap_or(inherited)
                .max(inherited);
            if viewer < required {
                continue;
            }
            let children = walk(root, &path, required, viewer)?;
            nodes.push(Node::Folder {
                title: meta.title.unwrap_or_else(|| pretty(&name)),
                path: rel,
                role: required,
                children,
            });
        } else {
            nodes.push(Node::File {
                title: file_title(&path).unwrap_or_else(|| pretty(&name)),
                path: rel,
            });
        }
    }
    Ok(nodes)
}

fn hidden(name: &str) -> bool {
    name.starts_with('_') || name.starts_with('.')
}

fn is_markdown(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()) == Some("md")
}

/// Posición en la lista `files` de `_meta.json`; lo que no está va al final.
fn position(order: &[String], name: &str) -> i32 {
    order
        .iter()
        .position(|f| f == name)
        .map(|i| i as i32)
        .unwrap_or(i32::MAX)
}

fn read_meta(dir: &Path) -> Meta {
    std::fs::read_to_string(dir.join("_meta.json"))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// Primer `# Título` del archivo, si lo hay.
fn file_title(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    title_of(&text)
}

fn title_of(text: &str) -> Option<String> {
    text.lines()
        .find_map(|l| l.strip_prefix("# ").map(|t| t.trim().to_string()))
}

/// `03-servicios.md` → `Servicios`; `PROMPT_NUEVO_CHAT.md` → `Prompt nuevo chat`
fn pretty(name: &str) -> String {
    let stem = name.strip_suffix(".md").unwrap_or(name);
    let stem = stem.trim_start_matches(|c: char| c.is_ascii_digit() || c == '-' || c == '_');
    let lower = stem.replace(['-', '_'], " ").to_lowercase();
    let mut chars = lower.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// Resuelve una ruta pedida dentro de `root` y devuelve el rol más estricto de su cadena de
/// carpetas. Rechaza todo lo que se salga de `root` o no sea `.md`.
fn resolve(root: &Path, rel: &str) -> ApiResult<(PathBuf, Role)> {
    let full = root
        .join(rel)
        .canonicalize()
        .map_err(|_| ApiError::NotFound)?;
    if !full.starts_with(root) || !is_markdown(&full) {
        return Err(ApiError::NotFound);
    }
    let rel_path = full.strip_prefix(root).map_err(|_| ApiError::NotFound)?;
    if rel_path
        .components()
        .any(|c| hidden(&c.as_os_str().to_string_lossy()))
    {
        return Err(ApiError::NotFound);
    }
    let mut required = Role::Viewer;
    let mut cur = root.to_path_buf();
    for comp in rel_path.components() {
        cur = cur.join(comp);
        if cur.is_dir() {
            if let Some(r) = read_meta(&cur).role.as_deref().and_then(Role::parse) {
                required = required.max(r);
            }
        }
    }
    Ok((full, required))
}

#[derive(Deserialize)]
struct ContentQuery {
    path: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Content {
    path: String,
    title: String,
    markdown: String,
    modified_at: Option<chrono::DateTime<chrono::Utc>>,
}

async fn content(
    State(state): State<AppState>,
    user: ReadyUser,
    Query(q): Query<ContentQuery>,
) -> ApiResult<Json<Content>> {
    let root = state.docs_root.clone();
    let viewer = user.0.role;
    let out = tokio::task::spawn_blocking(move || -> ApiResult<Content> {
        let (full, required) = resolve(&root, &q.path)?;
        if viewer < required {
            return Err(ApiError::Forbidden);
        }
        let markdown = std::fs::read_to_string(&full).map_err(|_| ApiError::NotFound)?;
        let modified_at = std::fs::metadata(&full)
            .and_then(|m| m.modified())
            .ok()
            .map(Into::into);
        let title = title_of(&markdown)
            .unwrap_or_else(|| pretty(&full.file_name().unwrap().to_string_lossy()));
        Ok(Content {
            path: q.path,
            title,
            markdown,
            modified_at,
        })
    })
    .await
    .map_err(|e| anyhow::anyhow!(e))??;
    Ok(Json(out))
}

#[derive(Deserialize)]
struct SearchQuery {
    q: String,
}

#[derive(Serialize, Debug)]
pub struct Hit {
    path: String,
    title: String,
    snippet: String,
}

const MAX_HITS: usize = 30;

/// Búsqueda de texto en los documentos que el rol puede ver. Sin distinguir mayúsculas ni tildes.
async fn search(
    State(state): State<AppState>,
    user: ReadyUser,
    Query(q): Query<SearchQuery>,
) -> ApiResult<Json<Vec<Hit>>> {
    let needle = fold(q.q.trim());
    if needle.chars().count() < 2 || needle.chars().count() > 100 {
        return Err(ApiError::BadRequest("invalid_query"));
    }
    let root = state.docs_root.clone();
    let viewer = user.0.role;
    let hits = tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<Hit>> {
        let mut files = Vec::new();
        collect_files(&walk(&root, &root, Role::Viewer, viewer)?, &mut files);
        let mut hits = Vec::new();
        for (path, title) in files {
            let text = std::fs::read_to_string(root.join(&path)).unwrap_or_default();
            if let Some(snippet) = snippet(&text, &needle) {
                hits.push(Hit {
                    path,
                    title,
                    snippet,
                });
            } else if fold(&title).contains(&needle) {
                hits.push(Hit {
                    path,
                    title,
                    snippet: String::new(),
                });
            }
            if hits.len() >= MAX_HITS {
                break;
            }
        }
        Ok(hits)
    })
    .await
    .map_err(|e| anyhow::anyhow!(e))??;
    Ok(Json(hits))
}

fn collect_files(nodes: &[Node], out: &mut Vec<(String, String)>) {
    for n in nodes {
        match n {
            Node::File { title, path } => out.push((path.clone(), title.clone())),
            Node::Folder { children, .. } => collect_files(children, out),
        }
    }
}

/// Minúsculas y sin tildes, carácter por carácter (la longitud se mantiene, así los índices coinciden).
fn fold(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'á' | 'à' | 'ä' | 'â' | 'Á' | 'À' | 'Ä' | 'Â' => 'a',
            'é' | 'è' | 'ë' | 'ê' | 'É' | 'È' | 'Ë' | 'Ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' | 'Í' | 'Ì' | 'Ï' | 'Î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' | 'Ó' | 'Ò' | 'Ö' | 'Ô' => 'o',
            'ú' | 'ù' | 'ü' | 'û' | 'Ú' | 'Ù' | 'Ü' | 'Û' => 'u',
            'ñ' | 'Ñ' => 'n',
            c => c.to_lowercase().next().unwrap_or(c),
        })
        .collect()
}

/// La línea donde aparece el texto buscado, recortada alrededor de la coincidencia.
fn snippet(text: &str, needle: &str) -> Option<String> {
    for line in text.lines() {
        let chars: Vec<char> = line.chars().collect();
        let folded: Vec<char> = fold(line).chars().collect();
        let n: Vec<char> = needle.chars().collect();
        if let Some(i) = folded.windows(n.len()).position(|w| w == n.as_slice()) {
            let start = i.saturating_sub(60);
            let end = (i + n.len() + 80).min(chars.len());
            let mut s: String = chars[start..end].iter().collect();
            s = s
                .trim()
                .trim_start_matches(['#', '-', '*', '>', ' '])
                .to_string();
            if start > 0 {
                s.insert(0, '…');
            }
            if end < chars.len() {
                s.push('…');
            }
            return Some(s);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pretty_quita_prefijo_y_extension() {
        assert_eq!(pretty("03-servicios.md"), "Servicios");
        assert_eq!(pretty("PROMPT_NUEVO_CHAT.md"), "Prompt nuevo chat");
    }

    #[test]
    fn orden_de_files() {
        let order = vec!["PLAN.md".to_string(), "ESTADO.md".to_string()];
        assert_eq!(position(&order, "PLAN.md"), 0);
        assert_eq!(position(&order, "ESTADO.md"), 1);
        assert_eq!(position(&order, "OTRO.md"), i32::MAX);
    }

    #[test]
    fn fold_ignora_tildes_y_mayusculas() {
        assert_eq!(fold("Diseño ÉXITO"), "diseno exito");
        assert_eq!(fold("Diseño").chars().count(), "Diseño".chars().count());
    }

    #[test]
    fn snippet_encuentra_sin_tildes() {
        let text = "# Título\n\nLa decisión se tomó el martes.";
        assert_eq!(
            snippet(text, &fold("decision")).as_deref(),
            Some("La decisión se tomó el martes.")
        );
        assert_eq!(snippet(text, "inexistente"), None);
    }

    #[test]
    fn resolve_no_sale_de_la_raiz_y_aplica_roles() {
        let dir = std::env::temp_dir().join(format!("ms-docs-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(dir.join("negocio")).unwrap();
        std::fs::write(dir.join("a.md"), "# A").unwrap();
        std::fs::write(
            dir.join("negocio/_meta.json"),
            r#"{ "role": "SUPERADMIN" }"#,
        )
        .unwrap();
        std::fs::write(dir.join("negocio/b.md"), "# B").unwrap();
        std::fs::write(dir.join("c.txt"), "x").unwrap();
        let root = dir.canonicalize().unwrap();

        assert_eq!(resolve(&root, "a.md").unwrap().1, Role::Viewer);
        assert_eq!(resolve(&root, "negocio/b.md").unwrap().1, Role::SuperAdmin);
        assert!(resolve(&root, "../etc/passwd").is_err());
        assert!(resolve(&root, "c.txt").is_err());
        assert!(resolve(&root, "negocio/_meta.json").is_err());

        let as_admin = walk(&root, &root, Role::Viewer, Role::Admin).unwrap();
        assert_eq!(as_admin.len(), 1, "un ADMIN no ve la carpeta de SUPERADMIN");
        let as_super = walk(&root, &root, Role::Viewer, Role::SuperAdmin).unwrap();
        assert_eq!(as_super.len(), 2);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
