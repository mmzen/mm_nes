use eframe::egui::{Color32, ColorImage};
use font8x8::UnicodeFonts;

pub struct Test8x8Generator;
const FONT_SCALE: usize = 2;

impl Test8x8Generator {

    fn set_pixel(image: &mut ColorImage, x: usize, y: usize, color: Color32) {
        let [image_width, image_height] = image.size;
        if x < image_width && y < image_height {
            image.pixels[y * image_width + x] = color;
        }
    }

    fn draw_glyph(image: &mut ColorImage, x: usize, y: usize, ch: char, color: Color32) {
        let glyph = font8x8::BASIC_FONTS.get(ch).or_else(|| font8x8::BASIC_FONTS.get('?'));
        if let Some(bitmap) = glyph {
            for (row, bits) in bitmap.iter().enumerate() {
                for col in 0..8 {
                    if (bits >> col) & 1u8 != 0u8 {
                        for sy in 0..FONT_SCALE {
                            for sx in 0..FONT_SCALE {
                                Test8x8Generator::set_pixel(
                                    image,
                                    x + col as usize * FONT_SCALE + sx,
                                    y + row * FONT_SCALE + sy,
                                    color,
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    // ---------- wrapping ----------
    fn wrap_text_to_lines(text: &str, max_chars_per_line: usize) -> Vec<String> {
        let mut lines = Vec::new();

        for paragraph in text.split('\n') {
            if paragraph.is_empty() {
                lines.push(String::new());
                continue;
            }

            let mut current = String::new();
            for word in paragraph.split_whitespace() {
                let word_len = word.chars().count();
                let cur_len = current.chars().count();

                if cur_len == 0 {
                    // start of line
                    if word_len <= max_chars_per_line {
                        current.push_str(word);
                    } else {
                        // hard wrap long word
                        for ch in word.chars() {
                            if current.chars().count() == max_chars_per_line {
                                lines.push(std::mem::take(&mut current));
                            }
                            current.push(ch);
                        }
                    }
                } else if cur_len + 1 + word_len <= max_chars_per_line {
                    current.push(' ');
                    current.push_str(word);
                } else {
                    lines.push(std::mem::take(&mut current));
                    // place word on new line (may still need hard-wrapping)
                    for ch in word.chars() {
                        if current.chars().count() == max_chars_per_line {
                            lines.push(std::mem::take(&mut current));
                        }
                        current.push(ch);
                    }
                }
            }
            if !current.is_empty() {
                lines.push(current);
            }
        }

        lines
    }

    fn truncate_lines_to_height(mut lines: Vec<String>, max_lines: usize, max_chars_per_line: usize) -> Vec<String> {
        if lines.len() <= max_lines { return lines; }
        lines.truncate(max_lines);
        // ensure last line ends with "..." and fits
        let last = lines.last_mut().unwrap();
        let mut chars: Vec<char> = last.chars().collect();
        let need = 3usize;
        if chars.len() >= need {
            chars.truncate(max_chars_per_line.saturating_sub(need));
        }
        let mut s = chars.into_iter().collect::<String>();
        while s.chars().count() + need > max_chars_per_line {
            s.pop();
            if s.is_empty() { break; }
        }
        s.push_str("...");
        *last = s;
        lines
    }

    pub fn draw_text_wrapped_centered(image: &mut ColorImage, text: &str, foreground: Color32) {
        let [image_width, image_height] = image.size;
        let char_width = 8 * FONT_SCALE;
        let char_height = 8 * FONT_SCALE;

        let max_chars_per_line = (image_width / char_width).max(1);
        let max_lines = (image_height / char_height).max(1);

        let mut lines = Test8x8Generator::wrap_text_to_lines(text, max_chars_per_line);
        if lines.len() > max_lines {
            lines = Test8x8Generator::truncate_lines_to_height(lines, max_lines, max_chars_per_line);
        }

        let text_block_width = lines
            .iter()
            .map(|line| line.chars().count() * char_width)
            .max()
            .unwrap_or(0);
        let text_block_height = lines.len() * char_height;

        let start_x = image_width.saturating_sub(text_block_width) / 2;
        let mut y = image_height.saturating_sub(text_block_height) / 2;

        for line in &lines {
            let line_width = line.chars().count() * char_width;
            let mut x = start_x + (text_block_width.saturating_sub(line_width) / 2);
            for ch in line.chars() {
                Test8x8Generator::draw_glyph(image, x, y, ch, foreground);
                x += char_width;
            }
            y += char_height;
        }
    }
}