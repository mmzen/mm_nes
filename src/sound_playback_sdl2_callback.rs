use std::collections::VecDeque;
use std::fmt::Debug;
use std::sync::{Arc, Mutex};
use log::debug;
use sdl2::audio::{AudioCallback, AudioDevice, AudioSpecDesired};
use sdl2::Sdl;
use crate::sound_playback::{SoundPlayback, SoundPlaybackError};

const SAMPLE_RATE: i32 = 44_100;
const BUFFER_SIZE: usize = 8192;

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
        self.buffer.push_back(sample);
    }

    fn pop_sample(&mut self) -> f32 {
        self.buffer.pop_front().unwrap_or(0.0)
    }
}

struct NesAudioCallback {
        audio_buffer: NESAudioBuffer
}

impl NesAudioCallback {
    fn new(audio_buffer: NESAudioBuffer) -> Self {
        NesAudioCallback {
            audio_buffer
        }
    }
}

impl AudioCallback for NesAudioCallback {
    type Channel = f32;

    fn callback(&mut self, out: &mut [f32]) {
        for i in out.iter_mut() {
            *i = self.audio_buffer.pop_sample();
        }
    }
}

pub struct SoundPlaybackSDL2 {
    audio_device: AudioDevice<NesAudioCallback>
}

impl Debug for SoundPlaybackSDL2 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SoundPlaybackSDL2").finish()
    }
}

impl SoundPlayback for SoundPlaybackSDL2 {
    fn push_sample(&mut self, sample: f32) {
        let clamped_sample = sample.clamp(-1.0, 1.0);
        self.audio_device.lock().audio_buffer.push_sample(clamped_sample)
    }

    fn resume(&self) {
        self.audio_device.resume()
    }
}

impl SoundPlaybackSDL2 {

    pub fn new(sdl_context: &Sdl) -> Result<Self, SoundPlaybackError> {

        debug!("initializing audio system...");

        let audio_subsystem = sdl_context.audio().unwrap();

        let desired_spec = AudioSpecDesired {
            freq: Some(SAMPLE_RATE),
            channels: Some(1),
            samples: None,
        };

        let audio_buffer = NESAudioBuffer::new();
        let audio_callback = NesAudioCallback::new(audio_buffer);

        let audio_device = audio_subsystem
            .open_playback(None, &desired_spec, |_| audio_callback)
            .unwrap();

        let player = SoundPlaybackSDL2 {
            audio_device
        };

        player.audio_device.resume();
        Ok(player)
    }
}