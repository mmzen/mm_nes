

const WIDTH: usize = 256;
const HEIGHT: usize = 240;

pub struct Frame {
    pub pixels: [u8; WIDTH * HEIGHT * 3]
}

impl Frame {

    pub fn new() -> Self {
        Frame {
            pixels: [0; WIDTH * HEIGHT * 3]
        }
    }

    pub fn set_pixel(&mut self, x: usize, y: usize, color: (u8, u8, u8)) {
        let index = (y * 3 * WIDTH) + (x * 3);

        self.pixels[index] = color.0;
        self.pixels[index + 1] = color.1;
        self.pixels[index + 2] = color.2;
    }
}