-- v2: qué fondo va en cada monitor. Es estado (lo elige la persona), no configuración.
-- output = nombre del monitor (DP-1, HDMI-A-1…) o '*' para «todos los monitores».
-- folder NULL = ese monitor sin fondo aunque haya uno para todos.
CREATE TABLE IF NOT EXISTS assignments (
    output     TEXT PRIMARY KEY,
    folder     TEXT,
    mode       TEXT NOT NULL DEFAULT 'fill',
    updated_at TEXT NOT NULL
);
