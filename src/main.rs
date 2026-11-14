mod ascii;

use macroquad::prelude::*;

use ascii::Ascii;
use rustris::bigfont;
use rustris::effects::{BoardGeom, CELL_ASPECT, Effects, Label, Rgb, piece_rgb};
use rustris::game::{Game, HEIGHT, PieceKind, WIDTH};
use rustris::player::{Action, HeldKeys, Player};

const CW: f32 = 6.0;
const CH: f32 = 10.0;
const COLS: usize = 112;
const ROWS: usize = 72;
const BW: i32 = 5;
const BH: i32 = 3;
const BX: i32 = 31;
const BY: i32 = 9;
const LEFT: (i32, i32) = (2, 28);
const RIGHT: (i32, i32) = (83, 109);

const FRAME: Rgb = [0.28, 0.36, 0.38];
const LABEL: Rgb = [0.45, 0.56, 0.56];
const TEXT: Rgb = [0.82, 0.94, 0.88];
const DIM: Rgb = [0.38, 0.46, 0.47];
const DENSE: &[u8] = b"####@%#";

fn geom() -> BoardGeom {
    BoardGeom {
        x: BX as f32,
        y: BY as f32,
        cell_w: BW as f32,
        cell_h: BH as f32
    }
}

fn scale(c: Rgb, f: f32) -> Rgb {
    [c[0] * f, c[1] * f, c[2] * f]
}

fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t
    ]
}

fn color(c: Rgb, a: f32) -> Color {
    Color::new(c[0], c[1], c[2], a)
}

fn hash(x: i32, y: i32, z: i32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8da6_b343)
        ^ (y as u32).wrapping_mul(0xd816_3841)
        ^ (z as u32).wrapping_mul(0xcb1a_b31f);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1_e995);
    h ^= h >> 15;
    (h & 0xffff) as f32 / 65535.0
}

struct View {
    scale: f32,
    ox: f32,
    oy: f32
}

impl View {
    fn fit(shake: Vec2) -> Self {
        let (dw, dh) = (COLS as f32 * CW, ROWS as f32 * CH);
        let scale = (screen_width() / dw).min(screen_height() / dh);
        View {
            scale,
            ox: ((screen_width() - dw * scale) / 2.0 + shake.x * scale).round(),
            oy: ((screen_height() - dh * scale) / 2.0 + shake.y * scale).round(),
        }
    }

    fn px(&self, col: f32, row: f32) -> (f32, f32) {
        (
            self.ox + col * CW * self.scale,
            self.oy + row * CH * self.scale,
        )
    }

    fn rect(&self, col: f32, row: f32, w: f32, h: f32, c: Color) {
        let (x, y) = self.px(col, row);
        draw_rectangle(x, y, w * CW * self.scale, h * CH * self.scale, c);
    }

    fn block(&self, col: f32, row: f32, w: f32, h: f32, rgb: Rgb, bright: f32) {
        self.rect(col, row, w, h, color(scale(rgb, 0.14 * bright), 1.0));
        self.rect(
            col, 
            row, 
            w - 1.0, 
            1.0, 
            color(scale(rgb, 1.05 * bright), 1.0)
        );
    }

    fn backing(&self, col: f32, row: f32, w: f32, h: f32, a: f32) {
        self.rect(col, row, w, h, Color::new(0.008, 0.012, 0.016, a));
    }

    fn glow(&self, col: f32, row: f32, radius: f32, rgb: Rgb, a: f32) {
        let (x, y) = self.px(col, row);
        draw_circle(x, y, radius * CW * self.scale, color(rgb, a));
    }
}

