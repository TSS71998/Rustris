use macroquad::prelude::*;

use crate::ascii::Ascii;
use rustris::bigfont;
use rustris::effects::{BoardGeom, CELL_ASPECT, Effects, Label, Rgb, piece_rgb};
use rustris::game::{Game, HEIGHT, PieceKind, WIDTH};

pub const CW: f32 = 6.0;
pub const CH: f32 = 10.0;
pub const BW: i32 = 5;
pub const BH: i32 = 3;
pub const BOARD_W: i32 = WIDTH as i32 * BW;
pub const BOARD_H: i32 = HEIGHT as i32 * BH;

pub const FRAME: Rgb = [0.28, 0.36, 0.38];
pub const LABEL: Rgb = [0.45, 0.56, 0.56];
pub const TEXT: Rgb = [0.82, 0.94, 0.88];
pub const DIM: Rgb = [0.38, 0.46, 0.47];
const DENSE: &[u8] = b"####@%#";

pub fn geom(x: i32, y: i32) -> BoardGeom {
    BoardGeom {
        x: x as f32,
        y: y as f32,
        cell_w: BW as f32,
        cell_h: BH as f32
    }
}

pub fn scale(c: Rgb, f: f32) -> Rgb {
    [c[0] * f, c[1] * f, c[2] * f]
}

pub fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t
    ]
}

pub fn color(c: Rgb, a: f32) -> Color {
    Color::new(c[0], c[1], c[2], a)
}

pub fn hash(x: i32, y: i32, z: i32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8da6_b343)
        ^ (y as u32).wrapping_mul(0xd816_3841)
        ^ (z as u32).wrapping_mul(0xcb1a_b31f);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1_e995);
    h ^= h >> 15;
    (h & 0xffff) as f32 / 65535.0
}

pub struct View {
    scale: f32,
    ox: f32,
    oy: f32
}

impl View {
    pub fn fit(cols: usize, rows: usize, shake: Vec2) -> Self {
        let (dw, dh) = (cols as f32 * CW, rows as f32 * CH);
        let scale = (screen_width() / dw).min(screen_height() / dh);
        View {
            scale,
            ox: ((screen_width() - dw * scale) / 2.0 + shake.x * scale).round(),
            oy: ((screen_height() - dh * scale) / 2.0 + shake.y * scale).round(),
        }
    }

    pub fn shake(strength: f32) -> Vec2 {
        vec2(
            rand::gen_range(-1.0, 1.0) * strength * 7.0,
            rand::gen_range(-1.0, 1.0) * strength * 7.0,
        )
    }

    fn px(&self, col: f32, row: f32) -> (f32, f32) {
        (
            self.ox + col * CW * self.scale,
            self.oy + row * CH * self.scale,
        )
    }

    pub fn cell(&self) -> Vec2 {
        vec2(CW * self.scale, CH * self.scale)
    }

    pub fn origin(&self) -> Vec2 {
        vec2(self.ox, self.oy)
    }

    fn rect(&self, col: f32, row: f32, w: f32, h: f32, c: Color) {
        let (x, y) = self.px(col, row);
        draw_rectangle(x, y, w * CW * self.scale, h * CH * self.scale, c);
    }

    pub fn block(&self, col: f32, row: f32, w: f32, h: f32, rgb: Rgb, bright: f32) {
        self.rect(col, row, w, h, color(scale(rgb, 0.14 * bright), 1.0));
        self.rect(col, row, w - 1.0, 1.0, color(scale(rgb, 1.05 * bright), 1.0));
    }

    pub fn backing(&self, col: f32, row: f32, w: f32, h: f32, a: f32) {
        self.rect(col, row, w, h, Color::new(0.008, 0.012, 0.016, a));
    }

    pub fn panel(&self, x0: i32, y0: i32, x1: i32, y1: i32) {
        self.rect(
            x0 as f32,
            y0 as f32,
            (x1 - x0 + 1) as f32,
            (y1 - y0 + 1) as f32,
            Color::new(0.02, 0.025, 0.03, 1.0),
        );
    }

    fn glow(&self, col: f32, row: f32, radius: f32, rgb: Rgb, a: f32) {
        let (x, y) = self.px(col, row);
        draw_circle(x, y, radius * CW * self.scale, color(rgb, a));
    }
}

pub fn frame(a: &mut Ascii, x0: i32, y0: i32, x1: i32, y1: i32, title: Option<&str>) {
    for x in x0..=x1 {
        a.put(x, y0, b'-', FRAME);
        a.put(x, y1, b'-', FRAME);
    }
    for y in y0..=y1 {
        a.put(x0, y, b'|', FRAME);
        a.put(x1, y, b'|', FRAME);
    }
    for (x, y) in [(x0, y0), (x1, y0), (x0, y1), (x1, y1)] {
        a.put(x, y, b'+', FRAME);
    }
    if let Some(t) = title {
        a.text(x0 + 2, y0, &format!("[ {t} ]"), LABEL);
    }
}

