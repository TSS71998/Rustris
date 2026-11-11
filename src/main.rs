use macroquad::prelude::*;

use rustris::game::{Game, HEIGHT, PieceKind, WIDTH};
use rustris::player::{Action, HeldKeys, Player};

const CELL: f32 = 30.0;
const MINI: f32 = 18.0;
const PAD: f32 = 20.0;
const SIDE_L: f32 = 6.0 * MINI + 2.0 * PAD;
const SIDE_R: f32 = 7.0 * MINI + 2.0 * PAD + 40.0;
const BOARD_X: f32 = PAD + SIDE_L;
const BOARD_Y: f32 = PAD;
const BOARD_W: f32 = WIDTH as f32 * CELL;
const BOARD_H: f32 = HEIGHT as f32 * CELL;
const WIN_W: f32 = BOARD_X + BOARD_W + SIDE_R;
const WIN_H: f32 = BOARD_H + 2.0 * PAD;

const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::new(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0)
}

const BG: Color = rgb(18, 20, 28);
const GRID: Color = rgb(34, 38, 50);
const PANEL: Color = rgb(26, 29, 40);
const TEXT: Color = rgb(225, 228, 238);
const DIM: Color = rgb(130, 136, 155);

fn piece_color(kind: PieceKind) -> Color {
    match kind {
        PieceKind::I => rgb(64, 214, 230),
        PieceKind::O => rgb(245, 210, 70),
        PieceKind::T => rgb(176, 98, 230),
        PieceKind::S => rgb(100, 210, 110),
        PieceKind::Z => rgb(235, 82, 90),
        PieceKind::J => rgb(72, 120, 235),
        PieceKind::L => rgb(245, 150, 60),
    }
}

fn shade(c: Color, f: f32) -> Color {
    Color::new(
        (c.r * f).min(1.0),
        (c.g * f).min(1.0),
        (c.b * f).min(1.0),
        c.a,
    )
}

struct View {
    scale: f32,
    ox: f32,
    oy: f32
}

impl View {
    fn fit() -> Self {
        let scale = (screen_width() / WIN_W).min(screen_height() / WIN_H);
        View {
            scale,
            ox: (screen_width() - WIN_W * scale) / 2.0,
            oy: (screen_height() - WIN_H * scale) / 2.0,
        }
    }

    fn rect(&self, x: f32, y: f32, w: f32, h: f32, color: Color) {
        let s = self.scale;
        draw_rectangle(self.ox + x * s, self.oy + y * s, w * s, h * s, color);
    }

    fn outline(&self, x: f32, y: f32, w: f32, h: f32, thick: f32, color: Color) {
        let s = self.scale;
        draw_rectangle_lines(
            self.ox + x * s,
            self.oy + y * s,
            w * s,
            h * s,
            thick * s,
            color,
        );
    }

    fn text(&self, s: &str, x: f32, y: f32, size: f32, color: Color) {
        let px = (size * self.scale).round().max(1.0) as u16;
        let dims = measure_text(s, None, px, 1.0);
        draw_text(
            s,
            self.ox + x * self.scale,
            self.oy + y * self.scale + dims.offset_y,
            px as f32,
            color,
        );
    }

    fn text_centered(&self, s: &str, x: f32, y: f32, size: f32, color: Color) {
        let px = (size * self.scale).round().max(1.0) as u16;
        let dims = measure_text(s, None, px, 1.0);
        draw_text(
            s,
            self.ox + x * self.scale - dims.width / 2.0,
            self.oy + y * self.scale - dims.height / 2.0 + dims.offset_y,
            px as f32,
            color,
        );
    }

    fn cell(&self, x: f32, y: f32, size: f32, color: Color) {
        self.rect(x + 1.0, y + 1.0, size - 2.0, size - 2.0, color);
        self.rect(x + 3.0, y + 3.0, size - 8.0, 3.0, shade(color, 1.25));
        self.outline(x + 1.0, y + 1.0, size - 2.0, size - 2.0, 1.0, shade(color, 0.7));
    }

    fn ghost_cell(&self, x: f32, y: f32, size: f32, color: Color) {
        self.outline(x + 2.0, y + 2.0, size - 4.0, size - 4.0, 2.0, shade(color, 0.55));
    }

    fn mini_piece(&self, kind: PieceKind, cx: f32, top: f32) {
        let cells = kind.cells(0);
        let min_r = cells.iter().map(|c| c.0).min().unwrap();
        let max_r = cells.iter().map(|c| c.0).max().unwrap();
        let min_c = cells.iter().map(|c| c.1).min().unwrap();
        let max_c = cells.iter().map(|c| c.1).max().unwrap();
        let w = (max_c - min_c + 1) as f32 * MINI;
        let h = (max_r - min_r + 1) as f32 * MINI;
        let (ox, oy) = (cx - w / 2.0, top + (2.0 * MINI - h) / 2.0);
        for (r, c) in cells {
            self.cell(
                ox + (c - min_c) as f32 * MINI,
                oy + (r - min_r) as f32 * MINI,
                MINI,
                piece_color(kind),
            );
        }
    }
}

