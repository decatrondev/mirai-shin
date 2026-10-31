//! Biblioteca local: índice SQLite de la carpeta de fondos.
//!
//! Los archivos son la verdad; la base es un índice que se puede reconstruir.
//! - Migraciones numeradas que solo agregan; versión en `PRAGMA user_version`.
//! - Respaldo antes de migrar una base existente (se conservan los últimos).
//! - Base dañada: se aparta con otro nombre y se crea una nueva; el siguiente
//!   escaneo la vuelve a llenar.

use chrono::{DateTime, Utc};
use miraishin_core::manifest::{self, Issue};
use rusqlite::{params, Connection};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Migraciones en orden. La posición + 1 es la versión que deja la base.
const MIGRATIONS: &[&str] = &[include_str!("../migrations/0001_init.sql")];

/// Respaldos previos a migrar que se conservan.
pub const KEEP_BACKUPS: usize = 5;

/// Límites al recorrer la carpeta de un fondo, para que una carpeta enorme no frene el daemon.
const MAX_DEPTH: usize = 8;
const MAX_FILES: usize = 10_000;

/// Códigos propios del escaneo (los del manifiesto vienen de `core`).
pub const MISSING_MANIFEST: &str = "missing_manifest";
pub const UNREADABLE_MANIFEST: &str = "unreadable_manifest";
pub const DUPLICATE_ID: &str = "duplicate_id";

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("base de datos: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("archivo: {0}")]
    Io(#[from] std::io::Error),
    #[error("falló la migración a la versión {version}: {source}")]
    Migration {
        version: usize,
        source: rusqlite::Error,
    },
    #[error("la carpeta de fondos no existe")]
    WallpapersDirMissing,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OpenReport {
    /// La base estaba dañada y se apartó con este nombre.
    pub damaged_moved_to: Option<PathBuf>,
    pub version_before: usize,
    pub version_after: usize,
    /// Respaldo hecho antes de migrar.
    pub backup: Option<PathBuf>,
    /// La base la creó una versión más nueva de la app: se usa sin migrar.
    pub newer_db: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct ScanReport {
    pub valid: usize,
    pub invalid: usize,
    pub removed: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WallpaperRow {
    pub folder: String,
    pub id: Option<String>,
    pub kind: Option<String>,
    pub title: Option<String>,
    pub file: Option<String>,
    pub preview: Option<String>,
    pub valid: bool,
    pub newer_schema: bool,
    pub issues: Vec<String>,
    pub added_at: String,
    pub updated_at: String,
}

pub struct Library {
    conn: Connection,
}

impl Library {
    pub fn open(
        db: &Path,
        backups: &Path,
        now: DateTime<Utc>,
    ) -> Result<(Self, OpenReport), StoreError> {
        Self::open_with(db, backups, now, MIGRATIONS)
    }

    fn open_with(
        db: &Path,
        backups: &Path,
        now: DateTime<Utc>,
        migrations: &[&str],
    ) -> Result<(Self, OpenReport), StoreError> {
        if let Some(dir) = db.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let mut report = OpenReport::default();
        let existed = db.exists();
        let mut conn = match open_checked(db) {
            Ok(c) => c,
            Err(_) if existed => {
                let moved = set_aside(db, now)?;
                report.damaged_moved_to = Some(moved);
                open_checked(db)?
            }
            Err(e) => return Err(e.into()),
        };

        let version = user_version(&conn)?;
        report.version_before = version;
        report.version_after = version;
        let target = migrations.len();
        if version > target {
            report.newer_db = true;
        } else if version < target {
            if version > 0 {
                report.backup = Some(backup(&conn, backups, version, now)?);
            }
            for (i, sql) in migrations.iter().enumerate().skip(version) {
                let to = i + 1;
                let tx = conn.transaction()?;
                tx.execute_batch(sql)
                    .and_then(|_| tx.pragma_update(None, "user_version", to as i64))
                    .map_err(|source| StoreError::Migration {
                        version: to,
                        source,
                    })?;
                // Si algo falla antes del commit, la transacción se revierte al soltarse.
                tx.commit()?;
                report.version_after = to;
            }
        }
        Ok((Self { conn }, report))
    }

    /// Recorre la carpeta de fondos y deja el índice igual a lo que hay en disco.
    /// Si la carpeta no existe no toca nada (podría ser un disco sin montar).
    pub fn rescan(
        &mut self,
        wallpapers_dir: &Path,
        now: DateTime<Utc>,
    ) -> Result<ScanReport, StoreError> {
        if !wallpapers_dir.is_dir() {
            return Err(StoreError::WallpapersDirMissing);
        }
        let mut folders: Vec<String> = std::fs::read_dir(wallpapers_dir)?
            .filter_map(Result::ok)
            .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|name| !name.starts_with('.'))
            .collect();
        // Orden estable: ante ids repetidos gana siempre la misma carpeta.
        folders.sort();

        let mut seen_ids: HashSet<String> = HashSet::new();
        let mut entries = Vec::with_capacity(folders.len());
        for folder in &folders {
            let mut entry = scan_folder(&wallpapers_dir.join(folder));
            if let Some(id) = entry.id.clone().filter(|_| entry.valid) {
                if !seen_ids.insert(id) {
                    entry.valid = false;
                    entry.issues.push(DUPLICATE_ID.to_string());
                }
            }
            entries.push((folder.clone(), entry));
        }

        let stamp = now.to_rfc3339();
        let tx = self.conn.transaction()?;
        let mut report = ScanReport::default();
        for (folder, e) in &entries {
            tx.execute(
                "INSERT INTO wallpapers (folder, wallpaper_id, kind, title, file, preview, manifest_hash,
                     newer_schema, valid, issues, added_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11)
                 ON CONFLICT(folder) DO UPDATE SET
                     wallpaper_id = excluded.wallpaper_id, kind = excluded.kind, title = excluded.title,
                     file = excluded.file, preview = excluded.preview, manifest_hash = excluded.manifest_hash,
                     newer_schema = excluded.newer_schema, valid = excluded.valid, issues = excluded.issues,
                     updated_at = CASE WHEN wallpapers.manifest_hash = excluded.manifest_hash
                                        AND wallpapers.issues = excluded.issues
                                       THEN wallpapers.updated_at ELSE excluded.updated_at END",
                params![
                    folder,
                    e.id,
                    e.kind,
                    e.title,
                    e.file,
                    e.preview,
                    e.hash,
                    e.newer_schema,
                    e.valid,
                    serde_json::to_string(&e.issues).unwrap_or_else(|_| "[]".into()),
                    stamp,
                ],
            )?;
            if e.valid {
                report.valid += 1;
            } else {
                report.invalid += 1;
            }
        }
        let present: HashSet<&str> = folders.iter().map(String::as_str).collect();
        let stored: Vec<String> = {
            let mut stmt = tx.prepare("SELECT folder FROM wallpapers")?;
            let rows = stmt.query_map([], |r| r.get(0))?;
            rows.collect::<Result<_, _>>()?
        };
        for folder in stored.iter().filter(|f| !present.contains(f.as_str())) {
            tx.execute("DELETE FROM wallpapers WHERE folder = ?1", [folder])?;
            report.removed += 1;
        }
        tx.commit()?;
        Ok(report)
    }

    pub fn list(&self) -> Result<Vec<WallpaperRow>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT folder, wallpaper_id, kind, title, file, preview, valid, newer_schema, issues,
                    added_at, updated_at
             FROM wallpapers ORDER BY valid DESC, title COLLATE NOCASE, folder",
        )?;
        let rows = stmt.query_map([], |r| {
            let issues: String = r.get(8)?;
            Ok(WallpaperRow {
                folder: r.get(0)?,
                id: r.get(1)?,
                kind: r.get(2)?,
                title: r.get(3)?,
                file: r.get(4)?,
                preview: r.get(5)?,
                valid: r.get(6)?,
                newer_schema: r.get(7)?,
                issues: serde_json::from_str(&issues).unwrap_or_default(),
                added_at: r.get(9)?,
                updated_at: r.get(10)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// (válidos, inválidos)
    pub fn counts(&self) -> Result<(usize, usize), StoreError> {
        let (valid, invalid): (i64, i64) = self.conn.query_row(
            "SELECT COALESCE(SUM(valid = 1), 0), COALESCE(SUM(valid = 0), 0) FROM wallpapers",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        Ok((valid as usize, invalid as usize))
    }

    pub fn schema_version(&self) -> Result<usize, StoreError> {
        user_version(&self.conn)
    }
}

/// Abre y comprueba que el archivo sea una base SQLite sana.
fn open_checked(db: &Path) -> rusqlite::Result<Connection> {
    let conn = Connection::open(db)?;
    let check: String = conn.query_row("PRAGMA quick_check", [], |r| r.get(0))?;
    if check != "ok" {
        return Err(rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CORRUPT),
            Some(check),
        ));
    }
    Ok(conn)
}

fn user_version(conn: &Connection) -> Result<usize, StoreError> {
    let v: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    Ok(v.max(0) as usize)
}

fn file_stamp(now: DateTime<Utc>) -> String {
    now.format("%Y%m%d-%H%M%S").to_string()
}

/// Mueve la base dañada (y sus archivos de diario) a `library.db.danada-<fecha>`.
fn set_aside(db: &Path, now: DateTime<Utc>) -> std::io::Result<PathBuf> {
    let target = PathBuf::from(format!("{}.danada-{}", db.display(), file_stamp(now)));
    std::fs::rename(db, &target)?;
    for suffix in ["-journal", "-wal", "-shm"] {
        let side = PathBuf::from(format!("{}{suffix}", db.display()));
        if side.exists() {
            let _ = std::fs::rename(&side, format!("{}{suffix}", target.display()));
        }
    }
    Ok(target)
}

/// Copia la base con la API de respaldo de SQLite y rota los respaldos viejos.
fn backup(
    conn: &Connection,
    dir: &Path,
    version: usize,
    now: DateTime<Utc>,
) -> Result<PathBuf, StoreError> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(format!("library-{}-v{version}.db", file_stamp(now)));
    conn.backup(rusqlite::DatabaseName::Main, &path, None)?;
    rotate_backups(dir)?;
    Ok(path)
}

fn rotate_backups(dir: &Path) -> std::io::Result<()> {
    let mut names: Vec<String> = std::fs::read_dir(dir)?
        .filter_map(Result::ok)
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| n.starts_with("library-") && n.ends_with(".db"))
        .collect();
    // El nombre empieza con la fecha: orden alfabético = orden en el tiempo.
    names.sort();
    let extra = names.len().saturating_sub(KEEP_BACKUPS);
    for old in &names[..extra] {
        std::fs::remove_file(dir.join(old))?;
    }
    Ok(())
}

#[derive(Debug, Default)]
struct FolderScan {
    id: Option<String>,
    kind: Option<&'static str>,
    title: Option<String>,
    file: Option<String>,
    preview: Option<String>,
    hash: String,
    newer_schema: bool,
    valid: bool,
    issues: Vec<String>,
}

fn scan_folder(folder: &Path) -> FolderScan {
    let text = match std::fs::read_to_string(folder.join("manifest.json")) {
        Ok(t) => t,
        Err(e) => {
            let code = if e.kind() == std::io::ErrorKind::NotFound {
                MISSING_MANIFEST
            } else {
                UNREADABLE_MANIFEST
            };
            return FolderScan {
                issues: vec![code.to_string()],
                ..Default::default()
            };
        }
    };
    let hash = Sha256::digest(text.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let files = list_files(folder);
    match manifest::validate(&text, |p| files.contains(p)) {
        Ok(v) => FolderScan {
            id: Some(v.manifest.id),
            kind: Some(v.manifest.kind.as_str()),
            title: Some(v.manifest.title),
            file: Some(v.manifest.file),
            preview: v.manifest.preview,
            hash,
            newer_schema: v.newer_schema,
            valid: true,
            issues: Vec::new(),
        },
        Err(issues) => FolderScan {
            hash,
            issues: issues.iter().map(Issue::to_code_string).collect(),
            ..Default::default()
        },
    }
}

/// Archivos normales dentro de la carpeta, como rutas relativas con `/`.
/// No sigue enlaces simbólicos: un enlace podría apuntar fuera de la carpeta del fondo.
fn list_files(root: &Path) -> HashSet<String> {
    let mut out = HashSet::new();
    let mut stack = vec![(root.to_path_buf(), String::new(), 0usize)];
    while let Some((dir, prefix, depth)) = stack.pop() {
        let Ok(read) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in read.filter_map(Result::ok) {
            if out.len() >= MAX_FILES {
                return out;
            }
            let Ok(name) = entry.file_name().into_string() else {
                continue;
            };
            let Ok(meta) = std::fs::symlink_metadata(entry.path()) else {
                continue;
            };
            let rel = if prefix.is_empty() {
                name
            } else {
                format!("{prefix}/{name}")
            };
            if meta.is_file() {
                out.insert(rel);
            } else if meta.is_dir() && depth < MAX_DEPTH {
                stack.push((entry.path(), rel, depth + 1));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use std::collections::HashMap;

    fn at(h: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 10, 30, h, 0, 0).unwrap()
    }

    fn write_wallpaper(dir: &Path, folder: &str, manifest: &str, files: &[&str]) {
        let f = dir.join(folder);
        std::fs::create_dir_all(&f).unwrap();
        std::fs::write(f.join("manifest.json"), manifest).unwrap();
        for file in files {
            let p = f.join(file);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, b"x").unwrap();
        }
    }

    fn image(id: &str, title: &str) -> String {
        format!(
            r#"{{"schema_version":1,"id":"{id}","type":"image","file":"img/a.png","title":"{title}"}}"#
        )
    }

    #[test]
    fn base_nueva_queda_en_la_ultima_version_sin_respaldo() {
        let t = tempfile::tempdir().unwrap();
        let (lib, r) =
            Library::open(&t.path().join("library.db"), &t.path().join("b"), at(1)).unwrap();
        assert_eq!(r.version_before, 0);
        assert_eq!(r.version_after, MIGRATIONS.len());
        assert_eq!(r.backup, None);
        assert_eq!(lib.schema_version().unwrap(), MIGRATIONS.len());
        // Abrir de nuevo no migra ni respalda.
        drop(lib);
        let (_, r) =
            Library::open(&t.path().join("library.db"), &t.path().join("b"), at(2)).unwrap();
        assert_eq!((r.version_before, r.backup), (MIGRATIONS.len(), None));
    }

    #[test]
    fn respalda_antes_de_migrar_y_rota() {
        let t = tempfile::tempdir().unwrap();
        let db = t.path().join("library.db");
        let backups = t.path().join("backups");
        std::fs::create_dir_all(&backups).unwrap();
        for i in 0..KEEP_BACKUPS {
            std::fs::write(
                backups.join(format!("library-2020010{i}-000000-v1.db")),
                b"",
            )
            .unwrap();
        }
        let v1: &[&str] = &[MIGRATIONS[0]];
        let v2: &[&str] = &[
            MIGRATIONS[0],
            "ALTER TABLE wallpapers ADD COLUMN rating INTEGER;",
        ];

        let (lib, _) = Library::open_with(&db, &backups, at(1), v1).unwrap();
        lib.conn
            .execute(
                "INSERT INTO wallpapers (folder, manifest_hash, valid, added_at, updated_at)
                 VALUES ('a', '', 1, 'x', 'x')",
                [],
            )
            .unwrap();
        drop(lib);

        let (lib, r) = Library::open_with(&db, &backups, at(3), v2).unwrap();
        assert_eq!((r.version_before, r.version_after), (1, 2));
        let path = r.backup.expect("debió respaldar");
        assert!(path.ends_with("library-20261030-030000-v1.db"));
        assert_eq!(lib.schema_version().unwrap(), 2);

        // El respaldo es la base de antes de migrar, con sus datos.
        let copy = Connection::open(&path).unwrap();
        let v: i64 = copy
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        let n: i64 = copy
            .query_row("SELECT count(*) FROM wallpapers", [], |r| r.get(0))
            .unwrap();
        assert_eq!((v, n), (1, 1));
        let names = std::fs::read_dir(&backups).unwrap().count();
        assert_eq!(names, KEEP_BACKUPS, "rota los más viejos");
    }

    #[test]
    fn migracion_que_falla_no_deja_la_base_a_medias() {
        let t = tempfile::tempdir().unwrap();
        let db = t.path().join("library.db");
        let v1: &[&str] = &[MIGRATIONS[0]];
        let roto: &[&str] = &[
            MIGRATIONS[0],
            "ALTER TABLE wallpapers ADD COLUMN r INTEGER; ESTO NO ES SQL;",
        ];
        drop(Library::open_with(&db, &t.path().join("b"), at(1), v1).unwrap());
        let err = Library::open_with(&db, &t.path().join("b"), at(2), roto)
            .err()
            .unwrap();
        assert!(matches!(err, StoreError::Migration { version: 2, .. }));
        let (lib, _) = Library::open_with(&db, &t.path().join("b"), at(3), v1).unwrap();
        assert_eq!(lib.schema_version().unwrap(), 1);
        // La columna de la migración fallida no quedó.
        assert!(lib.conn.prepare("SELECT r FROM wallpapers").is_err());
    }

    #[test]
    fn base_de_una_version_mas_nueva_se_usa_sin_migrar() {
        let t = tempfile::tempdir().unwrap();
        let db = t.path().join("library.db");
        let (lib, _) = Library::open(&db, &t.path().join("b"), at(1)).unwrap();
        drop(lib);
        let conn = Connection::open(&db).unwrap();
        conn.pragma_update(None, "user_version", 99).unwrap();
        drop(conn);
        let (lib, r) = Library::open(&db, &t.path().join("b"), at(2)).unwrap();
        assert!(r.newer_db);
        assert_eq!(r.backup, None);
        assert_eq!(lib.schema_version().unwrap(), 99);
    }

    #[test]
    fn base_danada_se_aparta_y_se_reconstruye() {
        let t = tempfile::tempdir().unwrap();
        let db = t.path().join("library.db");
        std::fs::write(
            &db,
            b"esto no es una base sqlite, es basura de prueba......",
        )
        .unwrap();
        let wall = t.path().join("wallpapers");
        write_wallpaper(&wall, "a", &image("a", "A"), &["img/a.png"]);

        let (mut lib, r) = Library::open(&db, &t.path().join("b"), at(4)).unwrap();
        let moved = r.damaged_moved_to.expect("debió apartarse");
        assert!(moved.ends_with("library.db.danada-20261030-040000"));
        assert!(moved.exists());
        assert_eq!(lib.rescan(&wall, at(4)).unwrap().valid, 1);
    }

    #[test]
    fn escaneo_validos_invalidos_y_borrados() {
        let t = tempfile::tempdir().unwrap();
        let wall = t.path().join("wallpapers");
        write_wallpaper(&wall, "1-bueno", &image("x", "Bueno"), &["img/a.png"]);
        write_wallpaper(&wall, "2-sin-archivo", &image("y", "Roto"), &[]);
        write_wallpaper(&wall, "3-repetido", &image("x", "Copia"), &["img/a.png"]);
        std::fs::create_dir_all(wall.join("4-sin-manifiesto")).unwrap();
        std::fs::create_dir_all(wall.join(".oculta")).unwrap();
        std::fs::write(wall.join("suelto.png"), b"x").unwrap();

        let (mut lib, _) =
            Library::open(&t.path().join("library.db"), &t.path().join("b"), at(5)).unwrap();
        let r = lib.rescan(&wall, at(5)).unwrap();
        assert_eq!(
            r,
            ScanReport {
                valid: 1,
                invalid: 3,
                removed: 0
            }
        );
        assert_eq!(lib.counts().unwrap(), (1, 3));

        let rows = lib.list().unwrap();
        let by: HashMap<_, _> = rows.iter().map(|r| (r.folder.as_str(), r)).collect();
        assert!(by["1-bueno"].valid);
        assert_eq!(by["1-bueno"].file.as_deref(), Some("img/a.png"));
        assert_eq!(by["2-sin-archivo"].issues, vec!["file_not_found:file"]);
        assert_eq!(by["3-repetido"].issues, vec![DUPLICATE_ID]);
        assert_eq!(by["4-sin-manifiesto"].issues, vec![MISSING_MANIFEST]);

        // Se borra una carpeta y se corrige otra.
        std::fs::remove_dir_all(wall.join("4-sin-manifiesto")).unwrap();
        write_wallpaper(&wall, "2-sin-archivo", &image("y", "Roto"), &["img/a.png"]);
        let r = lib.rescan(&wall, at(6)).unwrap();
        assert_eq!(
            r,
            ScanReport {
                valid: 2,
                invalid: 1,
                removed: 1
            }
        );
        let rows = lib.list().unwrap();
        let fixed = rows.iter().find(|r| r.folder == "2-sin-archivo").unwrap();
        assert!(fixed.valid);
        assert_eq!(
            fixed.added_at,
            at(5).to_rfc3339(),
            "conserva la fecha de alta"
        );
        assert_eq!(fixed.updated_at, at(6).to_rfc3339());
        let same = rows.iter().find(|r| r.folder == "1-bueno").unwrap();
        assert_eq!(
            same.updated_at,
            at(5).to_rfc3339(),
            "sin cambios no se toca"
        );
    }

    #[test]
    fn carpeta_inexistente_no_toca_el_indice() {
        let t = tempfile::tempdir().unwrap();
        let wall = t.path().join("wallpapers");
        write_wallpaper(&wall, "a", &image("a", "A"), &["img/a.png"]);
        let (mut lib, _) =
            Library::open(&t.path().join("library.db"), &t.path().join("b"), at(1)).unwrap();
        lib.rescan(&wall, at(1)).unwrap();
        assert!(matches!(
            lib.rescan(&t.path().join("no-montada"), at(2)),
            Err(StoreError::WallpapersDirMissing)
        ));
        assert_eq!(lib.counts().unwrap(), (1, 0));
    }

    #[cfg(unix)]
    #[test]
    fn no_sigue_enlaces_simbolicos() {
        let t = tempfile::tempdir().unwrap();
        let wall = t.path().join("wallpapers");
        std::fs::write(t.path().join("fuera.png"), b"x").unwrap();
        write_wallpaper(&wall, "a", &image("a", "A"), &[]);
        std::fs::create_dir_all(wall.join("a/img")).unwrap();
        std::os::unix::fs::symlink(t.path().join("fuera.png"), wall.join("a/img/a.png")).unwrap();
        let (mut lib, _) =
            Library::open(&t.path().join("library.db"), &t.path().join("b"), at(1)).unwrap();
        assert_eq!(lib.rescan(&wall, at(1)).unwrap().invalid, 1);
    }
}
