
#[derive(Debug, Clone, PartialEq)]
pub enum FrameState {
    Rendering,
    Completed
}

#[derive(Debug, Clone)]
pub struct NesFrame {
    pixels: Vec<u8>,
    width: usize,
    #[allow(dead_code)]
    height: usize,
    counter: u32,
    state: FrameState
}

impl NesFrame {

    pub fn new(width: usize, height: usize) -> Self {
        NesFrame {
            pixels: vec![0xFF; width * height * 4],
            width,
            height,
            counter: 0,
            state: FrameState::Rendering
        }
    }

    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn count(&self) -> u32 {
        self.counter
    }

    pub fn set_pixel(&mut self, x: u8, y: u8, color: (u8, u8, u8)) {
        let index = (y as usize * 4 * self.width) + (x as usize * 4);

        self.pixels[index] = color.0;
        self.pixels[index + 1] = color.1;
        self.pixels[index + 2] = color.2;
        //self.pixels[index + 3] = 0xFF;
    }

    pub fn get_pixel(&self, x: u8, y: u8) -> (u8, u8, u8) {
        let index = (y as usize * 4 * self.width) + (x as usize * 4);

        (self.pixels[index], self.pixels[index + 1], self.pixels[index + 2])
    }
    
    pub fn state(&self) -> FrameState {
        self.state.clone()
    }

    pub fn reset(&mut self) {
        self.state = FrameState::Rendering;
    }
    
    pub fn finish(&mut self) -> u32 {
        self.state = FrameState::Completed;
        self.counter += 1;
        self.counter
    }
}