use anyhow::Context;
use chrono;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::{Arc, Mutex};

const SAMPLE_RATE: u32 = 16000;
const CHANNELS: u16 = 1;

pub struct AudioRecorder {
    stream: Option<cpal::Stream>,
    buffer: Arc<Mutex<Vec<f32>>>,
}

impl AudioRecorder {
    pub fn new() -> Self {
        Self {
            stream: None,
            buffer: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn start_recording(&mut self) -> anyhow::Result<()> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .context("No input device found")?;

        let config = cpal::StreamConfig {
            channels: CHANNELS,
            sample_rate: SAMPLE_RATE,
            buffer_size: cpal::BufferSize::Default,
        };

        let buffer = self.buffer.clone();

        let err_fn = |err| {
            eprintln!("Audio stream error: {}", err);
        };

        let stream = device.build_input_stream(
            &config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                if let Ok(mut buf) = buffer.lock() {
                    buf.extend_from_slice(data);
                }
            },
            err_fn,
            None,
        )?;

        stream.play()?;
        self.stream = Some(stream);
        self.buffer.lock().unwrap().clear();

        Ok(())
    }

    pub fn stop_recording(&mut self) -> Vec<f32> {
        self.stream = None;
        self.buffer.lock().unwrap().clone()
    }

    pub fn is_recording(&self) -> bool {
        self.stream.is_some()
    }
}

/// Multiply every sample by `gain`, clamping to [-1.0, 1.0] to prevent clipping.
pub fn apply_gain(samples: Vec<f32>, gain: f32) -> Vec<f32> {
    samples.into_iter().map(|s| (s * gain).clamp(-1.0, 1.0)).collect()
}

/// Save f32 PCM samples as a 16-bit mono WAV file.
/// Returns the path written, or an error.
pub fn save_wav(samples: &[f32], out_dir: &str) -> anyhow::Result<String> {
    use std::io::Write;

    std::fs::create_dir_all(out_dir)?;

    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let path = format!("{}/recording_{}.wav", out_dir, timestamp);

    // Convert f32 [-1,1] → i16
    let pcm_i16: Vec<i16> = samples
        .iter()
        .map(|&s| (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16)
        .collect();

    let data_len = (pcm_i16.len() * 2) as u32;
    let mut f = std::fs::File::create(&path)?;

    // RIFF header
    f.write_all(b"RIFF")?;
    f.write_all(&(36u32 + data_len).to_le_bytes())?;
    f.write_all(b"WAVE")?;

    // fmt chunk — 16-bit mono PCM at 16 kHz
    f.write_all(b"fmt ")?;
    f.write_all(&16u32.to_le_bytes())?;          // chunk size
    f.write_all(&1u16.to_le_bytes())?;            // PCM
    f.write_all(&1u16.to_le_bytes())?;            // channels
    f.write_all(&SAMPLE_RATE.to_le_bytes())?;     // sample rate
    f.write_all(&(SAMPLE_RATE * 2).to_le_bytes())?; // byte rate
    f.write_all(&2u16.to_le_bytes())?;            // block align
    f.write_all(&16u16.to_le_bytes())?;           // bits per sample

    // data chunk
    f.write_all(b"data")?;
    f.write_all(&data_len.to_le_bytes())?;
    for s in &pcm_i16 {
        f.write_all(&s.to_le_bytes())?;
    }

    Ok(path)
}