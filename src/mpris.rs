use std::sync::mpsc::Sender;
use std::thread;

use anyhow::{Context, Result};
use mpris_server::{LoopStatus, Metadata, PlaybackStatus, Player, Time, TrackId};
use tokio::sync::watch;
use tracing::{info, warn};

use crate::eventos::AppEvento;
use crate::reproductor::ComandoReproductor;
use crate::reproductor::estado::{Estado, EstadoReproduccion, Repeticion};

const NOMBRE_BUS: &str = "mmmusic";

pub fn lanzar(
    rx_estado: watch::Receiver<EstadoReproduccion>,
    tx_cmd: Sender<ComandoReproductor>,
    tx_app: Sender<AppEvento>,
) -> Result<()> {
    thread::Builder::new()
        .name("mpris".to_string())
        .spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(error) => {
                    warn!("no se pudo crear el runtime de MPRIS: {error}");
                    return;
                }
            };
            runtime.block_on(async move {
                if let Err(error) = ejecutar(rx_estado, tx_cmd, tx_app).await {
                    warn!("MPRIS no disponible: {error:#}");
                }
            });
        })
        .context("no se pudo lanzar el hilo MPRIS")?;
    Ok(())
}

async fn ejecutar(
    rx_estado: watch::Receiver<EstadoReproduccion>,
    tx_cmd: Sender<ComandoReproductor>,
    tx_app: Sender<AppEvento>,
) -> Result<()> {
    let player = Player::builder(NOMBRE_BUS)
        .identity("mmmusic")
        .desktop_entry("mmmusic")
        .can_quit(true)
        .can_raise(false)
        .can_control(true)
        .can_play(true)
        .can_pause(true)
        .can_seek(true)
        .can_go_next(true)
        .can_go_previous(true)
        .build()
        .await
        .context("no se pudo registrar org.mpris.MediaPlayer2.mmmusic")?;

    {
        let cmd = tx_cmd.clone();
        player.connect_play_pause(move |_| {
            let _ = cmd.send(ComandoReproductor::AlternarPausa);
        });
    }
    {
        let cmd = tx_cmd.clone();
        player.connect_play(move |_| {
            let _ = cmd.send(ComandoReproductor::Reanudar);
        });
    }
    {
        let cmd = tx_cmd.clone();
        player.connect_pause(move |_| {
            let _ = cmd.send(ComandoReproductor::Pausar);
        });
    }
    {
        let cmd = tx_cmd.clone();
        player.connect_stop(move |_| {
            let _ = cmd.send(ComandoReproductor::Detener);
        });
    }
    {
        let cmd = tx_cmd.clone();
        player.connect_next(move |_| {
            let _ = cmd.send(ComandoReproductor::Siguiente);
        });
    }
    {
        let cmd = tx_cmd.clone();
        player.connect_previous(move |_| {
            let _ = cmd.send(ComandoReproductor::Anterior);
        });
    }
    {
        let cmd = tx_cmd.clone();
        player.connect_seek(move |_, offset| {
            let _ = cmd.send(ComandoReproductor::Buscar {
                ms: offset.as_millis(),
                relativo: true,
            });
        });
    }
    {
        let cmd = tx_cmd.clone();
        player.connect_set_position(move |_, _pista, posicion| {
            let _ = cmd.send(ComandoReproductor::Buscar {
                ms: posicion.as_millis(),
                relativo: false,
            });
        });
    }
    {
        let cmd = tx_cmd.clone();
        let app = tx_app.clone();
        player.connect_quit(move |_| {
            let _ = cmd.send(ComandoReproductor::Apagar);
            let _ = app.send(AppEvento::Salir);
        });
    }

    info!("MPRIS registrado como {NOMBRE_BUS}");
    let tarea_dbus = player.run();
    let tarea_estado = actualizar_estado(rx_estado, &player);
    tokio::select! {
        _ = tarea_dbus => {}
        _ = tarea_estado => {}
    }
    info!("MPRIS detenido");
    Ok(())
}

async fn actualizar_estado(
    mut rx: watch::Receiver<EstadoReproduccion>,
    player: &Player,
) -> Result<()> {
    let mut ultima_posicion = {
        let estado = rx.borrow().clone();
        aplicar_estado(&estado, player).await;
        estado.posicion_ms
    };
    while rx.changed().await.is_ok() {
        let estado = rx.borrow().clone();
        aplicar_estado(&estado, player).await;
        if (estado.posicion_ms - ultima_posicion).abs() > 2_000 {
            let _ = player.seeked(Time::from_millis(estado.posicion_ms)).await;
        }
        ultima_posicion = estado.posicion_ms;
    }
    Ok(())
}

async fn aplicar_estado(estado: &EstadoReproduccion, player: &Player) {
    let reproduciendo = match estado.estado {
        Estado::Reproduciendo => PlaybackStatus::Playing,
        Estado::Pausado => PlaybackStatus::Paused,
        _ => PlaybackStatus::Stopped,
    };
    let _ = player.set_playback_status(reproduciendo).await;
    let repeticion = match estado.repeticion {
        Repeticion::No => LoopStatus::None,
        Repeticion::Todo => LoopStatus::Playlist,
        Repeticion::Una => LoopStatus::Track,
    };
    let _ = player.set_loop_status(repeticion).await;
    let _ = player.set_shuffle(estado.aleatorio).await;
    let _ = player.set_volume(f64::from(estado.volumen) / 100.0).await;
    player.set_position(Time::from_millis(estado.posicion_ms));
    let metadatos = metadata_de(estado).unwrap_or_default();
    let _ = player.set_metadata(metadatos).await;
    let hay_cola = !estado.cola.is_empty();
    let _ = player.set_can_play(hay_cola).await;
    let _ = player.set_can_go_next(hay_cola).await;
    let _ = player.set_can_go_previous(hay_cola).await;
    let _ = player
        .set_can_pause(matches!(
            estado.estado,
            Estado::Reproduciendo | Estado::Pausado
        ))
        .await;
    let _ = player.set_can_seek(estado.pista_cargada()).await;
}

fn metadata_de(estado: &EstadoReproduccion) -> Option<Metadata> {
    let pista = estado.pista_actual.as_ref()?;
    let trackid = TrackId::try_from(format!("/org/mmmusic/track/{}", pista.id)).ok()?;
    let mut constructor = Metadata::builder()
        .trackid(trackid)
        .title(pista.titulo.clone())
        .artist([pista.artista.clone()])
        .album(pista.album.clone())
        .length(Time::from_millis(pista.duracion_ms))
        .url(format!("file://{}", pista.ruta));
    if let Some(caratula) = pista.caratula_ruta.as_ref() {
        constructor = constructor.art_url(format!("file://{caratula}"));
    }
    Some(constructor.build())
}
