-- Encuesta pública de la marca (miraishin.decatron.net/marca). Solo nombre, respuestas, idioma y fechas:
-- no se guarda la IP ni el correo. El código para cambiar las respuestas se guarda con SHA-256.
CREATE TABLE IF NOT EXISTS survey_responses (
    id          UUID PRIMARY KEY,
    survey      TEXT NOT NULL,
    name        TEXT NOT NULL,
    lang        TEXT NOT NULL,
    answers     JSONB NOT NULL,
    token_hash  TEXT NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS survey_responses_survey_idx ON survey_responses (survey, created_at);
