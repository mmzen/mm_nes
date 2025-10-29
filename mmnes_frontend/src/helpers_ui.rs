use eframe::egui::{Color32, ColorImage, RichText};
use image::codecs::jpeg::JpegEncoder;
use image::{ExtendedColorType, ImageError};

pub struct HelpersUI;

impl HelpersUI {
    pub fn monospace(s: &str) -> RichText {
        RichText::new(s).monospace()
    }

    pub fn header(s: &str) -> RichText {
        RichText::new(s).monospace().strong()
    }

    pub fn color_image_to_jpeg_bytes(image: &ColorImage, quality: u8, background: [u8; 3]) -> Result<Vec<u8>, ImageError> {
        let (width, height) = (image.size[0], image.size[1]);

        let mut rgb = Vec::with_capacity(width * height * 3);
        
        for pixel in &image.pixels {
            let a = pixel.a() as u32;
            let inv_a = 255 - a;
            let r = ((pixel.r() as u32 * a + background[0] as u32 * inv_a) / 255) as u8;
            let g = ((pixel.g() as u32 * a + background[1] as u32 * inv_a) / 255) as u8;
            let b = ((pixel.b() as u32 * a + background[2] as u32 * inv_a) / 255) as u8;
            rgb.extend_from_slice(&[r, g, b]);
        }
        
        let mut out = Vec::new();
        let mut encoder = JpegEncoder::new_with_quality(&mut out, quality.clamp(1, 100));
        encoder.encode(&rgb, width as u32, height as u32, ExtendedColorType::Rgb8)?;
        
        Ok(out)
    }

    pub(crate) fn create_default_texture(width: usize, height: usize, color: Color32) -> Vec<Color32> {
        let mut vec = Vec::<Color32>::with_capacity(width * height);

        for _ in 0..width * height {
            vec.push(color);
        }

        vec
    }
}