pub fn text_centered(a: &mut Ascii, cx: f32, row: i32, s: &str, rgb: Rgb) {
    a.text((cx - s.len() as f32 / 2.0).round() as i32, row, s, rgb);
}

#[allow(clippy::too_many_arguments)]
pub fn banner(
    a: &mut Ascii,
    v: &View,
    back: f32,
    text: &str,
    cx: f32,
    top: f32,
    max_w: f32,
    time: f32,
    enter: f32,
    exit: f32,
    tint: impl Fn(f32, f32) -> Rgb,
) {
    let mut lines: Vec<String> = Vec::new();
    for word in text.split(' ') {
        match lines.last_mut() {
            Some(l) if bigfont::width(&format!("{l} {word}"), Label::SCALE_X) as f32 <= max_w => {
                l.push(' ');
                l.push_str(word);
            }
            _ => lines.push(word.to_string()),
        }
    }
    let e = 1.0 - (1.0 - enter.clamp(0.0, 1.0)).powi(3);
    let tick = (time * 8.0) as i32;
    for (li, line) in lines.iter().enumerate() {
        let w = bigfont::width(line, Label::SCALE_X) as f32;
        let x0 = (cx - w / 2.0).round() as i32;
        let y0 = (top + li as f32 * (bigfont::HEIGHT as f32 + 1.0)).round() as i32;
        if back > 0.0 {
            let h = bigfont::HEIGHT as f32 + 2.0;
            v.backing(x0 as f32 - 2.0, y0 as f32 - 1.0, w + 4.0, h, back);
        }
        bigfont::for_each_pixel(line, Label::SCALE_X, |x, y| {
            let (x, y) = (x as i32, y as i32);
            let seed = li as i32 * 97;
            if hash(x, y, seed + 1) < exit {
                return;
            }
            let dx = ((hash(x, y, seed + 2) - 0.5) * 40.0 * (1.0 - e)).round() as i32;
            let dy = ((hash(x, y, seed + 3) - 0.5) * 16.0 * (1.0 - e) - exit * 3.0).round() as i32;
            // characters flicker now and then
            let flick = if hash(x, y, tick) < 0.08 { tick } else { 0 };
            let ch =
                DENSE[(hash(x, y, seed + 4 + flick) * DENSE.len() as f32) as usize % DENSE.len()];
            let shade = 0.82 + 0.18 * (time * 5.0 + x as f32 * 0.35).sin();
            let c = tint(x as f32 / w.max(1.0), y as f32 / bigfont::HEIGHT as f32);
            a.put(x0 + x + dx, y0 + y + dy, ch, scale(c, shade));
        });
    }
}

#[allow(clippy::too_many_arguments)]
pub fn big_text(a: &mut Ascii, v: &View, text: &str, cx: f32, top: f32, k: i32, rgb: Rgb, time: f32, t: f32) {
    let k = (k as f32 * (2.0 - t.clamp(0.0, 1.0))).round().max(1.0) as i32;
    let w = bigfont::width(text, 1) as i32;
    let (cols, rows) = (w * 2 * k, bigfont::HEIGHT as i32 * k);
    let x0 = (cx - cols as f32 / 2.0).round() as i32;
    let y0 = (top - rows as f32 / 2.0).round() as i32;
    v.backing((x0 - 2) as f32, (y0 - 1) as f32, (cols + 4) as f32, (rows + 2) as f32, 0.9);
    bigfont::for_each_pixel(text, 1, |px, py| {
        for dy in 0..k {
            for dx in 0..2 * k {
                let (x, y) = (px as i32 * 2 * k + dx, py as i32 * k + dy);
                let ch = DENSE[(hash(x, y, (time * 6.0) as i32) * DENSE.len() as f32) as usize % DENSE.len()];
                let shade = 1.0 - 0.35 * dy as f32 / k as f32;
                a.put(x0 + x, y0 + y, ch, scale(rgb, shade));
            }
        }
    });
}

pub fn title_tint(time: f32) -> impl Fn(f32, f32) -> Rgb {
    move |x, y| {
        let t = (x + (time * 0.4).sin() * 0.3).clamp(0.0, 1.0);
        scale(mix([0.45, 0.95, 0.8], [0.55, 0.9, 0.45], t), 1.0 - 0.25 * y)
    }
}

