use crate::bigfont;
use crate::game::{Board, HEIGHT, Row, WIDTH};
use crate::rng::Rng;

pub type Rgb = [f32; 3];

pub const CELL_ASPECT: f32 = 10.0 / 6.0;

pub const PALETTE: [Rgb; 9] = [
    [0.0, 0.0, 0.0],
    [0.36, 0.86, 0.86], // I
    [0.96, 0.82, 0.40], // O
    [0.74, 0.52, 0.94], // T
    [0.52, 0.88, 0.46], // S
    [0.96, 0.44, 0.44], // Z
    [0.44, 0.60, 0.98], // J
    [0.98, 0.62, 0.32], // L
    [0.50, 0.53, 0.56]  // garbage
];

pub fn piece_rgb(id: u8) -> Rgb {
    PALETTE[(id as usize).min(PALETTE.len() - 1)]
}

const WHITE: Rgb = [1.0, 1.0, 1.0];

const SHARD_SEQS: [&[u8]; 6] = [b"/,.", b"\\'.", b"|:.", b"-~.", b"<-.", b">-."];
const DISSOLVE_SEQ: &[u8] = b"@#%*+=-:.";
const SPARK_SEQS: [&[u8]; 3] = [b"*+:.", b"x+.", b"#*+."];
const FIREWORK_SEQS: [&[u8]; 4] = [b"@*+.", b"#x+.", b"%*:.", b"O*o."];
const RAIN_CHARS: &[u8] = b"0123456789ABCDEF$#%&*<>{}=+";
const TRAIL_SEQ: &[u8] = b"|:.";
const DUST_SEQ: &[u8] = b"o.,";
const CRUMBLE_SEQS: [&[u8]; 3] = [b"#%+:.", b"@*=-.", b"M#*,."];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoardGeom {
    pub x: f32,
    pub y: f32,
    pub cell_w: f32,
    pub cell_h: f32,
}

impl BoardGeom {
    pub fn width(&self) -> f32 {
        WIDTH as f32 * self.cell_w
    }

    pub fn height(&self) -> f32 {
        HEIGHT as f32 * self.cell_h
    }

    pub fn cell_origin(&self, row: f32, col: f32) -> (f32, f32) {
        (self.x + col * self.cell_w, self.y + row * self.cell_h)
    }

    pub fn center_x(&self) -> f32 {
        self.x + self.width() / 2.0
    }
}

#[derive(Debug, Clone)]
pub struct Particle {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub gravity: f32,
    pub drag: f32,
    pub wobble: f32,
    pub phase: f32,
    pub age: f32,
    pub life: f32,
    pub glyphs: &'static [u8],
    pub color: Rgb,
    pub heat: f32,
    pub glow: f32
}

impl Particle {
    fn new(x: f32, y: f32, glyphs: &'static [u8], color: Rgb, life: f32) -> Self {
        Particle {
            x,
            y,
            vx: 0.0,
            vy: 0.0,
            gravity: 0.0,
            drag: 0.0,
            wobble: 0.0,
            phase: 0.0,
            age: 0.0,
            life,
            glyphs,
            color,
            heat: 0.0,
            glow: 0.0
        }
    }

    pub fn visible(&self) -> bool {
        self.age >= 0.0 && self.age < self.life
    }

    pub fn t(&self) -> f32 {
        (self.age / self.life).clamp(0.0, 1.0)
    }

    pub fn glyph(&self) -> u8 {
        let i = (self.t() * self.glyphs.len() as f32) as usize;
        self.glyphs[i.min(self.glyphs.len() - 1)]
    }

    pub fn brightness(&self) -> f32 {
        (1.0 - self.t()).powf(0.8)
    }

    pub fn current_color(&self) -> Rgb {
        let h = self.heat * (1.0 - self.t()).powi(3);
        lerp_rgb(self.color, WHITE, h)
    }
}

#[derive(Debug, Clone)]
pub struct Ring {
    pub x: f32,
    pub y: f32,
    pub age: f32,
    pub life: f32,
    pub speed: f32,
    pub thickness: f32,
    pub color: Rgb
}

impl Ring {
    pub fn radius(&self) -> f32 {
        let t = (self.age / self.life).clamp(0.0, 1.0);
        self.speed * self.life * (1.0 - (1.0 - t).powi(2)) / 2.0
    }

