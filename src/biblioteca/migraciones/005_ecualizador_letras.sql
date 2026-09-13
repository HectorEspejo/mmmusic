-- Fase 4: ecualizador y letras.
-- Tablas nuevas: PRESETS_EQ (presets integrados y propios) y LETRAS (offset y
-- fuente preferida por pista). Migración aditiva: no recrea tablas.
-- Las claves de AJUSTES y los presets integrados se siembran por código.

CREATE TABLE PRESETS_EQ (
    id INTEGER PRIMARY KEY,
    nombre TEXT NOT NULL,
    nombre_norm TEXT NOT NULL UNIQUE,
    ganancias TEXT NOT NULL,
    preamp_db REAL NOT NULL DEFAULT 0,
    integrado INTEGER NOT NULL DEFAULT 0,
    creado_en TEXT NOT NULL
);

CREATE TABLE LETRAS (
    pista_id INTEGER PRIMARY KEY REFERENCES PISTAS(id) ON DELETE CASCADE,
    offset_ms INTEGER NOT NULL DEFAULT 0,
    fuente_preferida TEXT,
    actualizado_en TEXT NOT NULL
);