fn frame(a: &mut Ascii, x0: i32, y0: i32, x1: i32, y1: i32, title: Option<&str>) {
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

#[allow(clippy::too_many_arguments)]
fn banner(
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

const HELP: &[(&str, &str)] = &[
    ("LEFT/RIGHT", "move"),
    ("DOWN", "soft drop"),
    ("SPACE", "hard drop"),
    ("UP / X", "rotate"),
    ("Z", "rotate left"),
    ("C / SHIFT", "hold"),
    ("P", "pause"),
    ("R", "restart"),
    ("V", "visual mode"),
    ("1-4", "preview fx"),
    #[cfg(not(target_arch = "wasm32"))]
    ("ESC", "quit"),
];

struct Frame<'a> {
    player: &'a Player,
    fx: &'a Effects,
    time: f32,
    over_for: Option<f32>
}

fn draw(a: &mut Ascii, f: &Frame) {
    let game: &Game = &f.player.game;
    let fx = f.fx;
    let shake = vec2(
        rand::gen_range(-1.0, 1.0) * fx.shake * 7.0,
        rand::gen_range(-1.0, 1.0) * fx.shake * 7.0,
    );
    let v = View::fit(shake);
    a.begin_scene();

    v.rect(
        LEFT.0 as f32,
        (BY - 1) as f32,
        (RIGHT.1 - LEFT.0 + 1) as f32,
        (HEIGHT as i32 * BH + 2) as f32,
        Color::new(0.02, 0.025, 0.03, 1.0),
    );
    v.rect(
        (BX - 1) as f32,
        (BY - 1) as f32,
        (WIDTH as i32 * BW + 2) as f32,
        (HEIGHT as i32 * BH + 2) as f32,
        Color::new(0.01, 0.014, 0.018, 1.0),
    );

    for (r, row) in game.board.iter().enumerate() {
        for (c, &id) in row.iter().enumerate() {
            let (x, y) = ((BX + c as i32 * BW) as f32, (BY + r as i32 * BH) as f32);
            let gone = f.over_for.is_some_and(|t| t > r as f32 * 0.035);
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
                let (x, y) = ((BX + c * BW) as f32, (BY + gr * BH) as f32);
                v.rect(x, y, BW as f32, BH as f32, color(scale(rgb, 0.13), 1.0));
            }
        }
        let pulse = if game.on_ground() {
            0.9 + 0.2 * (f.time * 14.0).sin()
        } else {
            1.1
        };
        for (r, c) in p.cells() {
            if r >= 0 {
                v.block(
                    (BX + c * BW) as f32,
                    (BY + r * BH) as f32,
                    BW as f32,
                    BH as f32,
                    rgb,
                    pulse,
                );
            }
        }
    }

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

    let (bx1, by1) = (BX + WIDTH as i32 * BW, BY + HEIGHT as i32 * BH);
    frame(a, BX - 1, BY - 1, bx1, by1, None);
    frame(a, LEFT.0, BY - 1, LEFT.1, by1, None);
    frame(a, RIGHT.0, BY - 1, RIGHT.1, by1, None);

    let lx = LEFT.0 + 2;
    frame(a, lx, BY + 1, LEFT.1 - 2, BY + 10, Some("HOLD"));
    if let Some(kind) = game.hold {
        mini_piece(
            &v,
            kind,
            (LEFT.0 + LEFT.1 + 1) as f32 / 2.0,
            (BY + 4) as f32,
        );
    }
    for (i, (key, what)) in HELP.iter().enumerate() {
        let y = BY + 14 + i as i32 * 2;
        a.text(lx, y, key, LABEL);
        a.text(lx + 12, y, what, DIM);
    }

    let rx = RIGHT.0 + 2;
    frame(a, rx, BY + 1, RIGHT.1 - 2, BY + 33, Some("NEXT"));
    for (i, &kind) in game.queue.iter().take(5).enumerate() {
        let cx = (RIGHT.0 + RIGHT.1 + 1) as f32 / 2.0;
        mini_piece(&v, kind, cx, (BY + 3 + i as i32 * 6) as f32);
    }
    let stats = [
        ("SCORE", group_thousands(game.score)),
        ("LINES", game.lines.to_string()),
        ("LEVEL", game.level.to_string()),
        ("PIECES", game.pieces.to_string()),
        ("TETRISES", game.tetrises.to_string()),
    ];
    for (i, (label, value)) in stats.iter().enumerate() {
        let y = BY + 37 + i as i32 * 4;
        a.text(rx, y, label, LABEL);
        a.text(rx, y + 1, value, TEXT);
    }
    let mode = match a.mode {
        ascii::Mode::Full => "ascii + fx",
        ascii::Mode::Plain => "ascii",
        ascii::Mode::Raw => "raw",
    };
    a.text(rx, by1 - 2, &format!("view: {mode}"), DIM);

    banner(
        a,
        &v,
        1.0,
        "RUSTRIS",
        (BX as f32 + bx1 as f32) / 2.0,
        1.0,
        100.0,
        f.time,
        1.0,
        0.0,
        |x, y| {
            let t = (x + (f.time * 0.4).sin() * 0.3).clamp(0.0, 1.0);
            scale(mix([0.45, 0.95, 0.8], [0.55, 0.9, 0.45], t), 1.0 - 0.25 * y)
        },
    );

    // ---- particles, beams, rings
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
                a.put(
                    x,
                    y,
                    if k > 0.8 { b'#' } else { ch },
                    scale(ring.color, 0.3 + 0.9 * k),
                );
            }
        }
    }

    let center = BX as f32 + (WIDTH as i32 * BW) as f32 / 2.0;
    let board_w = (WIDTH as i32 * BW) as f32 - 2.0;
    for l in &fx.labels {
        let t = l.age / l.life;
        let (enter, exit) = (l.age / 0.18, ((t - 0.6) / 0.4).max(0.0));
        let back = 0.85 * enter.min(1.0) * (1.0 - exit);
        banner(
            a,
            &v,
            back,
            l.text,
            center,
            l.y,
            board_w,
            f.time,
            enter,
            exit,
            |_, y| scale(l.color, 1.0 - 0.3 * y),
        );
    }
    let overlay = |a: &mut Ascii, v: &View, title: &str, lines: &[String], t: f32| {
        v.rect(
            BX as f32,
            BY as f32,
            (WIDTH as i32 * BW) as f32,
            (HEIGHT as i32 * BH) as f32,
            Color::new(0.0, 0.0, 0.0, 0.6),
        );
        let top = BY as f32 + (HEIGHT as i32 * BH) as f32 / 2.0 - 10.0;
        banner(
            a,
            v,
            1.0,
            title,
            center,
            top,
            board_w,
            f.time,
            t / 0.3,
            0.0,
            |_, y| scale([0.95, 0.85, 0.6], 1.0 - 0.3 * y),
        );
        for (i, line) in lines.iter().enumerate() {
            let x = (center - line.len() as f32 / 2.0).round() as i32;
            a.text(x, top as i32 + 16 + i as i32 * 2, line, TEXT);
        }
    };
    if let Some(t) = f.over_for.filter(|&t| t > 0.9) {
        let lines = [
            format!("score {}", group_thousands(game.score)),
            "R to restart".into(),
        ];
        overlay(a, &v, "GAME OVER", &lines, t - 0.9);
    } else if f.player.paused {
        overlay(a, &v, "PAUSED", &["P to resume".into()], 1.0);
    }

    a.present(
        vec2(v.ox, v.oy),
        vec2(CW * v.scale, CH * v.scale),
        f.time,
        fx.flash,
    );
}

