-- v1: índice de la carpeta de fondos. Los archivos son la verdad; esta tabla se puede reconstruir.
-- Regla: las migraciones solo agregan (nada de renombrar ni borrar columnas).
CREATE TABLE IF NOT EXISTS wallpapers (
    folder        TEXT PRIMARY KEY,      -- nombre de la carpeta, relativo a la carpeta de fondos
    wallpaper_id  TEXT,                  -- id del manifiesto (NULL si no se pudo leer)
    kind          TEXT,                  -- image | video
    title         TEXT,
    file          TEXT,                  -- archivo principal, relativo a la carpeta del fondo
    preview       TEXT,
    manifest_hash TEXT NOT NULL,         -- sha256 del manifest.json ('' si no existe)
    newer_schema  INTEGER NOT NULL DEFAULT 0,
    valid         INTEGER NOT NULL,
    issues        TEXT NOT NULL DEFAULT '[]', -- códigos JSON, p. ej. ["missing_field:title"]
    added_at      TEXT NOT NULL,
    updated_at    TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS wallpapers_valid ON wallpapers (valid);