const HELP: &[&str] = &[
    "Left/Right  move",
    "Down  soft drop",
    "Space  hard drop",
    "Up / X  rotate",
    "Z  rotate left",
    "C / Shift  hold",
    "P  pause",
    "R  restart",
    #[cfg(not(target_arch = "wasm32"))]
    "Esc  quit",
];

fn draw(game: &Game, paused: bool) {
    let v = View::fit();
    clear_background(BG);

    v.rect(BOARD_X - 4.0, BOARD_Y - 4.0, BOARD_W + 8.0, BOARD_H + 8.0, PANEL);
    for (r, row) in game.board.iter().enumerate() {
        for (c, &id) in row.iter().enumerate() {
            let (x, y) = (BOARD_X + c as f32 * CELL, BOARD_Y + r as f32 * CELL);
            match PieceKind::from_id(id) {
                Some(kind) => v.cell(x, y, CELL, piece_color(kind)),
                None => v.outline(x, y, CELL, CELL, 1.0, GRID),
            }
        }
    }

    if !game.game_over {
        let p = game.current;
        let color = piece_color(p.kind);
        let ghost_dy = game.ghost_y() - p.y;
        for (r, c) in p.cells() {
            let gr = r + ghost_dy;
            if gr >= 0 {
                v.ghost_cell(BOARD_X + c as f32 * CELL, BOARD_Y + gr as f32 * CELL, CELL, color);
            }
        }

        for (r, c) in p.cells() {
            if r >= 0 {
                v.cell(BOARD_X + c as f32 * CELL, BOARD_Y + r as f32 * CELL, CELL, color);
            }
        }
    }

    let lx = PAD;
    let hold_w = SIDE_L - 2.0 * PAD + 10.0;
    v.text("RUSTRIS", lx, PAD, 30.0, TEXT);
    v.text("HOLD", lx, PAD + 50.0, 18.0, DIM);
    v.rect(lx, PAD + 75.0, hold_w, 3.0 * MINI + 10.0, PANEL);

    if let Some(kind) = game.hold {
        v.mini_piece(kind, lx + hold_w / 2.0, PAD + 80.0);
    }

    for (i, line) in HELP.iter().enumerate() {
        v.text(line, lx, PAD + 160.0 + i as f32 * 20.0, 14.0, DIM);
    }

    let rx = BOARD_X + BOARD_W + PAD;
    let pw = SIDE_R - 2.0 * PAD;
    let slot = 2.0 * MINI + 12.0;
    v.text("NEXT", rx, PAD, 18.0, DIM);
    v.rect(rx, PAD + 25.0, pw, 5.0 * slot + 10.0, PANEL);
    for (i, &kind) in game.queue.iter().take(5).enumerate() {
        v.mini_piece(kind, rx + pw / 2.0, PAD + 32.0 + i as f32 * slot);
    }
    let stats = [
        ("SCORE", group_thousands(game.score)),
        ("LINES", game.lines.to_string()),
        ("LEVEL", game.level.to_string()),
        ("PIECES", game.pieces.to_string()),
    ];
    let mut y = PAD + 25.0 + 5.0 * slot + 30.0;
    for (label, value) in stats {
        v.text(label, rx, y, 14.0, DIM);
        v.text(&value, rx, y + 16.0, 18.0, TEXT);
        y += 46.0;
    }

    let overlay: Option<(&str, Vec<String>)> = if game.game_over {
        Some((
            "GAME OVER",
            vec![
                format!("Score {}", group_thousands(game.score)),
                "R to restart".into()
            ],
        ))
    } else if paused {
        Some(("PAUSED", vec!["P to resume".into()]))
    } else {
        None
    };

    if let Some((title, lines)) = overlay {
        v.rect(BOARD_X, BOARD_Y, BOARD_W, BOARD_H, Color::new(0.0, 0.0, 0.0, 0.67));
        let (cx, cy) = (BOARD_X + BOARD_W / 2.0, BOARD_Y + BOARD_H / 2.0);
        v.text_centered(title, cx, cy - 20.0, 30.0, TEXT);
        for (i, line) in lines.iter().enumerate() {
            v.text_centered(line, cx, cy + 20.0 + i as f32 * 24.0, 18.0, DIM);
        }
    }
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
        window_width: WIN_W as i32,
        window_height: WIN_H as i32,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let mut player = Player::new(new_seed());

    loop {
        #[cfg(not(target_arch = "wasm32"))]
        if is_key_pressed(KeyCode::Escape) {
            break;
        }
        if is_key_pressed(KeyCode::R) {
            player = Player::new(new_seed());
        }
        if is_key_pressed(KeyCode::P) {
            player.toggle_pause();
        }

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
                player.act(action);
            }
        }

        let keys = HeldKeys {
            left: is_key_down(KeyCode::Left),
            right: is_key_down(KeyCode::Right),
            soft_drop: is_key_down(KeyCode::Down)
        };

        let dt = get_frame_time().min(0.1);
        player.update(dt, keys);

        draw(&player.game, player.paused);
        next_frame().await
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