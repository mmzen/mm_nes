use eframe::egui::{Color32, ColorImage};
use font8x8::UnicodeFonts;

pub struct Test8x8Generator;

impl Test8x8Generator {
    fn put_px(image: &mut ColorImage, x: usize, y: usize, color: Color32) {
        let width = image.size[0];
        let height = image.size[1];

        if x < width && y < height {
            image.pixels[y * width + x] = color;
        }
    }

    fn draw_character(image: &mut ColorImage, x: usize, y: usize, character: char, foreground: Color32, background: Color32, scale: usize) {
        if let Some(glyph) = font8x8::BASIC_FONTS.get(character) {
            for (row, bits) in glyph.iter().enumerate() {
                for col in 0..8usize {
                    let on = (bits >> col) & 1 != 0;
                    let color = if on { foreground } else { background };
                    for scaled_y in 0..scale {
                        for scaled_x in 0..scale {
                            Test8x8Generator::put_px(image, x + col*scale + scaled_x, y + row*scale + scaled_y, color);
                        }
                    }
                }
            }
        }
    }

    pub fn draw_text_centered(image: &mut ColorImage, text: &str, foreground: Color32, background: Color32) {
        // clear background
        //image.filled(background);

        let width = image.size[0];
        let height = image.size[1];

        let chars = text.chars().count().max(1);
        let mut scale = (width / (chars * 8)).max(1);
        scale = scale.min((height / 8).max(1)).max(1);

        let char_width = 8 * scale;
        let char_height = 8 * scale;
        let text_width = chars * char_width;

        let x0 = width.saturating_sub(text_width) / 2;
        let y0 = height.saturating_sub(char_height) / 2;

        let border = Color32::from_rgb(0x20, 0x20, 0x20);

        for x in 0..width {
            Test8x8Generator::put_px(image, x, 0, border);
            Test8x8Generator::put_px(image, x, height - 1, border);
        }
        for y in 0..height {
            Test8x8Generator::put_px(image, 0, y, border);
            Test8x8Generator::put_px(image, width - 1, y, border);
        }

        for (i, ch) in text.chars().enumerate() {
            let x = x0 + i * char_width;
            Test8x8Generator::draw_character(image, x, y0, ch, foreground, background, scale);
        }
    }
}