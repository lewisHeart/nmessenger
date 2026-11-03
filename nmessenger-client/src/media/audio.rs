use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

pub const AVAILABLE: bool = cfg!(feature = "native-audio");
pub type Samples = Arc<Mutex<VecDeque<i16>>>;

pub fn stop_playback() {
    super::playback::stop();
}

pub fn wav(samples: &[i16]) -> Result<Vec<u8>, String> {
    let mut cursor = std::io::Cursor::new(Vec::new());
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 16_000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    {
        let mut writer = hound::WavWriter::new(&mut cursor, spec).map_err(|e| e.to_string())?;
        for sample in samples {
            writer.write_sample(*sample).map_err(|e| e.to_string())?;
        }
        writer.finalize().map_err(|e| e.to_string())?;
    }
    Ok(cursor.into_inner())
}

pub fn decode_wav(bytes: &[u8]) -> Result<Vec<i16>, String> {
    let mut reader = hound::WavReader::new(std::io::Cursor::new(bytes))
        .map_err(|e| format!("Áudio WAV inválido: {e}"))?;
    let spec = reader.spec();
    if !(8_000..=192_000).contains(&spec.sample_rate)
        || !(1..=8).contains(&spec.channels)
        || reader.duration() > spec.sample_rate * 60
    {
        return Err("Use áudio WAV de até 60s, 8–192 kHz e até 8 canais.".into());
    }
    let raw: Vec<f32> = if spec.sample_format == hound::SampleFormat::Float {
        reader
            .samples::<f32>()
            .collect::<Result<_, _>>()
            .map_err(|e| e.to_string())?
    } else {
        let scale = 2_f32.powi(spec.bits_per_sample as i32 - 1);
        reader
            .samples::<i32>()
            .map(|s| s.map(|s| s as f32 / scale))
            .collect::<Result<_, _>>()
            .map_err(|e| e.to_string())?
    };
    let mono: Vec<f32> = raw
        .chunks(spec.channels as usize)
        .map(|frame| frame.iter().sum::<f32>() / frame.len() as f32)
        .collect();
    if mono.is_empty() {
        return Err("O áudio está vazio.".into());
    }
    let count = mono.len() * 16_000 / spec.sample_rate as usize;
    Ok((0..count)
        .map(|i| {
            let position = i as f64 * spec.sample_rate as f64 / 16_000.0;
            let index = position as usize;
            let a = mono[index.min(mono.len() - 1)];
            let b = mono[(index + 1).min(mono.len() - 1)];
            let sample = a + (b - a) * (position.fract() as f32);
            (sample.clamp(-1.0, 1.0) * 32768.0)
                .round()
                .clamp(-32768.0, 32767.0) as i16
        })
        .collect())
}

/// Notification streams are independent of deliberate voice-message playback.
pub fn play_notification(bytes: &[u8], device: &str) -> Result<(), String> {
    // Presence/message bursts must not open competing notification streams.
    static NOTIFICATION_OUTPUT: Mutex<()> = Mutex::new(());
    let _output = NOTIFICATION_OUTPUT
        .lock()
        .map_err(|_| "Saída de notificações interrompida")?;
    let samples = decode_wav(bytes)?;
    if super::test_media() && device == "test:null" {
        std::thread::sleep(std::time::Duration::from_secs_f32(
            samples.len() as f32 / 16000.0,
        ));
        return Ok(());
    }
    #[cfg(feature = "native-audio")]
    {
        native::output(
            Arc::new(Mutex::new(samples.into())),
            Arc::new(AtomicBool::new(false)),
            false,
            Arc::new(AtomicBool::new(false)),
            device,
            || {},
        )
    }
    #[cfg(not(feature = "native-audio"))]
    {
        let _ = (samples, device);
        Err("Áudio nativo indisponível nesta compilação.".into())
    }
}

#[cfg(feature = "native-audio")]
mod native {
    use super::*;
    use cpal::traits::{DeviceTrait, StreamTrait};
    use std::time::Duration;

