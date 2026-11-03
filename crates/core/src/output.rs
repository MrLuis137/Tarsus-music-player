use cpal::{
    traits::{DeviceTrait,HostTrait, StreamTrait},
    Device, Error, FromSample, OutputCallbackInfo, SampleFormat, SizedSample, Stream,
    StreamConfig, I24,
};

use ringbuf::{
    traits::{Consumer, Observer, Producer, Split},
    HeapCons, HeapProd, HeapRb,
};

#[derive(Debug, thiserror::Error)]
pub enum OutputError {
    #[error("No audio output")]
    NoDevice,
    #[error("Unsupported sample format: {0}")]
    UnsupportedFormat(String),
    #[error("Audio error: {0}")]
    Cpal(#[from] Error),
}

/// Salida de audio con UN stream persistente.
/// El stream se abre en `open()` y vive hasta que este valor se destruye.
/// Sin datos en el ring buffer, el callback emite silencio.
pub struct AudioOutput{
    _stream: Stream ,// el `_` evita el aviso de "no usado"; si se destruye, el audio se detiene
    producer: HeapProd<f32>,
    pub device_id: String,
    pub sample_rate: u32,
    pub channels: usize,
}

impl AudioOutput{
    pub fn open() -> Result<Self, OutputError>{
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or(OutputError::NoDevice)?;
        let device_id = device
            .id()
            .map(|id| id.to_string())
            .unwrap_or_else(|_| "unknown".into()); 

        let supported = device.default_output_config()?;
        let sample_format = supported.sample_format();
        let config: StreamConfig = supported.into();
        let sample_rate = config.sample_rate;
        let channels = config.channels as usize;

        // ~1 segundo de audio. El hilo que escribe es el productor;
        // el callback de cpal es el único consumidor.
        let ring = HeapRb::<f32>::new(sample_rate as usize * channels);
        let (producer, consumer) = ring.split();

        let stream =  match sample_format {
            SampleFormat::F32 => build_stream::<f32>(&device, config, consumer),
            SampleFormat::F64 => build_stream::<f64>(&device, config, consumer),
            SampleFormat::I32 => build_stream::<i32>(&device, config, consumer),
            SampleFormat::I24 => build_stream::<I24>(&device, config, consumer),
            SampleFormat::I16 => build_stream::<i16>(&device, config, consumer),
            SampleFormat::U16 => build_stream::<u16>(&device, config, consumer),
            other => return Err(OutputError::UnsupportedFormat(other.to_string())),
        }?;

        stream.play()?;

        Ok(Self {
            _stream: stream,
            producer,
            device_id,
            sample_rate,
            channels,
        })
    }

    /// Encola muestras f32 intercaladas. Devuelve cuántas cupieron.
    /// No bloquea: si el buffer está lleno, quien llama decide qué hacer.
    pub fn write(&mut self, samples: &[f32]) -> usize {
        self.producer.push_slice(samples)
    }

    // Muestras pendientes de reproducir en el ring buffer.
    pub fn queued_samples(&self) -> usize {
        self.producer.occupied_len()
    }

}

fn build_stream<T>(
    device: &Device,
    config: StreamConfig,
    mut consumer: HeapCons<f32>,
) -> Result<Stream, Error>
    where T: SizedSample + FromSample<f32>,
{
    let err_fn = |err: Error| eprintln!("Audio error: {err}");

    device.build_output_stream(
        config,
        move |data: &mut [T], _: & OutputCallbackInfo|{
            // Callback en tiempo real: sin bloqueos ni reservas de memoria.
            // Sin datos disponibles → silencio.
            for out in data.iter_mut(){
                *out = T::from_sample(consumer.try_pop().unwrap_or(0.0));
            }
        },
        err_fn,
        None,
    )
}