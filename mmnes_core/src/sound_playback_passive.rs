use crate::sound_playback::SoundPlayback;

const BUFFER_SIZE: usize = 1024;

#[derive(Debug)]
pub struct SoundPlaybackPassive {
    buffer: Vec<f32>,
}

impl SoundPlayback for SoundPlaybackPassive {
    fn push_sample(&mut self, sample: f32) {
        if self.buffer.len() < BUFFER_SIZE {
            self.buffer.push(sample);
        }
    }

    fn samples(&mut self) -> Vec<f32> {
        let batch = self.buffer.clone();
        self.buffer.clear();
        batch
    }

    fn resume(&self) {
        unreachable!()
    }
}

impl SoundPlaybackPassive {
    pub fn new() -> Self {
        SoundPlaybackPassive {
            buffer: Vec::with_capacity(BUFFER_SIZE)
        }
    }
}