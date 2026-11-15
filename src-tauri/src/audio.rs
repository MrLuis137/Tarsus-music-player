use std::{
    path::{Path, PathBuf},
    sync::mpsc::{self, TryRecvError},
    thread,
    time::Duration,
};

use player_core::{AudioDecoder, AudioOutput, Converter};
use tauri::{AppHandle, Emitter};

enum Command {
    Play {
        path: PathBuf,
        reply: mpsc::Sender<Result<(), String>>,
    },
}

/// Handle al hilo de audio. Es `Send + Sync`, así que puede vivir en el estado de Tauri.
/// El `cpal::Stream` nunca sale del hilo que lo creó.
pub struct AudioHandle {
    tx: mpsc::Sender<Command>,
}

impl AudioHandle {
    pub fn spawn(app: AppHandle) -> Self {
        let (tx, rx) = mpsc::channel();
        thread::Builder::new()
            .name("audio".into())
            .spawn(move || run(rx, app))
            .expect("no se pudo crear el hilo de audio");
        Self { tx }
    }

    /// Abre el archivo y empieza a reproducirlo. Devuelve error si no se puede
    /// abrir el archivo o si no hay dispositivo de salida.
    pub fn play(&self, path: PathBuf) -> Result<(), String> {
        let (reply, rx) = mpsc::channel();
        self.tx
            .send(Command::Play { path, reply })
            .map_err(|_| "El hilo de audio no está activo".to_string())?;
        rx.recv_timeout(Duration::from_secs(5))
            .map_err(|_| "El hilo de audio no respondió".to_string())?
    }
}

struct Playing {
    dec: AudioDecoder,
    conv: Converter,
    pending: Vec<f32>, // muestras ya convertidas, pendientes de entrar al ring buffer
    offset: usize,
}

fn start(out: &AudioOutput, path: &Path) -> Result<Playing, String> {
    let dec = AudioDecoder::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let conv = Converter::new(dec.sample_rate, out.sample_rate, dec.channels, out.channels)?;
    Ok(Playing { dec, conv, pending: Vec::new(), offset: 0 })
}

fn run(rx: mpsc::Receiver<Command>, app: AppHandle) {
    // El stream se abre UNA vez y vive mientras viva el hilo.
    let mut output =
        AudioOutput::open().map_err(|e| format!("No se pudo abrir la salida de audio: {e}"));
    let mut playing: Option<Playing> = None;

    loop {
        // Sin pista: dormimos hasta recibir un comando. Con pista: sondeamos sin bloquear.
        let cmd = if playing.is_some() {
            match rx.try_recv() {
                Ok(c) => Some(c),
                Err(TryRecvError::Empty) => None,
                Err(TryRecvError::Disconnected) => break,
            }
        } else {
            match rx.recv() {
                Ok(c) => Some(c),
                Err(_) => break,
            }
        };

        if let Some(Command::Play { path, reply }) = cmd {
            let result = match output.as_ref() {
                Ok(out) => start(out, &path),
                Err(e) => Err(e.clone()),
            };
            match result {
                Ok(p) => {
                    playing = Some(p); // reemplaza la pista anterior, si la había
                    let _ = reply.send(Ok(()));
                }
                Err(e) => {
                    let _ = reply.send(Err(e));
                }
            }
        }

        let mut done = false;
        if let (Some(p), Ok(out)) = (playing.as_mut(), output.as_mut()) {
            if p.offset < p.pending.len() {
                p.offset += out.write(&p.pending[p.offset..]);
            }
            if p.offset >= p.pending.len() {
                match p.dec.next_chunk() {
                    Ok(Some(chunk)) => {
                        p.pending = p.conv.process(chunk);
                        p.offset = 0;
                    }
                    Ok(None) => done = true,
                    Err(e) => {
                        let _ = app.emit("audio-error", format!("Error de decodificación: {e}"));
                        done = true;
                    }
                }
            } else {
                thread::sleep(Duration::from_millis(5)); // ring buffer lleno
            }
        }
        if done {
            playing = None;
        }
    }
}