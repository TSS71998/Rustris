use macroquad::miniquad::{UniformDesc, UniformType};
use macroquad::prelude::*;

const GLYPH_W: usize = 12;
const GLYPH_H: usize = 20;
const ATLAS_COLS: usize = 16;

const RAMP_CHARS: &[u8] =
    b".,:;'-~=+*!?/\\|()<>[]{}icrstvxzlunojfaeyw1237IJLTYCZVXUSOQ0456890kbdpqghmADEFHKNPRGB&%#$@MW";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Full,
    Plain,
    Raw
}

impl Mode {
    pub fn next(self) -> Mode {
        match self {
            Mode::Full => Mode::Plain,
            Mode::Plain => Mode::Raw,
            Mode::Raw => Mode::Full
        }
    }
}

pub struct Ascii {
    pub cols: usize,
    pub rows: usize,
    pub mode: Mode,
    material: Material,
    atlas: Texture2D,
    ramp: Texture2D,
    ramp_len: usize,
    layer: Image,
    layer_tex: Texture2D,
    scene: Option<(RenderTarget, u32, u32)>
}

impl Ascii {
    pub fn new(cols: usize, rows: usize) -> Self {
        let png = include_bytes!("../assets/glyphs.png");
        let atlas_img =
            Image::from_file_with_format(png, Some(ImageFormat::Png)).expect("bad glyph atlas");
        let atlas = Texture2D::from_image(&atlas_img);
        atlas.set_filter(FilterMode::Linear);

        let ink = |ch: u8| -> u32 {
            let slot = (ch - 32) as usize;
            let (cx, cy) = ((slot % ATLAS_COLS) * GLYPH_W, (slot / ATLAS_COLS) * GLYPH_H);
            let w = atlas_img.width as usize;
            let mut sum = 0;
            for y in cy..cy + GLYPH_H {
                for x in cx..cx + GLYPH_W {
                    sum += atlas_img.bytes[(y * w + x) * 4 + 3] as u32;
                }
            }
            sum
        };

        let mut chars = RAMP_CHARS.to_vec();
        chars.sort_by_key(|&c| ink(c));
        let mut ramp_img = Image::gen_image_color(chars.len() as u16, 1, BLANK);
        for (i, &c) in chars.iter().enumerate() {
            ramp_img.bytes[i * 4 + 3] = c - 32;
        }
        let ramp = Texture2D::from_image(&ramp_img);
        ramp.set_filter(FilterMode::Nearest);

        let layer = Image::gen_image_color(cols as u16, rows as u16, BLANK);
        let layer_tex = Texture2D::from_image(&layer);
        layer_tex.set_filter(FilterMode::Nearest);

        let uniforms = [
            ("resolution", UniformType::Float2),
            ("cell", UniformType::Float2),
            ("origin", UniformType::Float2),
            ("grid", UniformType::Float2),
            ("time", UniformType::Float1),
            ("flash", UniformType::Float1),
            ("mode", UniformType::Float1),
            ("ramp_len", UniformType::Float1),
            ("flip", UniformType::Float1),
        ]
        .into_iter()
        .map(|(name, ty)| UniformDesc::new(name, ty))
        .collect();
        let material = load_material(
            ShaderSource::Glsl { 
                vertex: VERTEX, 
                fragment: FRAGMENT 
            }, 
            MaterialParams {
                uniforms,
                textures: vec!["atlas".into(), "layer".into(), "ramp".into()],
                ..Default::default()
            }
        )
        .expect("ASCII shader failed to compile");

        Ascii {
            cols,
            rows,
            mode: Mode::Full,
            material,
            atlas,
            ramp,
            ramp_len: chars.len(),
            layer,
            layer_tex,
            scene: None
        }
    }

    pub fn begin_scene(&mut self) {
        let (w, h) = (
            screen_width().max(1.0) as u32,
            screen_height().max(1.0) as u32,
        );
        if !matches!(self.scene, Some((_, sw, sh)) if sw == w && sh == h) {
            let rt = render_target(w, h);
            rt.texture.set_filter(FilterMode::Nearest);
            self.scene = Some((rt, w, h));
        }
        let (rt, _, _) = self.scene.as_ref().unwrap();
        let mut cam = Camera2D::from_display_rect(Rect::new(0.0, 0.0, w as f32, h as f32));
        cam.render_target = Some(rt.clone());
        set_camera(&cam);
        clear_background(Color::new(0.0, 0.0, 0.0, 0.0));

        for b in self.layer.bytes.iter_mut() {
            *b = 0;
        }
    }

    pub fn put(&mut self, col: i32, row: i32, ch: u8, rgb: [f32; 3]) {
        if col < 0
            || row < 0
            || col >= self.cols as i32
            || row >= self.rows as i32
            || ch <= 32
            || ch > 126
        {
            return;
        }
        let i = (row as usize * self.cols + col as usize) * 4;
        let to8 = |v: f32| (v.clamp(0.0, 1.0) * 255.0) as u8;
        self.layer.bytes[i..i + 4].copy_from_slice(&[to8(rgb[0]), to8(rgb[1]), to8(rgb[2]), ch]);
    }

    pub fn text(&mut self, col: i32, row: i32, s: &str, rgb: [f32; 3]) {
        for (i, b) in s.bytes().enumerate() {
            self.put(col + i as i32, row, b, rgb);
        }
    }

    pub fn present(&mut self, origin: Vec2, cell: Vec2, time: f32, flash: f32) {
        set_default_camera();
        self.layer_tex.update(&self.layer);
        let Some((rt, w, h)) = &self.scene else {
            return;
        };
        let m = &self.material;
        m.set_uniform("resolution", vec2(*w as f32, *h as f32));
        m.set_uniform("cell", cell);
        m.set_uniform("origin", origin);
        m.set_uniform("grid", vec2(self.cols as f32, self.rows as f32));
        m.set_uniform("time", time);
        m.set_uniform("flash", flash);
        m.set_uniform(
            "mode",
            match self.mode {
                Mode::Full => 0.0f32,
                Mode::Plain => 1.0,
                Mode::Raw => 2.0,
            },
        );
        m.set_uniform("ramp_len", self.ramp_len as f32);
        // Render targets come out upside down relative to the screen.
        m.set_uniform("flip", 1.0f32);
        m.set_texture("atlas", self.atlas.clone());
        m.set_texture("layer", self.layer_tex.clone());
        m.set_texture("ramp", self.ramp.clone());
        gl_use_material(m);
        draw_texture_ex(
            &rt.texture,
            0.0,
            0.0,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(*w as f32, *h as f32)),
                ..Default::default()
            },
        );
        gl_use_default_material();
    }
}

const VERTEX: &str = include_str!("shaders/ascii.vert");

const FRAGMENT: &str = include_str!("shaders/ascii.frag");