fn mini_piece(v: &View, kind: PieceKind, cx: f32, top: f32) {
    let cells = kind.cells(0);
    let min_r = cells.iter().map(|c| c.0).min().unwrap();
    let max_r = cells.iter().map(|c| c.0).max().unwrap();
    let min_c = cells.iter().map(|c| c.1).min().unwrap();
    let max_c = cells.iter().map(|c| c.1).max().unwrap();

    let w = (max_c - min_c + 1) as f32 * 3.0;
    let h = (max_r - min_r + 1) as f32 * 2.0;
    let (ox, oy) = ((cx - w / 2.0).round(), (top + (4.0 - h) / 2.0).round());
    for (r, c) in cells {
        let (x, y) = (ox + (c - min_c) as f32 * 3.0, oy + (r -min_r) as f32 * 2.0);
        let rgb = piece_rgb(kind.id());
        v.rect(x, y, 3.0, 2.0, color(scale(rgb, 0.5), 1.0));
        v.rect(x, y, 3.0, 1.0, color(scale(rgb, 0.95), 1.0));
    }
}

pub fn hold_box(a: &mut Ascii, v: &View, game: &Game, x0: i32, x1: i32, y: i32) {
    frame(a, x0, y, x1, y + 9, Some("HOLD"));
    if let Some(kind) = game.hold {
        // greyed out while hold can't be used again
        mini_piece(v, kind, (x0 + x1 + 1) as f32 / 2.0, (y + 3) as f32);
        if game.hold_used {
            v.rect((x0 + 1) as f32, (y + 1) as f32, (x1 - x0 - 1) as f32, 8.0, Color::new(0.0, 0.0, 0.0, 0.55));
        }
    }
}

pub fn next_box(a: &mut Ascii, v: &View, game: &Game, x0: i32, x1: i32, y: i32, count: usize) {
    frame(a, x0, y, x1, y + count as i32 * 6 + 2, Some("NEXT"));
    let cx = (x0 + x1 + 1) as f32 / 2.0;
    for (i, &kind) in game.queue.iter().take(count).enumerate() {
        mini_piece(v, kind, cx, (y + 2 + i as i32 * 6) as f32);
    }
}

pub fn board(a: &mut Ascii, v: &View, game: &Game, bx: i32, by: i32, time: f32, over_for: Option<f32>) {
    v.rect(
        (bx - 1) as f32,
        (by - 1) as f32,
        (WIDTH as i32 * BW + 2) as f32,
        (HEIGHT as i32 * BH + 2) as f32,
        Color::new(0.01, 0.014, 0.018, 1.0),
    );

    for (r, row) in game.board.iter().enumerate() {
        for (c, &id) in row.iter().enumerate() {
            let (x, y) = ((bx + c as i32 * BW) as f32, (by + r as i32 * BH) as f32);
            let gone = over_for.is_some_and(|t| t > r as f32 * 0.035);
            if id != 0 && !gone {
                v.block(x, y, BW as f32, BH as f32, piece_rgb(id), 0.9);
            } else {
                a.put(x as i32 + 2, y as i32 + 1, b'.', [0.1, 0.14, 0.14]);
            }
        }
    }
    if !game.game_over {
        let p = game.current;
        let rgb = piece_rgb(p.kind.id());
        let ghost_dy = game.ghost_y() - p.y;
        for (r, c) in p.cells() {
            let gr = r + ghost_dy;
            if gr >= 0 {
                let (x, y) = ((bx + c * BW) as f32, (by + gr * BH) as f32);
                v.rect(x, y, BW as f32, BH as f32, color(scale(rgb, 0.13), 1.0));
            }
        }
        let pulse = if game.on_ground() {
            0.9 + 0.2 * (time * 14.0).sin()
        } else {
            1.1
        };
        for (r, c) in p.cells() {
            if r >= 0 {
                v.block(
                    (bx + c * BW) as f32,
                    (by + r * BH) as f32,
                    BW as f32,
                    BH as f32,
                    rgb,
                    pulse,
                );
            }
        }
    }
    frame(a, bx - 1, by - 1, bx + BOARD_W, by + BOARD_H, None);
}

pub fn fx_scene(v: &View, fx: &Effects) {
    for fl in &fx.flashes {
        v.rect(fl.x, fl.y, fl.w, fl.h, color(fl.color, fl.brightness()));
    }
    for s in &fx.sweeps {
        v.glow(s.x, s.y + s.h / 2.0, 3.0, s.color, 0.5);
    }
    for p in &fx.particles {
        if p.visible() && p.glow > 0.0 {
            let b = p.brightness() * p.glow;
            v.glow(p.x, p.y, 1.6, p.current_color(), 0.35 * b);
        }
    }
    for c in &fx.comets {
        let (x, y) = c.at(c.t());
        v.glow(x, y, 1.6 + c.lines as f32 * 0.3, c.color, 0.45);
    }
}

