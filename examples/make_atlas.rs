use std::fs::File;
use std::io::BufWriter;

const GLYPH_W: usize = 12;
const GLYPH_H: usize = 20;
const COLS: usize = 16;
const ROWS: usize = 6;
const FONT_PX: f32 = 17.0;

fn main() {
    let font_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf".into());

    let bytes =
        std::fs::read(&font_path).unwrap_or_else(|e| panic!("can't read font {font_path}: {e}"));

    let font = fontdue::Font::from_bytes(bytes, fontdue::FontSettings::default())
        .expect("not a valid TTF/OTF font");

    let line = font
        .horizontal_line_metrics(FONT_PX)
        .expect("font as no horizontal metrics");

    let baseline = ((GLYPH_H as f32 - (line.ascent - line.descent)) / 2.0 + line.ascent).round();

    let (w, h) = (COLS * GLYPH_W, ROWS * GLYPH_H);
    let mut rgba = vec![0u8; w * h * 4];
    for code in 32u8..127 {
        let slot = (code - 32) as usize;
        let (cx, cy) = ((slot % COLS) * GLYPH_W, (slot / COLS) * GLYPH_H);
        let (m, bitmap) = font.rasterize(code as char, FONT_PX);
        let left = ((GLYPH_W as f32 - m.advance_width) / 2.0).round() as i32 + m.xmin;
        let top = baseline as i32 - m.ymin - m.height as i32;
        for gy in 0..m.height {
            for gx in 0..m.width {
                let (x, y) = (left + gx as i32, top + gy as i32);
                if x < 0 || y < 0 || x >= GLYPH_W as i32 || y >= GLYPH_H as i32 {
                    continue;
                }
                let i = ((cy + y as usize) * w + cx + x as usize) * 4;
                rgba[i..i + 3].copy_from_slice(&[255, 255, 255]);
                rgba[i + 3] = bitmap[gy * m.width + gx];
            }
        }
    }

    std::fs::create_dir_all("assets").unwrap();
    let out = File::create("assets/glyphs.png").expect("can't create assets/glyphs.png");
    let mut enc = png::Encoder::new(BufWriter::new(out), w as u32, h as u32);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()
        .and_then(|mut wr| wr.write_image_data(&rgba))
        .expect("failed to write PNG");
    println!("wrote assets/glyph.png ({w}x{h}) from {font_path}");
}