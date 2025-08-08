use log::{debug, info};
use std::fmt::{Debug, Formatter};
use sdl2::Sdl;
use sdl2::audio::{AudioQueue, AudioSpecDesired};

use crate::sound_playback::{SoundPlayback, SoundPlaybackError};

const SAMPLE_RATE: i32 = 44_100;
const CHUNK_SIZE: u16 = 1024;
const BATCH_SAMPLES: usize = 1024;

pub struct SoundPlaybackSDL2Queue {
    audio_queue: AudioQueue<f32>,
    batch: Vec<f32>
}

impl Debug for SoundPlaybackSDL2Queue {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SoundPlaybackSDL2Queue").finish()
    }
}

impl SoundPlayback for SoundPlaybackSDL2Queue {
    fn push_sample(&mut self, sample: f32) {
        let safe_samples = sample.clamp(-1.0, 1.0);
        self.batch.push(safe_samples);

        if self.batch.len() >= BATCH_SAMPLES {
            self.audio_queue.queue_audio(&self.batch).unwrap();
            self.batch.clear();
        }
    }

    fn resume(&self) {
        self.audio_queue.resume();
    }
}

impl SoundPlaybackSDL2Queue {
    pub fn new(sdl_context: &Sdl) -> Result<Self, SoundPlaybackError> {
        info!("initializing audio system (queue): buffer size: chunk size: {} samples ...",
            CHUNK_SIZE);

        let audio_subsystem = sdl_context.audio().unwrap();

        let desired_spec = AudioSpecDesired {
            freq: Some(SAMPLE_RATE),
            channels: Some(1),
            samples: Some(CHUNK_SIZE),
        };

        let mut player = SoundPlaybackSDL2Queue {
            audio_queue: audio_subsystem.open_queue(None, &desired_spec).unwrap(),
            batch: Vec::with_capacity(BATCH_SAMPLES)
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