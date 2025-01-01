use std::fmt::Debug;

pub trait SoundPlayback : Debug {
    fn playback(&self);
}

#[derive(Debug, PartialEq)]
pub enum SoundPlaybackError {
    SoundPlaybackFailure(String)
}