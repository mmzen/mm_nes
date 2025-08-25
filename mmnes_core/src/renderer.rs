use crate::nes_frame::NesFrame;

const WIDTH: usize = 256;
const HEIGHT: usize = 240;

#[cfg(not(test))]
pub struct Renderer {
    frame: NesFrame
}

#[cfg(not(test))]
impl Renderer {
    pub fn new() -> Self {
        Renderer {
            frame: NesFrame::new(WIDTH, HEIGHT)
        }
    }

    pub fn frame_as_mut(&mut self) -> &mut NesFrame {
        &mut self.frame
    }

    pub fn frame(&self) -> &NesFrame {
        &self.frame
    }

    pub fn update(&mut self) {
        self.frame.finish();
    }
    
    pub fn reset(&mut self) {
        self.frame.reset();
    }
}

#[cfg(test)]
pub struct Renderer {
    pub frame: NesFrame
}

#[cfg(test)]
impl Renderer {
    pub fn new() -> Self {
        Renderer {
            frame: NesFrame::new(WIDTH, HEIGHT)
        }
    }

    pub fn frame_as_mut(&mut self) -> &mut NesFrame {
        &mut self.frame
    }

    pub fn frame(&self) -> &NesFrame {
        &self.frame
    }

    pub fn update(&mut self) {
    }

    pub fn reset(&mut self) {
    }
}