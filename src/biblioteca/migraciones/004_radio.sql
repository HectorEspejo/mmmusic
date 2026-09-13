-- Fase 5: radio y streams.
-- Tablas nuevas: EMISORAS, EMISORA_TITULOS y BUSQUEDAS_RADIO.
-- Recreación de COLA, HISTORIAL_REPRODUCCION y ENVIOS para admitir emisoras:
-- pista_id pasa a NULL admisible y se añade emisora_id (cascada), tipo en
-- COLA y titulo_icy en HISTORIAL. La migración se ejecuta con las claves
-- foráneas desactivadas (bd::migrar) para no perder enlaces al recrear.

CREATE TABLE EMISORAS (
    id INTEGER PRIMARY KEY,
    nombre TEXT NOT NULL,
    nombre_norm TEXT NOT NULL,
    url TEXT NOT NULL UNIQUE,
    pagina_web TEXT,
    pais TEXT,
    etiquetas TEXT,
    codec TEXT,
    bitrate_kbps INTEGER,
    logo_url TEXT,
    logo_ruta TEXT,
    radiobrowser_uuid TEXT UNIQUE,
    favorita INTEGER NOT NULL DEFAULT 0,
    anadida_en TEXT NOT NULL,
    ultima_reproduccion TEXT,
    ultimo_error TEXT
);

CREATE INDEX idx_emisoras_nombre_norm ON EMISORAS(nombre_norm);
CREATE INDEX idx_emisoras_favorita ON EMISORAS(favorita, nombre_norm);
CREATE INDEX idx_emisoras_ultima ON EMISORAS(ultima_reproduccion);

CREATE TABLE EMISORA_TITULOS (
    id INTEGER PRIMARY KEY,
    emisora_id INTEGER NOT NULL REFERENCES EMISORAS(id) ON DELETE CASCADE,
    titulo TEXT NOT NULL,
    visto_en TEXT NOT NULL
);

CREATE INDEX idx_emisora_titulos_emisora ON EMISORA_TITULOS(emisora_id, visto_en);

CREATE TABLE BUSQUEDAS_RADIO (
    clave TEXT PRIMARY KEY,
    respuesta_json TEXT NOT NULL,
    obtenido_en TEXT NOT NULL
);

CREATE INDEX idx_busquedas_radio_obtenido ON BUSQUEDAS_RADIO(obtenido_en);

CREATE TABLE COLA_NUEVA (
    id INTEGER PRIMARY KEY,
    pista_id INTEGER REFERENCES PISTAS(id) ON DELETE CASCADE,
    emisora_id INTEGER REFERENCES EMISORAS(id) ON DELETE CASCADE,
    tipo TEXT NOT NULL DEFAULT 'pista',
    posicion INTEGER NOT NULL UNIQUE,
    posicion_orig INTEGER NOT NULL
);

INSERT INTO COLA_NUEVA (id, pista_id, emisora_id, tipo, posicion, posicion_orig)
    SELECT id, pista_id, NULL, 'pista', posicion, posicion_orig FROM COLA;

DROP TABLE COLA;
ALTER TABLE COLA_NUEVA RENAME TO COLA;

CREATE TABLE HISTORIAL_NUEVA (
    id INTEGER PRIMARY KEY,
    pista_id INTEGER REFERENCES PISTAS(id) ON DELETE CASCADE,
    emisora_id INTEGER REFERENCES EMISORAS(id) ON DELETE CASCADE,
    titulo_icy TEXT,
    reproducido_en TEXT NOT NULL,
    completada INTEGER NOT NULL DEFAULT 0
);

INSERT INTO HISTORIAL_NUEVA (id, pista_id, emisora_id, titulo_icy, reproducido_en, completada)
    SELECT id, pista_id, NULL, NULL, reproducido_en, completada FROM HISTORIAL_REPRODUCCION;

DROP TABLE HISTORIAL_REPRODUCCION;
ALTER TABLE HISTORIAL_NUEVA RENAME TO HISTORIAL_REPRODUCCION;

CREATE INDEX idx_historial_reproducido ON HISTORIAL_REPRODUCCION(reproducido_en);
CREATE INDEX idx_historial_pista ON HISTORIAL_REPRODUCCION(pista_id);
CREATE INDEX idx_historial_emisora ON HISTORIAL_REPRODUCCION(emisora_id, reproducido_en);

CREATE TABLE ENVIOS_NUEVA (
    id INTEGER PRIMARY KEY,
    servicio TEXT NOT NULL,
    tipo TEXT NOT NULL,
    pista_id INTEGER REFERENCES PISTAS(id) ON DELETE CASCADE,
    emisora_id INTEGER REFERENCES EMISORAS(id) ON DELETE CASCADE,
    historial_id INTEGER REFERENCES HISTORIAL_REPRODUCCION(id) ON DELETE SET NULL,
    reproducido_en TEXT,
    estado TEXT NOT NULL DEFAULT 'pendiente',
    intentos INTEGER NOT NULL DEFAULT 0,
    proximo_intento_en TEXT NOT NULL,
    error_msg TEXT,
    creado_en TEXT NOT NULL,
    enviado_en TEXT
);

INSERT INTO ENVIOS_NUEVA (
        id, servicio, tipo, pista_id, emisora_id, historial_id, reproducido_en,
        estado, intentos, proximo_intento_en, error_msg, creado_en, enviado_en)
    SELECT id, servicio, tipo, pista_id, NULL, historial_id, reproducido_en,
        estado, intentos, proximo_intento_en, error_msg, creado_en, enviado_en
    FROM ENVIOS;

DROP TABLE ENVIOS;
ALTER TABLE ENVIOS_NUEVA RENAME TO ENVIOS;

CREATE INDEX idx_envios_estado_proximo ON ENVIOS(estado, proximo_intento_en);
CREATE INDEX idx_envios_servicio_tipo_pista ON ENVIOS(servicio, tipo, pista_id);
CREATE INDEX idx_envios_emisora ON ENVIOS(emisora_id, estado);
