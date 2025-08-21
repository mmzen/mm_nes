use std::fmt::Debug;

pub trait SoundPlayback : Debug {
    fn push_sample(&mut self, sample: f32);
    fn resume(&self);
}

#[derive(Debug, PartialEq)]
pub enum SoundPlaybackError {
    SoundPlaybackFailure(String)
}