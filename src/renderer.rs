use sdl2::pixels::PixelFormatEnum;
use sdl2::render::{Canvas, Texture, TextureCreator};
use sdl2::video::{Window, WindowContext};
use crate::frame::Frame;

const WIDTH: usize = 256;
const HEIGHT: usize = 240;

#[cfg(not(test))]
pub struct Renderer {
    canvas: Canvas<Window>,
    texture_creator: TextureCreator<WindowContext>,
    texture: Texture,
    frame: Frame
}

#[cfg(not(test))]
impl Renderer {
    pub fn new() -> Self {
        let sdl_context = sdl2::init().unwrap();
        let video_subsystem = sdl_context.video().unwrap();
        let window = video_subsystem
            .window("NES", (WIDTH * 3) as u32, (HEIGHT *3) as u32)
            .position_centered()
            .build()
            .unwrap();

        let mut canvas = window.into_canvas().present_vsync().build().unwrap();
        canvas.set_scale(3.0, 3.0).unwrap();

        let texture_creator = canvas.texture_creator();
        let texture = texture_creator
            .create_texture_target(PixelFormatEnum::RGB24, WIDTH as u32, HEIGHT as u32)
            .unwrap();

        Renderer {
            canvas,
            texture_creator,
            texture,
            frame: Frame::new(WIDTH, HEIGHT)
        }
    }

    fn create_texture(&self) -> Texture {
        self.texture_creator
            .create_texture_target(PixelFormatEnum::RGB24, WIDTH as u32, HEIGHT as u32)
            .unwrap()
    }

    pub fn frame(&mut self) -> &mut Frame {
        &mut self.frame
    }

    pub fn update(&mut self) {
        self.texture.update(None, &self.frame.pixels, WIDTH * 3).unwrap();
        self.canvas.copy(&self.texture, None, None).unwrap();
        self.canvas.present();
    }
}

#[cfg(test)]
pub struct Renderer {
    frame: Frame
}

#[cfg(test)]
impl Renderer {
    pub fn new() -> Self {
        Renderer {
            frame: Frame::new(WIDTH, HEIGHT)
        }
    }

    pub fn frame(&mut self) -> &mut Frame {
        &mut self.frame
    }

    pub fn update(&mut self) {
    }
}