use sdl2::Sdl;
use crate::sound_playback::{SoundPlayback, SoundPlaybackError};

#[derive(Debug)]
pub struct SoundPlaybackSDL2 {
}

impl SoundPlayback for SoundPlaybackSDL2 {
    fn playback(&self) {
    }
}

impl SoundPlaybackSDL2 {
    pub fn new(sdl_context: &Sdl) -> Result<Self, SoundPlaybackError> {
        let player = SoundPlaybackSDL2 {};
        Ok(player)
    }
}