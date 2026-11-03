use std::{error::Error, path::{Path, PathBuf}, thread, time::Duration};

use player_core::{AudioDecoder, AudioOutput}; // <- cambia por el nombre real de tu crate

fn main() -> Result<(), Box<dyn Error>> {
    let paths: Vec<PathBuf> = std::env::args().skip(1).map(PathBuf::from).collect();
    if paths.is_empty() {
        eprintln!("uso: play <archivo> [archivo...]");
        return Ok(());
    }

    // El stream se abre UNA vez y se reutiliza para todas las pistas.
    let mut out = AudioOutput::open()?;
    println!(
        "Salida: {} ({} Hz, {} canales)",
        out.device_id, out.sample_rate, out.channels
    );

    for path in &paths {
        println!("Reproduciendo {}", path.display());
        play_file(&mut out, path)?;
    }

    // Esperar a que el buffer se vacíe y a que el dispositivo termine su cola interna.
    while out.queued_samples() > 0 {
        thread::sleep(Duration::from_millis(20));
    }
    thread::sleep(Duration::from_millis(300));
    println!("Fin. El stream seguía abierto durante todo el proceso.");
    Ok(())
}

fn play_file(out: &mut AudioOutput, path: &Path) -> Result<(), Box<dyn Error>> {
    let mut dec = AudioDecoder::open(path)?;
    println!("  {} Hz, {} canales", dec.sample_rate, dec.channels);
    let mut conv = Converter::new(dec.sample_rate, out.sample_rate, dec.channels, out.channels)?;

    while let Some(chunk) = dec.next_chunk()? {
        let converted = conv.process(chunk);
        push_blocking(out, &converted);
    }
    Ok(())
}

/// Escribe todo el bloque esperando a que haya sitio en el ring buffer.
fn push_blocking(out: &mut AudioOutput, mut samples: &[f32]) {
    while !samples.is_empty() {
        let n = out.write(samples);
        samples = &samples[n..];
        if !samples.is_empty() {
            thread::sleep(Duration::from_millis(5));
        }
    }
}

/// DEUDA TÉCNICA: conversor mínimo (interpolación lineal) solo para pruebas.
/// Sustituir por `rubato` en el motor real (ver nota al final de la historia).
struct Converter {
    ratio: f64, // frecuencia de entrada / frecuencia de salida
    in_ch: usize,
    out_ch: usize,
    pos: f64,       // posición fraccionaria; el índice -1 es el último frame del bloque anterior
    last: Vec<f32>, // último frame del bloque anterior
}

impl Converter {
    fn new(in_rate: u32, out_rate: u32, in_ch: usize, out_ch: usize) -> Result<Self, String> {
        if in_ch != out_ch && in_ch != 1 {
            return Err(format!("{in_ch} canales → {out_ch}: aún no soportado"));
        }
        Ok(Self {
            ratio: in_rate as f64 / out_rate as f64,
            in_ch,
            out_ch,
            pos: 0.0,
            last: vec![0.0; in_ch],
        })
    }

    fn process(&mut self, input: &[f32]) -> Vec<f32> {
        let in_ch = self.in_ch;
        let n = input.len() / in_ch;
        if n == 0 {
            return Vec::new();
        }

        let sample = |idx: isize, ch: usize| -> f32 {
            if idx < 0 {
                self.last[ch]
            } else {
                input[idx as usize * in_ch + ch]
            }
        };

        let mut out = Vec::with_capacity(((n as f64 / self.ratio) as usize + 2) * self.out_ch);
        let mut p = self.pos;
        while p < (n - 1) as f64 {
            let i = p.floor();
            let frac = (p - i) as f32;
            let i = i as isize;
            for c in 0..self.out_ch {
                let src = if in_ch == 1 { 0 } else { c }; // mono → duplica en todos los canales
                let a = sample(i, src);
                let b = sample(i + 1, src);
                out.push(a + (b - a) * frac);
            }
            p += self.ratio;
        }

        self.pos = p - n as f64;
        for ch in 0..in_ch {
            self.last[ch] = input[(n - 1) * in_ch + ch];
        }
        out
    }
}