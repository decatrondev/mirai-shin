CREATE TABLE IF NOT EXISTS admin_users (
    id                   UUID PRIMARY KEY,
    email                TEXT NOT NULL UNIQUE,
    password_hash        TEXT NOT NULL,
    role                 TEXT NOT NULL CHECK (role IN ('VIEWER', 'ADMIN', 'SUPERADMIN')),
    -- clave temporal: el panel obliga a cambiarla antes de mostrar nada
    must_change_password BOOLEAN NOT NULL DEFAULT TRUE,
    -- invalida los tokens emitidos antes de un cambio de clave
    password_changed_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_at           TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_login_at        TIMESTAMPTZ
);