fn group_thousands(n: u32) -> String {
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

fn new_seed() -> u64 {
    (macroquad::miniquad::date::now() * 1000.0) as u64
}

fn window_conf() -> Conf {
    Conf {
        window_title: "Rustris".to_owned(),
        window_width: (COLS as f32 * CW) as i32,
        window_height: (ROWS as f32 * CH) as i32,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let mut ascii = Ascii::new(COLS, ROWS);
    let mut player = Player::new(new_seed());
    let mut fx = Effects::new(geom(), new_seed() ^ 0x5eed);
    let mut over_for: Option<f32> = None;
    let mut time = 0.0f32;

    loop {
        #[cfg(not(target_arch = "wasm32"))]
        if is_key_pressed(KeyCode::Escape) {
            break;
        }
        if is_key_pressed(KeyCode::R) {
            player = Player::new(new_seed());
            fx.clear();
            over_for = None;
        }
        if is_key_pressed(KeyCode::P) {
            player.toggle_pause();
        }
        if is_key_pressed(KeyCode::V) {
            ascii.mode = ascii.mode.next();
        }

        for (n, key) in [KeyCode::Key1, KeyCode::Key2, KeyCode::Key3, KeyCode::Key4]
            .into_iter()
            .enumerate()
        {
            if is_key_pressed(key) {
                let rows: Vec<_> = (0..=n)
                    .map(|i| {
                        let mut row = [0u8; WIDTH];
                        for cell in row.iter_mut() {
                            *cell = rand::gen_range(1, 8);
                        }
                        (HEIGHT - 1 - i, row)
                    })
                    .collect();
                fx.line_clear(&rows);
            }
        }

        let before = (player.game.pieces, player.game.level);
        let presses = [
            (KeyCode::Up, Action::RotateCw),
            (KeyCode::X, Action::RotateCw),
            (KeyCode::Z, Action::RotateCcw),
            (KeyCode::Space, Action::HardDrop),
            (KeyCode::C, Action::Hold),
            (KeyCode::LeftShift, Action::Hold),
            (KeyCode::RightShift, Action::Hold),
        ];

        for (key, action) in presses {
            if is_key_pressed(key) {
                let game = &player.game;
                let active = !player.paused && !game.game_over;
                // remember where a hard-dropped piece starts and lands
                let drop = (action == Action::HardDrop && active).then(|| {
                    let p = game.current;
                    let dist = game.ghost_y() - p.y;
                    let cells: Vec<_> = p.cells().iter().map(|&(r, c)| (r + dist, c)).collect();
                    (cells, dist, piece_rgb(p.kind.id()))
                });
                player.act(action);
                if let Some((cells, dist, rgb)) = drop {
                    fx.hard_drop(&cells, dist, rgb);
                }
            }
        }

        let keys = HeldKeys {
            left: is_key_down(KeyCode::Left),
            right: is_key_down(KeyCode::Right),
            soft_drop: is_key_down(KeyCode::Down),
        };
        let dt = get_frame_time().min(0.1);
        player.update(dt, keys);

        let game = &player.game;
        if game.pieces != before.0 && game.last_clear > 0 {
            let n = game.last_clear as usize;
            let rows: Vec<_> = (0..n)
                .map(|i| (game.cleared_rows[i], game.cleared_cells[i]))
                .collect();
            fx.line_clear(&rows);
        }
        if game.level > before.1 {
            fx.label("LEVEL UP", (BY + 6) as f32, [1.0, 0.85, 0.45], 1.6);
        }
        if game.game_over && over_for.is_none() {
            fx.game_over(&game.board);
            over_for = Some(0.0);
        }
        if let Some(t) = over_for.as_mut() {
            *t += dt;
        }

        time += dt;
        fx.update(dt);
        draw(
            &mut ascii, 
            &Frame {
                player: &player,
                fx: &fx,
                time,
                over_for
            }
        );
        next_frame().await;
    }
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