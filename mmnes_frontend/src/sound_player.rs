use std::fmt::{Display, Formatter};
use log::info;
use sdl2::audio::{AudioQueue, AudioSpecDesired};
use sdl2::Sdl;

const SAMPLE_RATE: i32 = 44_100;
const CHUNK_SIZE: u16 = 1024;
const BATCH_SAMPLES: usize = 1024;

pub enum SoundPlayerError {
    SdlFailure(String),
}

impl Display for SoundPlayerError {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            SoundPlayerError::SdlFailure(s) => write!(f, "SDL error: {}", s),
        }
    }
}

pub struct SoundPlayer {
    #[allow(dead_code)]
    sdl: Sdl,
    audio_queue: AudioQueue<f32>,
    batch_buffer: Vec<f32>
}

impl SoundPlayer {

    pub fn push_sample(&mut self, sample: f32) {
        let normalized_samples = sample.clamp(-1.0, 1.0);
        self.batch_buffer.push(normalized_samples);

        if self.batch_buffer.len() >= BATCH_SAMPLES {
            self.audio_queue.queue_audio(&self.batch_buffer).unwrap();
            self.batch_buffer.clear();
        }
    }

    pub fn resume(&mut self) {
        self.audio_queue.resume();
    }

    #[allow(dead_code)]
    pub fn pause(&mut self) {
        self.audio_queue.pause();
    }

    fn init_sdl() -> Result<Sdl, SoundPlayerError> {
        let sdl = sdl2::init().map_err(
            |s| SoundPlayerError::SdlFailure(s)
        )?;

        Ok(sdl)
    }

    fn initialize() -> Result<Self, SoundPlayerError> {
        let sdl = SoundPlayer::init_sdl()?;
        info!("initializing audio system (queue): buffer size: chunk size: {} samples ...", CHUNK_SIZE);

        let audio_subsystem = if let Ok(audio) = sdl.audio() {
            audio
        } else {
            return Err(SoundPlayerError::SdlFailure("failed to initialize audio subsystem".to_string()));
        };

        let desired_spec = AudioSpecDesired {
            freq: Some(SAMPLE_RATE),
            channels: Some(1),
            samples: Some(CHUNK_SIZE),
        };

        let audio_queue: AudioQueue<f32> = if let Ok(queue) = audio_subsystem.open_queue(None, &desired_spec) {
            queue
        } else {
            return Err(SoundPlayerError::SdlFailure("failed to open audio queue".to_string()));
        };

        let player = SoundPlayer {
            sdl,
            audio_queue,
            batch_buffer: Vec::with_capacity(BATCH_SAMPLES)
        };

        Ok(player)
    }

    pub fn new() -> Result<Self, SoundPlayerError> {
        let mut player = SoundPlayer::initialize()?;
        player.resume();

        Ok(player)
    }
}