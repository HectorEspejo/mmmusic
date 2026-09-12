CREATE TABLE ARTISTAS (
    id INTEGER PRIMARY KEY,
    nombre TEXT NOT NULL,
    nombre_norm TEXT NOT NULL UNIQUE,
    creado_en TEXT NOT NULL
);

CREATE TABLE ESCANEOS (
    id INTEGER PRIMARY KEY,
    iniciado_en TEXT NOT NULL,
    finalizado_en TEXT,
    estado TEXT NOT NULL,
    nuevas INTEGER NOT NULL DEFAULT 0,
    actualizadas INTEGER NOT NULL DEFAULT 0,
    eliminadas INTEGER NOT NULL DEFAULT 0,
    error_msg TEXT
);

CREATE TABLE ALBUMES (
    id INTEGER PRIMARY KEY,
    artista_id INTEGER NOT NULL REFERENCES ARTISTAS(id),
    titulo TEXT NOT NULL,
    titulo_norm TEXT NOT NULL,
    anio INTEGER,
    caratula_ruta TEXT,
    creado_en TEXT NOT NULL
);

CREATE UNIQUE INDEX idx_albumes_artista_titulo ON ALBUMES(artista_id, titulo_norm);

CREATE TABLE PISTAS (
    id INTEGER PRIMARY KEY,
    album_id INTEGER NOT NULL REFERENCES ALBUMES(id),
    artista_id INTEGER NOT NULL REFERENCES ARTISTAS(id),
    titulo TEXT NOT NULL,
    titulo_norm TEXT NOT NULL,
    numero_pista INTEGER,
    numero_disco INTEGER,
    genero TEXT,
    duracion_ms INTEGER NOT NULL,
    ruta TEXT NOT NULL UNIQUE,
    formato TEXT NOT NULL,
    tamano_bytes INTEGER NOT NULL,
    modificado_en INTEGER NOT NULL,
    bitrate_kbps INTEGER,
    anadido_en TEXT NOT NULL,
    escaneo_id INTEGER REFERENCES ESCANEOS(id)
);

CREATE INDEX idx_pistas_album_disco ON PISTAS(album_id, numero_disco, numero_pista);
CREATE INDEX idx_pistas_artista ON PISTAS(artista_id);
CREATE INDEX idx_pistas_titulo_norm ON PISTAS(titulo_norm);
CREATE INDEX idx_pistas_anadido ON PISTAS(anadido_en);

CREATE TABLE PLAYLISTS (
    id INTEGER PRIMARY KEY,
    nombre TEXT NOT NULL UNIQUE,
    creado_en TEXT NOT NULL,
    actualizado_en TEXT NOT NULL
);

CREATE TABLE PLAYLIST_PISTAS (
    id INTEGER PRIMARY KEY,
    playlist_id INTEGER NOT NULL REFERENCES PLAYLISTS(id) ON DELETE CASCADE,
    pista_id INTEGER NOT NULL REFERENCES PISTAS(id) ON DELETE CASCADE,
    posicion INTEGER NOT NULL
);

CREATE UNIQUE INDEX idx_playlist_pistas_posicion ON PLAYLIST_PISTAS(playlist_id, posicion);
CREATE INDEX idx_playlist_pistas_pista ON PLAYLIST_PISTAS(pista_id);

CREATE TABLE COLA (
    id INTEGER PRIMARY KEY,
    pista_id INTEGER NOT NULL REFERENCES PISTAS(id) ON DELETE CASCADE,
    posicion INTEGER NOT NULL UNIQUE,
    posicion_orig INTEGER NOT NULL
);

CREATE TABLE HISTORIAL_REPRODUCCION (
    id INTEGER PRIMARY KEY,
    pista_id INTEGER NOT NULL REFERENCES PISTAS(id) ON DELETE CASCADE,
    reproducido_en TEXT NOT NULL,
    completada INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX idx_historial_reproducido ON HISTORIAL_REPRODUCCION(reproducido_en);
CREATE INDEX idx_historial_pista ON HISTORIAL_REPRODUCCION(pista_id);

CREATE TABLE AJUSTES (
    clave TEXT PRIMARY KEY,
    valor TEXT NOT NULL
);