    pub fn brightness(&self) -> f32 {
        if self.age < 0.0 {
            0.0
        } else {
            (1.0 - self.age / self.life).clamp(0.0, 1.0)
        }
    }
}

#[derive(Debug, Clone)]
pub struct Sweep {
    pub x: f32,
    pub end: f32,
    pub dir: f32,
    pub y: f32,
    pub h: f32,
    pub speed: f32,
    pub color: Rgb,
    next_col: f32
}

impl Sweep {
    pub fn done(&self) -> bool {
        (self.x - self.end) * self.dir >= 0.0
    }
}

#[derive(Debug, Clone)]
struct Stream {
    x: f32,
    y: f32,
    end: f32,
    speed: f32,
    delay: f32,
    color: Rgb,
    next_row: f32
}

#[derive(Debug, Clone)]
pub struct Label {
    pub text: &'static str,
    pub x: f32,
    pub y: f32,
    pub age: f32,
    pub life: f32,
    pub color: Rgb
}

impl Label {
    pub const SCALE_X: usize = 2;

    pub fn width(&self) -> usize {
        bigfont::width(self.text, Self::SCALE_X)
    }
}

#[derive(Debug, Clone)]
pub struct Flash {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub age: f32,
    pub life: f32,
    pub color: Rgb
}

