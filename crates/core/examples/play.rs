use std::{error::Error, path::{Path, PathBuf}, thread, time::Duration};
use player_core::{AudioDecoder, AudioOutput, Converter};
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