    pub fn input(
        device_name: &str,
        stop: Arc<AtomicBool>,
        limit: bool,
        emit: impl Fn(Vec<i16>) + Send + 'static,
        ready: impl FnOnce(),
    ) -> Result<Vec<i16>, String> {
        crate::platform::microphone_permission()?;
        if stop.load(Ordering::Relaxed) {
            return Ok(Vec::new());
        }
        let (device, supported) = {
            let primary = super::super::devices::input(device_name);
            match primary.and_then(|d| {
                d.default_input_config()
                    .map(|cfg| (d, cfg))
                    .map_err(|e| e.to_string())
            }) {
                Ok(pair) => pair,
                Err(first_err) => {
                    if !device_name.is_empty() {
                        let fallback = super::super::devices::input("");
                        fallback
                            .and_then(|d| {
                                d.default_input_config()
                                    .map(|cfg| (d, cfg))
                                    .map_err(|e| e.to_string())
                            })
                            .map_err(|_| first_err)?
                    } else {
                        return Err(first_err);
                    }
                }
            }
        };
        let format = supported.sample_format();
        let config: cpal::StreamConfig = supported.into();
        let samples = Arc::new(Mutex::new(Vec::new()));
        let queue = Arc::new(Mutex::new(VecDeque::<i16>::new()));
        let failed = Arc::new(Mutex::new(None::<String>));
        macro_rules! stream {
            ($ty:ty, $convert:expr) => {{
                let samples = samples.clone();
                let queue = queue.clone();
                let stop_callback = stop.clone();
                let failed = failed.clone();
                let channels = config.channels as usize;
                let rate = config.sample_rate;
                let mut phase = 0u64;
                device.build_input_stream(
                    config.clone(),
                    move |data: &[$ty], _| {
                        let mut output = samples.lock().unwrap();
                        let mut live = queue.lock().unwrap();
                        for frame in data.chunks(channels) {
                            phase += 16_000;
                            let mono = frame.iter().map($convert).sum::<f32>() / channels as f32;
                            let sample = (mono.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
                            while phase >= rate as u64 {
                                phase -= rate as u64;
                                if limit {
                                    if output.len() >= 16_000 * 60 {
                                        stop_callback.store(true, Ordering::Relaxed);
                                        break;
                                    }
                                    output.push(sample);
                                } else if live.len() < 32_000 {
                                    live.push_back(sample);
                                }
                            }
                        }
                    },
                    move |e| *failed.lock().unwrap() = Some(format!("Microfone: {e}")),
                    None,
                )
            }};
        }
        let stream = match format {
            cpal::SampleFormat::F32 => stream!(f32, |s: &f32| *s),
            cpal::SampleFormat::I16 => stream!(i16, |s: &i16| *s as f32 / 32768.0),
            cpal::SampleFormat::U16 => stream!(u16, |s: &u16| (*s as f32 - 32768.0) / 32768.0),
            cpal::SampleFormat::I32 => stream!(i32, |s: &i32| *s as f32 / 2147483648.0),
            cpal::SampleFormat::U32 => {
                stream!(u32, |s: &u32| (*s as f32 - 2147483648.0) / 2147483648.0)
            }
            cpal::SampleFormat::F64 => stream!(f64, |s: &f64| *s as f32),
            _ => return Err("Formato do microfone não suportado (use f32/i16/u16).".into()),
        }
        .map_err(|e| e.to_string())?;
        stream.play().map_err(|e| e.to_string())?;
        ready();
        while !stop.load(Ordering::Relaxed) {
            if let Some(error) = failed.lock().unwrap().take() {
                return Err(error);
            }
            if !limit {
                let chunk: Vec<i16> = {
                    let mut queue = queue.lock().unwrap();
                    if queue.len() >= 320 {
                        queue.drain(..320).collect()
                    } else {
                        Vec::new()
                    }
                };
                if !chunk.is_empty() {
                    emit(chunk);
                }
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        drop(stream);
        let result = std::mem::take(&mut *samples.lock().unwrap());
        Ok(result)
    }

    pub fn output(
        samples: Samples,
        stop: Arc<AtomicBool>,
        live: bool,
        paused: Arc<AtomicBool>,
        device_name: &str,
        ready: impl FnOnce(),
    ) -> Result<(), String> {
        if stop.load(Ordering::Relaxed) {
            return Ok(());
        }
        let (device, supported) = {
            let primary = super::super::devices::output(device_name);
            match primary.and_then(|d| {
                d.default_output_config()
                    .map(|cfg| (d, cfg))
                    .map_err(|e| e.to_string())
            }) {
                Ok(pair) => pair,
                Err(first_err) => {
                    if !device_name.is_empty() {
                        let fallback = super::super::devices::output("");
                        fallback
                            .and_then(|d| {
                                d.default_output_config()
                                    .map(|cfg| (d, cfg))
                                    .map_err(|e| e.to_string())
                            })
                            .map_err(|_| first_err)?
                    } else {
                        return Err(first_err);
                    }
                }
            }
        };
        let format = supported.sample_format();
        let config: cpal::StreamConfig = supported.into();
        let failed = Arc::new(Mutex::new(None::<String>));
        macro_rules! stream {
            ($ty:ty, $convert:expr) => {{
                let queue = samples.clone();
                let paused = paused.clone();
                let failed = failed.clone();
                let channels = config.channels as usize;
                let rate = config.sample_rate as u64;
                let mut phase = rate;
                let mut sample = 0i16;
                device.build_output_stream(
                    config.clone(),
                    move |data: &mut [$ty], _| {
                        let mut queue = queue.lock().unwrap();
                        for frame in data.chunks_mut(channels) {
                            while phase >= rate {
                                phase -= rate;
                                sample = if paused.load(Ordering::Relaxed) {
                                    0
                                } else {
                                    queue.pop_front().unwrap_or(0)
                                };
                            }
                            phase += 16_000;
                            for out in frame {
                                *out = ($convert)(sample);
                            }
                        }
                    },
                    move |e| *failed.lock().unwrap() = Some(format!("Saída de áudio: {e}")),
                    None,
                )
            }};
        }
        let stream = match format {
            cpal::SampleFormat::F32 => stream!(f32, |s: i16| s as f32 / 32768.0),
            cpal::SampleFormat::I16 => stream!(i16, |s: i16| s),
            cpal::SampleFormat::U16 => stream!(u16, |s: i16| (s as i32 + 32768) as u16),
            cpal::SampleFormat::I32 => stream!(i32, |s: i16| (s as i32) << 16),
            cpal::SampleFormat::U32 => stream!(u32, |s: i16| ((s as i64 + 32768) << 16) as u32),
            cpal::SampleFormat::F64 => stream!(f64, |s: i16| s as f64 / 32768.0),
            _ => return Err("Formato de saída não suportado.".into()),
        }
        .map_err(|e| e.to_string())?;
        stream.play().map_err(|e| e.to_string())?;
        ready();
        loop {
            if let Some(error) = failed.lock().unwrap().take() {
                return Err(error);
            }
            if stop.load(Ordering::Relaxed) {
                break;
            }
            if !live && samples.lock().unwrap().is_empty() {
                std::thread::sleep(Duration::from_millis(120));
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        Ok(())
    }
}

pub fn record_on(
    device: &str,
    stop: Arc<AtomicBool>,
    limit: bool,
    emit: impl Fn(Vec<i16>) + Send + 'static,
    ready: impl FnOnce(),
) -> Result<Vec<i16>, String> {
    if super::test_media() && device == "test:sine" {
        ready();
        let mut all = Vec::new();
        let mut position = 0usize;
        while !stop.load(Ordering::Relaxed) && (!limit || all.len() < 960_000) {
            let chunk: Vec<i16> = (0..320)
                .map(|offset| {
                    (((position + offset) as f32 * 440.0 * std::f32::consts::TAU / 16000.0).sin()
                        * 6000.0) as i16
                })
                .collect();
            position += chunk.len();
            if limit {
                all.extend_from_slice(&chunk);
            } else {
                emit(chunk);
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        return Ok(all);
    }
    #[cfg(feature = "native-audio")]
    {
        native::input(device, stop, limit, emit, ready)
    }
    #[cfg(not(feature = "native-audio"))]
    {
        let _ = (device, stop, limit, emit, ready);
        Err("Áudio nativo não habilitado".into())
    }
}

pub fn output_on(
    device: &str,
    samples: Samples,
    stop: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
    ready: impl FnOnce(),
) -> Result<(), String> {
    if super::test_media() && device == "test:null" {
        ready();
        while !stop.load(Ordering::Relaxed) {
            if !paused.load(Ordering::Relaxed) {
                let mut queue = samples.lock().unwrap();
                let count = queue.len().min(320);
                queue.drain(..count);
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        return Ok(());
    }
    #[cfg(feature = "native-audio")]
    {
        native::output(samples, stop, true, paused, device, ready)
    }
    #[cfg(not(feature = "native-audio"))]
    {
        let _ = (device, samples, stop, paused, ready);
        Err("Áudio nativo não habilitado".into())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn stereo_wav_is_mixed_and_resampled_with_its_duration_preserved() {
        let mut bytes = std::io::Cursor::new(Vec::new());
        let mut writer = hound::WavWriter::new(
            &mut bytes,
            hound::WavSpec {
                channels: 2,
                sample_rate: 48_000,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )
        .unwrap();
        for _ in 0..4800 {
            writer.write_sample(12000_i16).unwrap();
            writer.write_sample(4000_i16).unwrap();
        }
        writer.finalize().unwrap();
        let decoded = super::decode_wav(bytes.get_ref()).unwrap();
        assert_eq!(decoded.len(), 1600);
        assert!(decoded.iter().all(|s| *s == 8000));
    }
    #[test]
    fn voice_payload_is_lossless_mono_16khz_signed_16bit_wav() {
        let samples = [i16::MIN, -1000, 0, 1000, i16::MAX];
        let bytes = super::wav(&samples).unwrap();
        let mut reader = hound::WavReader::new(std::io::Cursor::new(bytes)).unwrap();
        assert_eq!(reader.spec().sample_rate, 16_000);
        assert_eq!(reader.spec().channels, 1);
        assert_eq!(reader.spec().bits_per_sample, 16);
        assert_eq!(
            reader
                .samples::<i16>()
                .collect::<Result<Vec<_>, _>>()
                .unwrap(),
            samples
        );
    }
}