impl Flash {
    pub fn brightness(&self) -> f32 {
        (1.0 - self.age / self.life).clamp(0.0, 1.0).powi(2)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClearStyle {
    Shatter,
    Dissolve,
    Sweep,
    Cascade,
    Fireworks,
    Shockwave
}

impl ClearStyle {
    pub const ALL: [ClearStyle; 6] = [
        ClearStyle::Shatter,
        ClearStyle::Dissolve,
        ClearStyle::Sweep,
        ClearStyle::Cascade,
        ClearStyle::Fireworks,
        ClearStyle::Shockwave
    ];
}

pub const CLEAR_NAMES: [&str; 5] = ["", "SINGLE", "DOUBLE", "TRIPLE", "TETRIS"];

const MAX_PARTICLES: usize = 8000;

#[derive(Debug, Clone)]
pub struct Comet {
    pub from: (f32, f32),
    pub to: (f32, f32),
    pub age: f32,
    pub life: f32,
    pub lines: u32,
    pub color: Rgb
}

impl Comet {
    pub fn at(&self, t: f32) -> (f32, f32) {
        let t = t.clamp(0.0, 1.0);
        let (x0, y0) = self.from;
        let (x1, y1) = self.to;
        let (cx, cy) = ((x0 + x1) / 2.0, y0.min(y1) - 18.0);
        let u = 1.0 - t;
        (
            u * u * x0 + 2.0 * u * t * cx + t * t * x1,
            u * u * y0 + 2.0 * u * t * cy + t * t * y1,
        )
    }

    pub fn t(&self) -> f32 {
        (self.age / self.life).clamp(0.0, 1.0)
    }
}

pub struct Effects {
    pub geom: BoardGeom,
    pub particles: Vec<Particle>,
    pub rings: Vec<Ring>,
    pub sweeps: Vec<Sweep>,
    pub labels: Vec<Label>,
    pub flashes: Vec<Flash>,
    pub comets: Vec<Comet>,
    pub shake: f32,
    pub flash: f32,
    streams: Vec<Stream>,
    rng: Rng,
    last_single: Option<ClearStyle>
}

impl Effects {
    pub fn new(geom: BoardGeom, seed: u64) -> Self {
        Effects {
            geom,
            particles: Vec::new(),
            rings: Vec::new(),
            sweeps: Vec::new(),
            labels: Vec::new(),
            flashes: Vec::new(),
            comets: Vec::new(),
            shake: 0.0,
            flash: 0.0,
            streams: Vec::new(),
            rng: Rng::new(seed),
            last_single: None
        }
    }

    pub fn is_idle(&self) -> bool {
        self.particles.is_empty()
            && self.rings.is_empty()
            && self.sweeps.is_empty()
            && self.labels.is_empty()
            && self.flashes.is_empty()
            && self.streams.is_empty()
            && self.comets.is_empty()
            && self.shake == 0.0
            && self.flash == 0.0
    }

    pub fn clear(&mut self) {
        let (geom, seed) = (self.geom, self.rng.next_u64());
        *self = Effects::new(geom, seed);
    }

    fn push(&mut self, p: Particle) {
        if self.particles.len() < MAX_PARTICLES {
            self.particles.push(p);
        }
    }

    pub fn comet(&mut self, from: (f32, f32), to: (f32, f32), lines: u32, color: Rgb) {
        self.comets.push(Comet {
            from,
            to,
            age: 0.0,
            life: 0.5,
            lines,
            color
        });
    }

    pub fn garbage_in(&mut self, lines: u32) {
        let g = self.geom;
        let n = (lines as usize).min(HEIGHT);
        for r in HEIGHT - n..HEIGHT {
            let (x, y) = g.cell_origin(r as f32, 0.0);
            self.flashes.push(Flash {
                x,
                y,
                w: g.width(),
                h: g.cell_h,
                age: 0.0,
                life: 0.35,
                color: [0.9, 0.3, 0.3]
            });
        }
        self.shake = self.shake.max(0.1 + 0.05 * n as f32);
    }

    pub fn choose_styles(&mut self, n: usize) -> Vec<ClearStyle> {
        use ClearStyle::*;
        match n {
            0 => vec![],
            1 => {
                let options: Vec<_> = [Shatter, Dissolve, Sweep]
                    .into_iter()
                    .filter(|&s| Some(s) != self.last_single)
                    .collect();
                let s = self.rng.pick(&options);
                self.last_single = Some(s);
                vec![s]
            }
            2 => vec![self.rng.pick(&[Sweep, Fireworks, Shatter, Cascade])],
            3 => vec![self.rng.pick(&[Sweep, Fireworks]), Shatter],
            _ => vec![Shockwave, Fireworks, Cascade]
        }
    }

    pub fn line_clear(&mut self, rows: &[(usize, Row)]) {
        let styles = self.choose_styles(rows.len());
        self.line_clear_with(rows, &styles);
    }

    pub fn line_clear_with(&mut self, rows: &[(usize, Row)], styles: &[ClearStyle]) {
        if rows.is_empty() {
            return;
        }
        let g = self.geom;
        for &(r, _) in rows {
            let (x, y) = g.cell_origin(r as f32, 0.0);
            self.flashes.push(Flash {
                x,
                y,
                w: g.width(),
                h: g.cell_h,
                age: 0.0,
                life: 0.3,
                color: [0.75, 0.9, 0.85]
            });
        }
        for &style in styles {
            for &(r, ref cells) in rows {
                match style {
                    ClearStyle::Shatter => self.shatter(r, cells),
                    ClearStyle::Dissolve => self.dissolve(r, cells),
                    ClearStyle::Sweep => self.sweep(r, cells),
                    ClearStyle::Cascade => self.cascade(r, cells),
                    ClearStyle::Fireworks => self.fireworks(r, cells),
                    ClearStyle::Shockwave => {}
                }
            }
            if style == ClearStyle::Shockwave {
                self.shockwave(rows);
            }
        }

        let n = rows.len().min(4);

        let top_row = rows.iter().map(|&(r, _)| r).min().unwrap_or(0);
        let label_row = if top_row >= 8 { 3.0 } else { 14.0 };
        let y = g.y + label_row * g.cell_h;
        let color = [
            [0.0; 3],
            [0.7, 0.95, 0.85],
            [0.6, 0.9, 1.0],
            [1.0, 0.85, 0.5],
            [1.0, 0.6, 0.95],
        ][n];
        self.label(CLEAR_NAMES[n], y, color, 0.9 + 0.15 * n as f32);
        self.shake = self.shake.max([0.0, 0.0, 0.15, 0.3, 1.0][n])
    }

    pub fn label(&mut self, text: &'static str, y: f32, color: Rgb, life: f32) {
        self.labels.retain(|l| l.text != text);
        self.labels.push(Label {
            text,
            x: self.geom.center_x(),
            y,
            age: 0.0,
            life,
            color
        });
    }

    fn row_cells(&self, row: usize, cells: &Row) -> Vec<(f32, f32, Rgb)> {
        cells
            .iter()
            .enumerate()
            .filter(|&(_, &id)| id != 0)
            .map(|(c, &id)| {
                let (x, y) = self.geom.cell_origin(row as f32, c as f32);
                (x, y, piece_rgb(id))
            })
            .collect()
    }

    fn shatter(&mut self, row: usize, cells: &Row) {
        let g = self.geom;
        let half = g.width() / 2.0;
        for (x, y, color) in self.row_cells(row, cells) {
            for _ in 0..6 {
                let px = x + self.rng.range(0.0, g.cell_w);
                let py = y + self.rng.range(0.0, g.cell_h);
                let out = (px - g.center_x()) / half;
                let mut p = Particle::new(
                    px,
                    py,
                    self.rng.pick(&SHARD_SEQS),
                    color,
                    self.rng.range(0.6, 1.2)
                );
                p.vx = out * self.rng.range(8.0, 22.0) + self.rng.range(-6.0, 6.0);
                p.vy = self.rng.range(-16.0, -4.0);
                p.gravity = 34.0;
                p.drag = 0.5;
                p.heat = 0.5;
                p.glow = 0.5;
                self.push(p);
            }
        }
    }

    fn dissolve(&mut self, row: usize, cells: &Row) {
        let g = self.geom;
        for (x, y, color) in self.row_cells(row, cells) {
            let col = ((x - g.x) / g.cell_w).round();
            for i in 0..g.cell_w as usize {
                for j in 0..g.cell_h as usize {
                    let mut p = Particle::new(
                        x + i as f32 + 0.5,
                        y + j as f32 + 0.5,
                        DISSOLVE_SEQ,
                        color,
                        self.rng.range(0.7, 1.5)
                    );
                    p.vx = self.rng.range(-1.5, 1.5);
                    p.vy = self.rng.range(-7.0, -2.0);
                    p.drag = 0.4;
                    p.wobble = self.rng.range(1.0, 3.0);
                    p.phase = self.rng.range(0.0, std::f32::consts::TAU);
                    p.age = -((col - 4.5).abs() * 0.03 + self.rng.range(0.0, 0.15));
                    p.heat = 0.3;
                    p.glow = 0.25;
                    self.push(p);
                }
            }
        }
    }

    fn sweep(&mut self, row: usize, cells: &Row) {
        let g = self.geom;
        let (_, y) = g.cell_origin(row as f32, 0.0);
        let color = self
            .row_cells(row, cells)
            .first()
            .map(|c| c.2)
            .unwrap_or(WHITE);
        let cx = g.center_x();
        for dir in [-1.0, 1.0] {
            self.sweeps.push(Sweep {
                x: cx,
                end: if dir < 0.0 { g.x } else { g.x + g.width() },
                dir,
                y,
                h: g.cell_h,
                speed: 95.0,
                color: lerp_rgb(color, WHITE, 0.4),
                next_col: cx
            });
        }
    }

    fn cascade(&mut self, row: usize, cells: &Row) {
        let g = self.geom;
        let (_, y) = g.cell_origin(row as f32, 0.0);
        let bottom = g.y + g.height();
        for i in 0..g.width() as usize {
            if self.rng.f32() > 0.65 {
                continue;
            }
            let c = (i as f32 / g.cell_w) as usize;
            let tint = if cells[c.min(WIDTH - 1)] != 0 {
                piece_rgb(cells[c.min(WIDTH - 1)])
            } else {
                WHITE
            };
            let start = y + self.rng.range(0.0, g.cell_h);
            self.streams.push(Stream {
                x: g.x + i as f32 + 0.5,
                y: start,
                end: bottom,
                speed: self.rng.range(28.0, 60.0),
                delay: self.rng.range(0.0, 0.35),
                color: lerp_rgb([0.45, 1.0, 0.6], tint, 0.35),
                next_row: start.floor() + 0.5
            });
        }
    }

    fn fireworks(&mut self, row: usize, cells: &Row) {
        let g = self.geom;
        let blocks = self.row_cells(row, cells);
        let bursts = 2 + self.rng.below(3);
        for b in 0..bursts {
            let (bx, by, color) = blocks[self.rng.below(blocks.len())];
            let (cx, cy) = (bx + g.cell_w / 2.0, by + g.cell_h / 2.0);
            let delay = b as f32 * 0.09;
            let seq = self.rng.pick(&FIREWORK_SEQS);
            for _ in 0..30 {
                let a = self.rng.range(0.0, std::f32::consts::TAU);
                let speed = self.rng.range(8.0, 24.0);
                let mut p = Particle::new(
                    cx,
                    cy,
                    seq,
                    lerp_rgb(color, WHITE, 0.25),
                    self.rng.range(0.6, 1.2)
                );
                p.vx = a.cos() * speed;
                p.vy = a.sin() * speed / CELL_ASPECT;
                p.gravity = 10.0;
                p.drag = 1.6;
                p.age = -delay;
                p.heat = 0.8;
                p.glow = 0.8;
                self.push(p);
            }
        }
    }

    fn shockwave(&mut self, rows: &[(usize, Row)]) {
        let g = self.geom;
        let mid = rows.iter().map(|&(r, _)| r as f32 + 0.5).sum::<f32>() / rows.len() as f32;
        let (cx, cy) = (g.center_x(), g.y + mid * g.cell_h);
        for (delay, color) in [
            (0.0, [0.75, 1.0, 1.0]),
            (0.12, [1.0, 0.6, 0.95]),
            (0.24, [1.0, 0.9, 0.55]),
        ] {
            self.rings.push(Ring {
                x: cx,
                y: cy,
                age: -delay,
                life: 0.9,
                speed: 150.0,
                thickness: 1.6,
                color
            });
        }

        for _ in 0..90 {
            let a = self.rng.range(0.0, std::f32::consts::TAU);
            let speed = self.rng.range(20.0, 55.0);
            let mut p = Particle::new(
                cx,
                cy,
                self.rng.pick(&SPARK_SEQS),
                [1.0, 0.8, 0.5],
                self.rng.range(0.5, 1.1)
            );
            p.vx = a.cos() * speed;
            p.vy = a.sin() * speed / CELL_ASPECT;
            p.drag = 2.2;
            p.gravity = 6.0;
            p.heat = 1.0;
            p.glow = 0.6;
            self.push(p);
        }
        self.flash = self.flash.max(0.7);
        self.shake = self.shake.max(1.0);
    }

    pub fn hard_drop(&mut self, cells: &[(i32, i32)], dist: i32, color: Rgb) {
        if dist <= 0 {
            return;
        }
        let g = self.geom;
        for &(r, c) in cells {
            if cells.iter().any(|&(r2, c2)| c2 == c && r2 < r) {
                continue;
            }
            let (x, y) = g.cell_origin(r as f32, c as f32);
            let top = y - dist as f32 * g.cell_h;
            let mut yy = y - 0.5;
            while yy > top.max(g.y) {
                for i in [1.0, 3.0] {
                    let mut p = Particle::new(
                        x + i + 0.5,
                        yy,
                        TRAIL_SEQ,
                        color,
                        self.rng.range(0.15, 0.3)
                    );
                    p.age = -(yy - top) * 0.003;
                    self.push(p);
                }
                yy -= 1.0;
            }
        }
        for &(r, c) in cells {
            if cells.iter().any(|&(r2, c2)| c2 == c && r2 < r) {
                continue;
            }
            let (x, y) = g.cell_origin(r as f32, c as f32);
            for _ in 0..3 {
                let mut p = Particle::new(
                    x + self.rng.range(0.0, g.cell_w),
                    y - 0.5,
                    DUST_SEQ,
                    [0.7, 0.75, 0.8],
                    self.rng.range(0.25, 0.45)
                );
                p.vx = self.rng.range(-10.0, 10.0);
                p.vy = self.rng.range(-4.0, -1.0);
                p.drag = 3.0;
                self.push(p);
            }
        }
        self.shake = self.shake.max(0.06 + 0.01 * dist.min(10) as f32);

    }

    pub fn game_over(&mut self, board: &Board) {
        let g = self.geom;
        for (r, row) in board.iter().enumerate() {
            for (c, &id) in row.iter().enumerate() {
                if id == 0 {
                    continue;
                }
                let (x, y) = g.cell_origin(r as f32, c as f32);
                for _ in 0..4 {
                    let mut p = Particle::new(
                        x + self.rng.range(0.0, g.cell_w),
                        y + self.rng.range(0.0, g.cell_h),
                        self.rng.pick(&CRUMBLE_SEQS),
                        piece_rgb(id),
                        self.rng.range(0.9, 1.6)
                    );
                    p.vx = self.rng.range(-3.0, 3.0);
                    p.vy = self.rng.range(-3.0, 1.0);
                    p.gravity = 30.0;
                    p.age = -(r as f32 * 0.035 + self.rng.range(0.0, 0.1));
                    p.glow = 0.2;
                    self.push(p);
                }
            }
        }
        self.shake = self.shake.max(0.5);
    }

    pub fn update(&mut self, dt: f32) {
        let bottom = self.geom.y + self.geom.height() + 30.0;
        for p in &mut self.particles {
            p.age += dt;
            if p.age < 0.0 {
                continue;
            }
            let keep = (1.0 - p.drag * dt).max(0.0);
            p.vx *= keep;
            p.vy *= keep;
            p.vy += p.gravity * dt;
            p.x += (p.vx + p.wobble * (p.phase + p.age * 7.0).sin()) * dt;
            p.y += p.vy * dt;
        }
        self.particles.retain(|p| p.age < p.life && p.y < bottom && p.y > -60.0);

        let mut spawned = Vec::new();
        for s in &mut self.sweeps {
            s.x += s.dir * s.speed * dt;
            while (s.next_col - s.x) * s.dir < 0.0 && (s.next_col - s.end) * s.dir < 0.0 {
                let col = s.next_col.floor() + 0.5;
                for j in 0..s.h as usize {
                    let mut p = Particle::new(col, s.y + j as f32 + 0.5, b"=-.", s.color, 0.35);
                    p.heat = 0.6;
                    spawned.push(p);
                }
                s.next_col += s.dir;
            }
            for _ in 0..((dt * 150.0) as usize).max(1) {
                let mut p = Particle::new(
                    s.x.clamp(s.end.min(s.x), s.end.max(s.x)),
                    s.y + self.rng.range(0.0, s.h),
                    self.rng.pick(&SPARK_SEQS),
                    s.color,
                    self.rng.range(0.3, 0.6)
                );
                p.vx = -s.dir * self.rng.range(3.0, 14.0);
                p.vy = self.rng.range(-9.0, 3.0);
                p.gravity = 25.0;
                p.drag = 1.0;
                p.heat = 0.9;
                p.glow = 0.6;
                spawned.push(p);
            }
        }
        self.sweeps.retain(|s| !s.done());

        for s in &mut self.streams {
            if s.delay > 0.0 {
                s.delay -= dt;
                continue;
            }
            s.y += s.speed * dt;
            while s.next_row < s.y.min(s.end) {
                let i = self.rng.below(RAIN_CHARS.len());
                let mut p = Particle::new(
                    s.x,
                    s.next_row,
                    &RAIN_CHARS[i..i + 1],
                    s.color,
                    self.rng.range(0.35, 0.8)
                );
                p.heat = 1.0;
                p.glow = 0.3;
                spawned.push(p);
                s.next_row += 1.0;
            }
        }
        self.streams.retain(|s| s.y < s.end);
        for p in spawned {
            self.push(p);
        }

        for r in &mut self.rings {
            r.age += dt;
        }
        self.rings.retain(|r| r.age < r.life);
        for l in &mut self.labels {
            l.age += dt;
        }
        self.labels.retain(|l| l.age < l.life);
        for f in &mut self.flashes {
            f.age += dt;
        }
        self.flashes.retain(|f| f.age < f.life);

        let mut sparks = Vec::new();
        for c in &mut self.comets {
            c.age += dt;
            let (x, y) = c.at(c.t());
            let n = if c.age >= c.life { 18 + 4 * c.lines } else { 1 + c.lines / 2};
            for _ in 0..n {
                let mut p = Particle::new(x, y, self.rng.pick(&SPARK_SEQS), c.color, self.rng.range(0.2, 0.5));
                let a = self.rng.range(0.0, std::f32::consts::TAU);
                let speed = if c.age >= c.life { self.rng.range(6.0, 20.0) } else { self.rng.range(0.5, 3.0)};
                p.vx = a.cos() * speed;
                p.vy = a.sin() * speed / CELL_ASPECT;
                p.drag = 2.0;
                p.heat = 0.8;
                p.glow = 0.5;
                sparks.push(p);
            }
        }
        self.comets.retain(|c| c.age < c.life);
        for p in sparks {
            self.push(p);
        }

        self.shake = (self.shake - dt * 2.0).max(0.0);
        self.flash = (self.flash - dt * 2.5).max(0.0);
    }
}

fn lerp_rgb(a: Rgb, b: Rgb, t: f32) -> Rgb {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn geom() -> BoardGeom {
        BoardGeom {
            x: 31.0,
            y: 9.0,
            cell_w: 5.0,
            cell_h: 3.0,
        }
    }

    fn full_row(id: u8) -> Row {
        [id; WIDTH]
    }

    fn run(fx: &mut Effects, seconds: f32) {
        for _ in 0..(seconds * 60.0) as usize {
            fx.update(1.0 / 60.0);
        }
    }

    #[test]
    fn every_style_produces_something_then_finishes() {
        let rows = [(19, full_row(1)), (18, full_row(3))];
        for style in ClearStyle::ALL {
            let mut fx = Effects::new(geom(), 1);
            fx.line_clear_with(&rows, &[style]);
            run(&mut fx, 0.1);
            let busy = !fx.particles.is_empty() || !fx.rings.is_empty();
            assert!(busy, "{style:?} made nothing");
            run(&mut fx, 4.0);
            assert!(fx.is_idle(), "{style:?} never finished");
        }
    }

    #[test]
    fn particles_start_near_the_board() {
        let g = geom();
        let mut fx = Effects::new(g, 2);
        fx.line_clear_with(&[(10, full_row(5))], &ClearStyle::ALL);
        for p in &fx.particles {
            assert!(p.x >= g.x - 1.0 && p.x <= g.x + g.width() + 1.0);
            assert!(p.y >= g.y - 1.0 && p.y <= g.y + g.height() + 1.0);
            assert!(p.x.is_finite() && p.vx.is_finite() && p.vy.is_finite());
        }
    }

    #[test]
    fn singles_vary() {
        let mut fx = Effects::new(geom(), 3);
        let mut prev = None;
        for _ in 0..50 {
            let s = fx.choose_styles(1)[0];
            assert_ne!(Some(s), prev);
            prev = Some(s);
        }
    }

    #[test]
    fn tetris_is_the_big_one() {
        let mut fx = Effects::new(geom(), 4);
        assert!(fx.choose_styles(4).contains(&ClearStyle::Shockwave));
        let rows: Vec<_> = (16..20).map(|r| (r, full_row(1))).collect();
        fx.line_clear(&rows);
        assert!(fx.shake >= 1.0 && fx.flash > 0.0);
        assert_eq!(fx.labels[0].text, "TETRIS");
    }

    #[test]
    fn labels_fit_on_the_board() {
        let g = geom();
        for name in CLEAR_NAMES
            .iter()
            .chain(&["GAME OVER", "PAUSED", "LEVEL UP", "RUSTRIS"])
        {
            for word in name.split(' ') {
                assert!(word.chars().all(|c| bigfont::glyph(c).is_some()), "{name}");
            }
            assert!(
                bigfont::width(name, Label::SCALE_X) as f32 <= g.width() || name.contains(' '),
                "{name}"
            );
        }
    }

    #[test]
    fn particle_glyphs_step_through_sequence() {
        let mut p = Particle::new(0.0, 0.0, b"abc", WHITE, 1.0);
        assert_eq!(p.glyph(), b'a');
        p.age = 0.5;
        assert_eq!(p.glyph(), b'b');
        p.age = 0.99;
        assert_eq!(p.glyph(), b'c');
        assert!(p.brightness() < 0.1);
    }

    #[test]
    fn hard_drop_and_game_over_effects() {
        let mut fx = Effects::new(geom(), 5);
        fx.hard_drop(&[(19, 4), (19, 5), (18, 4), (18, 5)], 15, WHITE);
        assert!(!fx.particles.is_empty());
        let mut board = [[0u8; WIDTH]; HEIGHT];
        board[19] = full_row(2);
        fx.game_over(&board);
        run(&mut fx, 4.0);
        assert!(fx.is_idle());
    }
}