// Authorship: Human 100% | Claude 0%
use std::fmt::Debug;

pub trait SoundPlayback : Debug {
    fn push_sample(&mut self, sample: f32);
    fn samples(&mut self) -> Vec<f32>;
    fn resume(&self);
    fn clear(&mut self);
}

#[derive(Debug, PartialEq)]
pub enum SoundPlaybackError {
    SoundPlaybackFailure(String)
}