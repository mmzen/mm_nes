

pub struct Frame {
    pixels: Vec<u8>,
    width: usize,
    #[allow(dead_code)]
    height: usize,
}

impl Frame {

    pub fn new(width: usize, height: usize) -> Self {
        Frame {
            pixels: vec![0; width * height * 3],
            width,
            height
        }
    }

    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    pub fn set_pixel(&mut self, x: u8, y: u8, color: (u8, u8, u8)) {
        let index = (y as usize * 3 * self.width) + (x as usize * 3);

        self.pixels[index] = color.0;
        self.pixels[index + 1] = color.1;
        self.pixels[index + 2] = color.2;
    }

    pub fn get_pixel(&self, x: u8, y: u8) -> (u8, u8, u8) {
        let index = (y as usize * 3 * self.width) + (x as usize * 3);

        (self.pixels[index], self.pixels[index + 1], self.pixels[index + 2])
    }
}