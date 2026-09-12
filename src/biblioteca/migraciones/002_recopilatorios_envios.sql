ALTER TABLE ALBUMES ADD COLUMN varios_artistas INTEGER NOT NULL DEFAULT 0;
ALTER TABLE ALBUMES ADD COLUMN carpeta TEXT;

DROP INDEX idx_albumes_artista_titulo;

CREATE UNIQUE INDEX idx_albumes_artista_titulo
    ON ALBUMES(artista_id, titulo_norm) WHERE varios_artistas = 0;
CREATE UNIQUE INDEX idx_albumes_titulo_carpeta_varios
    ON ALBUMES(titulo_norm, carpeta) WHERE varios_artistas = 1;

ALTER TABLE PISTAS ADD COLUMN carpeta TEXT NOT NULL DEFAULT '';
ALTER TABLE PISTAS ADD COLUMN artista_album_etiquetado INTEGER NOT NULL DEFAULT 0;

CREATE INDEX idx_pistas_carpeta ON PISTAS(carpeta);

CREATE TABLE ENVIOS (
    id INTEGER PRIMARY KEY,
    servicio TEXT NOT NULL,
    tipo TEXT NOT NULL,
    pista_id INTEGER NOT NULL REFERENCES PISTAS(id) ON DELETE CASCADE,
    historial_id INTEGER REFERENCES HISTORIAL_REPRODUCCION(id) ON DELETE SET NULL,
    reproducido_en TEXT,
    estado TEXT NOT NULL DEFAULT 'pendiente',
    intentos INTEGER NOT NULL DEFAULT 0,
    proximo_intento_en TEXT NOT NULL,
    error_msg TEXT,
    creado_en TEXT NOT NULL,
    enviado_en TEXT
);

CREATE INDEX idx_envios_estado_proximo ON ENVIOS(estado, proximo_intento_en);
CREATE INDEX idx_envios_servicio_tipo_pista ON ENVIOS(servicio, tipo, pista_id);

CREATE TABLE FAVORITAS (
    id INTEGER PRIMARY KEY,
    pista_id INTEGER NOT NULL UNIQUE REFERENCES PISTAS(id) ON DELETE CASCADE,
    marcada_en TEXT NOT NULL
);

INSERT INTO AJUSTES (clave, valor) VALUES ('reescaneo_completo_pendiente', '1');