pub fn fx_glyphs(a: &mut Ascii, fx: &Effects) {
    for p in &fx.particles {
        if p.visible() {
            let c = scale(p.current_color(), 0.35 + 0.9 * p.brightness());
            a.put(p.x.floor() as i32, p.y.floor() as i32, p.glyph(), c);
        }
    }
    for s in &fx.sweeps {
        for j in 0..s.h as i32 {
            let (x, y) = (s.x.floor() as i32, s.y as i32 + j);
            a.put(x, y, b'#', [1.0, 1.0, 1.0]);
            a.put(x + s.dir as i32, y, b'|', s.color);
        }
    }
    for ring in &fx.rings {
        let b = ring.brightness();
        if b <= 0.0 {
            continue;
        }
        let rad = ring.radius();
        let reach = (rad + ring.thickness) as i32 + 1;
        let (cx, cy) = (ring.x, ring.y);
        for dy in -reach..=reach {
            for dx in -reach..=reach {
                let (x, y) = (cx.floor() as i32 + dx, cy.floor() as i32 + dy);
                let (fx_, fy) = (x as f32 + 0.5 - cx, (y as f32 + 0.5 - cy) * CELL_ASPECT);
                let d = (fx_ * fx_ + fy * fy).sqrt();
                let off = (d - rad).abs();
                if off > ring.thickness * CELL_ASPECT {
                    continue;
                }
                let tangent = fy.atan2(fx_).to_degrees() + 90.0;
                let a180 = tangent.rem_euclid(180.0);
                let ch = match a180 {
                    a if !(22.5..157.5).contains(&a) => b'-',
                    a if a < 67.5 => b'\\',
                    a if a < 112.5 => b'|',
                    _ => b'/',
                };
                let k = (1.0 - off / (ring.thickness * CELL_ASPECT)) * b;
                a.put(x, y, if k > 0.8 { b'#' } else { ch }, scale(ring.color, 0.3 + 0.9 * k));
            }
        }
    }

    for c in &fx.comets {
        let t = c.t();
        let trail = 10 + c.lines as i32 * 2;
        for i in (0..trail).rev() {
            let tt = t - i as f32 * 0.012;
            if tt < 0.0 {
                continue;
            }
            let (x, y) = c.at(tt);
            let k = 1.0 - i as f32 / trail as f32;
            let ch = match i {
                0 => b'@',
                1..=2 => b'O',
                3..=5 => b'o',
                _ => b'.',
            };
            a.put(x.floor() as i32, y.floor() as i32, ch, mix(c.color, [1.0; 3], k * 0.6));
        }
    }
}

pub fn fx_labels(a: &mut Ascii, v: &View, fx: &Effects, time: f32) {
    let g = fx.geom;
    for l in &fx.labels {
        let t = l.age / l.life;
        let (enter, exit) = (l.age / 0.18, ((t - 0.6) / 0.4).max(0.0));
        let back = 0.85 * enter.min(1.0) * (1.0 - exit);
        banner(a, v, back, l.text, g.center_x(), l.y, g.width() - 2.0, time, enter, exit, |_, y| {
            scale(l.color, 1.0 - 0.3 * y)
        });
    }
}

pub fn overlay(
    a: &mut Ascii,
    v: &View,
    bx: i32,
    by: i32,
    title: &str,
    tint: Rgb,
    lines: &[String],
    time: f32,
    t: f32
) {
    v.rect(bx as f32, by as f32, BOARD_W as f32, BOARD_H as f32, Color::new(0.0, 0.0, 0.0, 0.6));
    let center = (bx + BOARD_W / 2) as f32;
    let top = (by + BOARD_H / 2) as f32 - 10.0;
    banner(a, v, 1.0, title, center, top, BOARD_W as f32 - 2.0, time, t / 0.3, 0.0, |_, y| {
        scale(tint, 1.0 - 0.3 * y)
    });
    for (i, line) in lines.iter().enumerate() {
        text_centered(a, center, top as i32 + 16 + i as i32 * 2, line, TEXT);
    }
}

pub fn group_thousands(n: u32) -> String {
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, ch) in s.chars().enumerate() {
        if (i > 0) && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::group_thousands;

    #[test]
    fn thousands_separator() {
        assert_eq!(group_thousands(0), "0");
        assert_eq!(group_thousands(999), "999");
        assert_eq!(group_thousands(1000), "1,000");
        assert_eq!(group_thousands(1234567), "1,234,567");
    }
}