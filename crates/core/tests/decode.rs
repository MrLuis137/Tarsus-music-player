use std::path::PathBuf;

use player_core::AudioDecoder;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    .join("tests/fixtures")
    .join(name)
}

fn decode_all(name: &str) -> (u32, usize, usize, f32) {
    let mut dec = AudioDecoder::open(&fixture(name)).expect("open");
    let(rate,channels) = (dec.sample_rate, dec.channels);
    let mut samples = 0usize;
    let mut peak = 0f32;
    while let Some(chunk) = dec.next_chunk().expect("Decode"){
        samples += chunk.len();
        peak = chunk.iter().fold(peak, |m,s| m.max(s.abs()));
    }
    (rate,channels,samples,peak)
}

#[test]
fn decode_flac() {
    let (rate, ch, samples, peak) = decode_all("test.flac");
    assert_eq!((rate, ch), (44100, 2));
    assert_eq!(samples / ch, 2 * 44100); // FLAC es exacto
    assert!(peak > 0.01, "el audio no debe ser silencio");
}

#[test]
fn decode_mp3() {
    let (rate, ch, samples, peak) = decode_all("test.mp3");
    assert_eq!((rate, ch), (44100, 2));
    println!("peak: {peak}");
    // MP3 añade relleno del codificador: se tolera ±0,1 s.
    let frames = (samples / ch) as i64;
    assert!((frames - 2 * 44100).abs() < 4410, "frames = {frames}");
    assert!(peak > 0.01, "el audio no debe ser silencio");
}

#[test]
fn archivo_inexistente_da_error() {
    assert!(AudioDecoder::open(&fixture("no-existe.flac")).is_err());
}