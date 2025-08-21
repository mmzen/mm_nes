use std::collections::VecDeque;
use std::fmt::Debug;
use std::sync::{Arc, Mutex};
use log::{debug, info};
use sdl2::audio::{AudioCallback, AudioDevice, AudioSpecDesired};
use sdl2::Sdl;
use crate::sound_playback::{SoundPlayback, SoundPlaybackError};

const SAMPLE_RATE: i32 = 44_100;
const BUFFER_SIZE: usize = 65536;
const CHUNK_SIZE: u16 = 1024;

#[derive(Debug)]
struct NESAudioBuffer {
    buffer: VecDeque<f32>
}

impl NESAudioBuffer {

    fn new() -> Self {
        NESAudioBuffer {
            buffer: VecDeque::with_capacity(BUFFER_SIZE)
        }
    }

    fn push_sample(&mut self, sample: f32) {
        let safe_sample = sample.clamp(-1.0, 1.0);

        if self.buffer.len() >= BUFFER_SIZE {
            self.buffer.pop_front();
        }

        self.buffer.push_back(safe_sample);
    }

    fn pop_sample(&mut self) -> f32 {
        self.buffer.pop_front().unwrap_or(0.0)
    }
}

struct NesAudioCallback {
    audio_buffer: Arc<Mutex<NESAudioBuffer>>
}

impl NesAudioCallback {
    fn new(audio_buffer: Arc<Mutex<NESAudioBuffer>>) -> Self {
        NesAudioCallback {
            audio_buffer
        }
    }
}

impl AudioCallback for NesAudioCallback {
    type Channel = f32;

    fn callback(&mut self, out: &mut [f32]) {

        if let Ok(mut buf) = self.audio_buffer.lock() {
            for i in out.iter_mut() {
                *i = buf.pop_sample();
            }
        }
    }
}

pub struct SoundPlaybackSDL2Callback {
    audio_buffer: Arc<Mutex<NESAudioBuffer>>,
    audio_device: AudioDevice<NesAudioCallback>
}

impl Debug for SoundPlaybackSDL2Callback {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SoundPlaybackSDL2Callback").finish()
    }
}

impl SoundPlayback for SoundPlaybackSDL2Callback {
    fn push_sample(&mut self, sample: f32) {
        self.audio_buffer.lock().unwrap().push_sample(sample);
    }

    fn resume(&self) {
        self.audio_device.resume()
    }
}

impl SoundPlaybackSDL2Callback {

    pub fn new(sdl_context: &Sdl) -> Result<Self, SoundPlaybackError> {

        info!("initializing audio system (callback): buffer size: {} samples, chunk size: {} samples ...",
            BUFFER_SIZE, CHUNK_SIZE);

        let audio_subsystem = sdl_context.audio().unwrap();

        let desired_spec = AudioSpecDesired {
            freq: Some(SAMPLE_RATE),
            channels: Some(1),
            samples: Some(CHUNK_SIZE),
        };

        let audio_buffer = Arc::new(Mutex::new(NESAudioBuffer::new()));
        let audio_callback = NesAudioCallback::new(audio_buffer.clone());

        let audio_device = audio_subsystem
            .open_playback(None, &desired_spec, |_| audio_callback)
            .unwrap();

        let mut player = SoundPlaybackSDL2Callback {
            audio_buffer,
            audio_device,
        };

        player.prefill();
        player.resume();

        Ok(player)
    }

    fn prefill(&mut self) {
        for _ in 0..(SAMPLE_RATE as usize / 5) { // ~200ms
            self.push_sample(0.0);
        }
    }
}