use std::{fs::File, io::ErrorKind, path::Path};

use symphonia::core::{
    audio::SampleBuffer,
    codecs::{Decoder,DecoderOptions, CODEC_TYPE_NULL},
    errors::Error as SymphoniaError,
    formats::{FormatOptions, FormatReader},
    io::MediaSourceStream,
    meta::MetadataOptions,
    probe::Hint,
};

#[derive(Debug, thiserror::Error)]
pub enum DecodeError{
    #[error("no se pudo abrir el archivo: {0}")]
    Io(#[from] std::io::Error),
    #[error("formato no soportado: {0}")]
    Unsupported(&'static str),
    #[error("error de decodificación: {0}")]
    Decode(#[from] SymphoniaError),
}


/// Decodifica un archivo de audio a muestras f32 intercaladas.
/// No sabe nada de `cpal` ni de la UI: solo produce muestras.
pub struct AudioDecoder{
    format: Box<dyn FormatReader>,
    decoder: Box<dyn Decoder>,
    track_id:u32,
    sample_buf: Option<SampleBuffer<f32>>,
    pub sample_rate: u32,
    pub channels: usize,
    /// Total de frames (muestras por canal), si el formato lo declara.
    pub total_frames: Option<u64>
}


impl AudioDecoder{
    pub fn open(path: &Path) -> Result<Self, DecodeError>{
        let file = File::open(path)?;
        let mms = MediaSourceStream::new(Box:: new(file), Default::default());

        // La extensión es solo una pista; symphonia detecta el formato real.
        let mut hint = Hint::new();
        if let Some(ext) = path.extension().and_then(|e| e.to_str()){
            hint.with_extension(ext);
        }

        let probed = symphonia::default::get_probe().format(
            &hint,
            mms,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )?;
        let format = probed.format;

        let track= format
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
            .ok_or(DecodeError::Unsupported("File have no audio track"))?;

        let params = &track.codec_params;
        let sample_rate =  params.sample_rate
            .ok_or(DecodeError::Unsupported("Unknown Sample Frecuency"))?;
        let channels = params
            .channels
            .ok_or(DecodeError::Unsupported("Unknown Channels"))?
            .count();
        let total_frames = params.n_frames;
        let track_id = track.id;

        let decoder = 
            symphonia::default::get_codecs().make(params, &DecoderOptions::default())?;

        Ok(Self {
            format,
            decoder,
            track_id,
            sample_buf: None,
            sample_rate,
            channels,
            total_frames,
        })
    }

    
    /// Devuelve el siguiente bloque de muestras f32 intercaladas,
    /// o `None` al llegar al final del archivo.
    pub fn next_chunk(&mut self) -> Result<Option<&[f32]>, DecodeError>{
        loop{
            let packet = match self.format.next_packet(){
                Ok(p) => p,
                // En symphonia 0.5 el fin de archivo llega como UnexpectedEof.
                Err(SymphoniaError::IoError(e)) if e.kind() == ErrorKind::UnexpectedEof =>{
                    return Ok(None)
                }
                Err(e) => return Err(e.into()),
            };

            if packet.track_id() != self.track_id{
                continue;
            }

            match self.decoder.decode(&packet) {
                Ok(decoded) => {
                    let spec =  *decoded.spec();
                    let needed = decoded.capacity() * spec.channels.count();

                    // (Re)crear el buffer si no existe o se quedó pequeño.

                    let too_small = self.sample_buf
                        .as_ref()
                        .map_or(true, |b| b.capacity() < needed);
                    if too_small {
                        self.sample_buf =  Some(SampleBuffer::new(decoded.capacity() as u64, spec));
                    }
                    let buf = self.sample_buf.as_mut().expect("Buffer alreade crated");
                    buf.copy_interleaved_ref(decoded);
                    return Ok(Some(buf.samples()));
                }

                 // Paquete corrupto: se salta y se sigue.
                Err(SymphoniaError::DecodeError(_)) => continue,
                Err(e) => return Err(e.into()),
            }
        }
    }
}