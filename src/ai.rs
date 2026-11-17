use std::sync::OnceLock;

use crate::game::{Game, HEIGHT, PieceKind, WIDTH};
use crate::player::{Action, Player};

const FULL: u16 = (1 << WIDTH) - 1;

pub type Rows = [u16; HEIGHT];

pub const N_FEATURES: usize = 11;

pub const FEATURE_NAMES: [&str; N_FEATURES] = [
    "landing_height",  // how high the piece came to rest
    "eroded_cells",    // lines cleared x piece cells that were cleared
    "row_transitions", // filled/empty switches along each row
    "col_transitions", // filled/empty switches down each column
    "holes",           // empty cells with something above them
    "wells",           // depth of narrow wells (1+2+..+d per well)
    "rows_with_holes", // how many rows contain at least one hole
    "bumpiness",       // sum of height differences between neighbours
    "tetrises",        // 1 if this move cleared four lines
    "small_clears",    // lines cleared by a 1-3 line clear
    "danger",          // (max height - 10)^2 once the stack passes half way
];

const STEP_FEATURES: [usize; 4] = [0, 1, 8, 9];

pub type Features = [f32; N_FEATURES];

#[derive(Debug, Clone, PartialEq)]
pub struct Weights(pub [f32; N_FEATURES]);

impl Weights {
    pub fn parse(text: &str) -> Result<Weights, String> {
        let mut w = [None; N_FEATURES];
        for (n, line) in text.lines().enumerate() {
            let line = line.split('#').next().unwrap().trim();
            if line.is_empty() {
                continue;
            }
            let mut parts = line.split_whitespace();
            let (Some(name), Some(value), None) = (parts.next(), parts.next(), parts.next()) else {
                return Err(format!("line {}: expected `name value`", n + 1));
            };
            let i = FEATURE_NAMES
                .iter()
                .position(|&f| f == name)
                .ok_or_else(|| format!("line {}: unknown feature `{name}`", n + 1))?;
            let v: f32 = value
                .parse()
                .map_err(|_| format!("line {}: bad number `{value}`", n + 1))?;
            if w[i].replace(v).is_some() {
                return Err(format!("line {}: `{name}` given twice", n + 1));
            }
        }
        let mut out = [0.0; N_FEATURES];
        for (i, v) in w.iter().enumerate() {
            out[i] = v.ok_or_else(|| format!("missing feature `{}`", FEATURE_NAMES[i]))?;
        }
        Ok(Weights(out))
    }

    pub fn to_text(&self) -> String {
        FEATURE_NAMES
            .iter()
            .zip(self.0)
            .map(|(name, v)| format!("{name} {v}\n"))
            .collect()
    }

    fn dot(&self, f: &Features) -> f32 {
        self.0.iter().zip(f).map(|(w, x)| w * x).sum()
    }

    fn dot_step(&self, f: &Features) -> f32 {
        STEP_FEATURES.iter().map(|&i| self.0[i] * f[i]).sum()
    }
}

impl Default for Weights {
    fn default() -> Self {
        Weights::parse(include_str!("../assets/ai_weights.txt"))
            .expect("assets/ai_weights.txt is invalid")
    }
}

struct Orient {
    rot: u8,
    minc: i32,
    minr: i32,
    maxr: i32,
    width: usize,
    bottoms: [i32; 4],
    /// (box row, mask with the leftmost column at bit 0).
    rowmasks: Vec<(i32, u16)>
}

fn orients(kind: PieceKind) -> &'static [Orient] {
    static TABLE: OnceLock<Vec<Vec<Orient>>> = OnceLock::new();
    let table = TABLE.get_or_init(|| {
        PieceKind::ALL
            .iter()
            .map(|&k| {
                let mut seen: Vec<Vec<(i32, i32)>> = Vec::new();
                let mut out = Vec::new();
                for rot in 0..4u8 {
                    let cells = k.cells(rot);
                    let minc = cells.iter().map(|c| c.1).min().unwrap();
                    let minr = cells.iter().map(|c| c.0).min().unwrap();
                    let maxr = cells.iter().map(|c| c.0).max().unwrap();
                    let mut norm: Vec<_> = cells.iter().map(|&(r, c)| (r - minr, c - minc)).collect();
                    norm.sort();
                    if seen.contains(&norm) {
                        continue;
                    }
                    seen.push(norm);
                    let width = (cells.iter().map(|c| c.1).max().unwrap() - minc + 1) as usize;
                    let mut bottoms = [i32::MIN; 4];
                    let mut rowmasks: Vec<(i32, u16)> = Vec::new();
                    for &(r, c) in &cells {
                        let j = (c - minc) as usize;
                        bottoms[j] = bottoms[j].max(r);
                        match rowmasks.iter_mut().find(|(rr, _)| *rr == r) {
                            Some((_, m)) => *m |= 1 << j,
                            None => rowmasks.push((r, 1 << j)),
                        }
                    }
                    out.push(Orient {
                        rot,
                        minc,
                        minr,
                        maxr,
                        width,
                        bottoms,
                        rowmasks
                    });
                }
                out
            })
            .collect()
    });
    &table[kind as usize]
}

/// Convert the engine's board into one bitmask per row.
pub fn board_rows(game: &Game) -> Rows {
    let mut rows = [0u16; HEIGHT];
    for (r, row) in game.board.iter().enumerate() {
        for (c, &v) in row.iter().enumerate() {
            if v != 0 {
                rows[r] |= 1 << c;
            }
        }
    }
    rows
}

/// Index of the highest filled row in each column (HEIGHT if empty).
fn column_tops(rows: &Rows) -> [i32; WIDTH] {
    let mut tops = [HEIGHT as i32; WIDTH];
    let mut seen = 0u16;
    for (r, &b) in rows.iter().enumerate() {
        let mut new = b & !seen;
        seen |= new;
        while new != 0 {
            let c = new.trailing_zeros() as usize;
            tops[c] = r as i32;
            new &= new -1;
        }
        if seen == FULL {
            break;
        }
    }
    tops
}


fn board_features(rows: &Rows, f: &mut Features) {
    const WALLS: u16 = (1 << (WIDTH + 1)) | 1;
    const PAIR: u16 = (1 << (WIDTH + 1)) - 1;
    let (mut row_t, mut col_t, mut holes, mut rows_holes) = (0, 0, 0, 0);
    let (mut cover, mut seen, mut prev) = (0u16, 0u16, 0u16);
    let mut heights = [0i32; WIDTH];
    for (r, &b) in rows.iter().enumerate() {
        col_t += (b ^ prev).count_ones();
        prev = b;
        if b != 0 {
            let x = (b << 1) | WALLS;
            row_t += ((x ^ (x >> 1)) & PAIR).count_ones();
            let mut new = b & !seen;
            seen |= new;
            while new != 0 {
                let c = new.trailing_zeros() as usize;
                heights[c] = (HEIGHT - r) as i32;
                new &= new - 1;
            }
        }
        let hole = !b & cover & FULL;
        if hole != 0 {
            holes += hole.count_ones();
            rows_holes += 1;
        }
        cover |= b;
    }
    col_t += (FULL & !prev).count_ones();
    
    let (mut wells, mut bump) = (0, 0);
    for c in 0..WIDTH {
        let h = heights[c];
        let left = if c > 0 { heights[c - 1] } else { HEIGHT as i32 };
        let right = if c + 1 < WIDTH { heights[c + 1] } else { HEIGHT as i32 };
        let d = left.min(right) - h;
        if d > 0 {
            wells += d * (d + 1) / 2;
        }
        if c + 1 < WIDTH {
            bump += (h - right).abs();
        }
    }
    let max_h = *heights.iter().max().unwrap();
    f[2] = row_t as f32;
    f[3] = col_t as f32;
    f[4] = holes as f32;
    f[5] = wells as f32;
    f[6] = rows_holes as f32;
    f[7] = bump as f32;
    f[10] = ((max_h - 10).max(0) as f32).powi(2);
}

#[derive(Debug, Clone)]
pub struct Placement {
    pub rot: u8,
    pub x: i32,
    pub rows: Rows,
    pub lines: u32,
    pub features: Features
}

pub fn placements(rows: &Rows, kind: PieceKind, mut f: impl FnMut(Placement)) {
    let tops = column_tops(rows);
    for o in orients(kind) {
        for x in 0..=(WIDTH - o.width) {
            let y = (0..o.width)
                .map(|j| tops[x + j] - 1 - o.bottoms[j])
                .min()
                .unwrap();
            if y + o.minr < 0 {
                continue;
            }
            let mut new = *rows;
            for &(r, m) in &o.rowmasks {
                new[(y + r) as usize] |= m << x;
            }
            let mut lines = 0;
            let mut eroded = 0;
            for &(r, m) in &o.rowmasks {
                if new[(y + r) as usize] == FULL {
                    lines += 1;
                    eroded += m.count_ones();
                }
            }
            if lines > 0 {
                let mut out = [0u16; HEIGHT];
                let mut dst = HEIGHT;
                for &b in new.iter().rev() {
                    if b != FULL {
                        dst -= 1;
                        out[dst] = b;
                    }
                }
                new = out;
            }
            let mut feats = [0.0; N_FEATURES];
            board_features(&new, &mut feats);
            feats[0] = HEIGHT as f32 - (y as f32 + (o.minr + o.maxr) as f32 / 2.0);
            feats[1] = (lines * eroded) as f32;
            feats[8] = (lines == 4) as u32 as f32;
            feats[9] = if (1..4).contains(&lines) { lines as f32 } else { 0.0 };
            f(Placement {
                rot: o.rot,
                x: x as i32 - o.minc,
                rows: new,
                lines,
                features: feats
            });
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Move {
    pub hold: bool,
    pub rot: u8,
    pub x: i32
}

#[derive(Debug, Clone)]
pub struct Ai {
    pub weights: Weights,
    pub lookahead: bool,
    pub use_hold: bool
}

impl Default for Ai {
    fn default() -> Self {
        Ai {
            weights: Weights::default(),
            lookahead: true,
            use_hold: true
        }
    }
}

impl Ai {
    pub fn new(weights: Weights) -> Self {
        Ai {
            weights,
            ..Default::default()
        }
    }

    fn search(&self, rows: &Rows, kind: PieceKind, next: Option<PieceKind>) -> Option<(f32, u8, i32)> {
        let w = &self.weights;
        let mut best: Option<(f32, u8, i32)> = None;
        placements(rows, kind, |p| {
            let score = match next {
                None => w.dot(&p.features),
                Some(next) => {
                    let mut best2 = f32::NEG_INFINITY;
                    placements(&p.rows, next, |q| {
                        best2 = best2.max(w.dot(&q.features))
                    });
                    w.dot_step(&p.features) + if best2.is_finite() { best2 } else { -1e9 }
                }
            };
            if best.is_none_or(|(s, _, _)| score > s) {
                best = Some((score, p.rot, p.x));
            }
        });
        best
    }

    pub fn plan(&self, game: &Game) -> Option<Move> {
        if game.game_over {
            return None;
        }
        let rows = board_rows(game);
        let queue = |i: usize| game.queue.get(i).copied();
        let look = |n: Option<PieceKind>| if self.lookahead { n } else { None };

        let mut best = self
            .search(&rows, game.current.kind, look(queue(0)))
            .map(|(s, rot, x)| (s, Move { hold: false, rot, x}));
        if self.use_hold && !game.hold_used {
            let (piece, next) = match game.hold {
                Some(h) => (Some(h), queue(0)),
                None => (queue(0), queue(1)),
            };
            let swap = piece
                .filter(|&p| p != game.current.kind)
                .and_then(|p| self.search(&rows, p, look(next)));
            if let Some((s, rot, x)) = swap
                && best.is_none_or(|(b, _)| s > b)
            {
                best = Some((s, Move { hold: true, rot, x }));
            }
        }
        best.map(|(_, m)| m)
    }
}

pub fn play_game(ai: &Ai, seed: u64, max_pieces: u32) -> Game {
    let mut game = Game::new(seed);
    while !game.game_over && game.pieces < max_pieces {
        let Some(mut mv) = ai.plan(&game) else { break };
        if mv.hold {
            game.hold();
            match ai.plan(&game) {
                Some(m) => mv = m,
                None => break
            }
        }
        game.place(mv.rot, mv.x);
    }
    game
}

#[derive(Debug, Clone)]
pub struct Pilot {
    pub ai: Ai,
    pub step: f32,
    plan: Option<Move>,
    plan_for: Option<(u32, bool)>,
    timer: f32,
    actions: u32
}

impl Pilot {
    pub fn new(ai: Ai, step: f32) -> Self {
        Pilot {
            ai,
            step,
            plan: None,
            plan_for: None,
            timer: 0.0,
            actions: 0
        }
    }

    fn next_action(&self, game: &Game) -> Action {
        let Some(mv) = self.plan else {
            return Action::HardDrop;
        };
        let p = game.current;
        if self.actions > 16 {
            return Action::HardDrop;
        }
        if mv.hold {
            return Action::Hold;
        }
        if p.rot != mv.rot {
            return if (mv.rot + 4 - p.rot) % 4 == 3 {
                Action::RotateCcw
            } else {
                Action::RotateCw
            };
        }
        if p.x < mv.x {
            Action::ShiftRight
        } else if p.x > mv.x {
            Action::ShiftLeft
        } else {
            Action::HardDrop
        }
    }

    pub fn update(&mut self, player: &mut Player, dt: f32, mut apply: impl FnMut(&mut Player, Action)) {
        if player.paused || player.game.game_over {
            return;
        }

        let rush = player.game.gravity_interval() < 4.0 * self.step.max(1.0 / 60.0);
        self.timer += dt;
        let mut dropped = false;
        for _ in 0..64 {
            let game = &player.game;
            if game.game_over {
                break;
            }
            let key = (game.pieces, game.hold_used);
            if self.plan_for != Some(key) {
                self.plan = self.ai.plan(game);
                self.plan_for = Some(key);
                self.actions = 0;
            }
            let action = self.next_action(game);
            if action == Action::HardDrop && dropped {
                break;
            }
            if !(rush && action != Action::HardDrop) {
                if self.timer < self.step {
                    break;
                }
                self.timer -= self.step;
            }
            let before = game.current;
            apply(player, action);
            self.actions += 1;
            dropped |= action == Action::HardDrop;
            if matches!(action, Action::RotateCw | Action::RotateCcw | Action::ShiftLeft | Action::ShiftRight)
                && player.game.current == before
            {
                self.actions = u32::MAX / 2;
            }
        }
        if self.step == 0.0 {
            self.timer = 0.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::player::HeldKeys;

    #[test]
    fn shipped_weights_parse_and_round_trip() {
        let w = Weights::default();
        assert_eq!(Weights::parse(&w.to_text()).unwrap(), w);
    }

    #[test]
    fn bad_weight_files_are_rejected() {
        assert!(Weights::parse("holes 1").is_err()); // missing features
        let mut text = Weights::default().to_text();
        text.push_str("holes 2\n");
        assert!(Weights::parse(&text).is_err()); // duplicate
        assert!(Weights::parse("nonsense 1").is_err());
    }

    #[test]
    fn placements_match_the_engine() {
        // every placement the AI simulates must equal what Game::place does
        let mut setup = Game::new(9);
        setup.place(1, 0);
        setup.place(0, 5);
        setup.place(0, 2);
        let rows = board_rows(&setup);
        let kind = setup.current.kind;
        let mut count = 0;
        placements(&rows, kind, |p| {
            let mut g = setup.clone();
            let lines = g.place(p.rot, p.x);
            assert!(!g.game_over);
            assert_eq!(board_rows(&g), p.rows, "{kind:?} rot {} x {}", p.rot, p.x);
            assert_eq!(lines, p.lines);
            count += 1;
        });
        assert!(count >= 9, "only {count} placements");
    }

    #[test]
    fn features_on_a_known_board() {
        let mut rows = [0u16; HEIGHT];
        rows[HEIGHT - 1] = 0b11_1111_1110; // bottom row, gap at column 0
        let mut f = [0.0; N_FEATURES];
        board_features(&rows, &mut f);
        assert_eq!(f[4], 0.0, "column 0 is open to the top: a well, not a hole");
        assert_eq!(f[5], 1.0, "well of depth 1 next to the wall");
        rows[HEIGHT - 2] = 0b00_0000_0001; // block above the gap
        board_features(&rows, &mut f);
        assert_eq!(f[4], 1.0, "now the gap is a hole");
        assert_eq!(f[6], 1.0);
    }

    #[test]
    fn ai_survives_and_scores() {
        let ai = Ai {
            lookahead: false,
            ..Ai::default()
        };
        let game = play_game(&ai, 1, 500);
        assert!(!game.game_over, "died after {} pieces", game.pieces);
        assert!(game.lines >= 150, "only {} lines", game.lines);
    }

    #[test]
    fn lookahead_and_hold_play_legally() {
        let game = play_game(&Ai::default(), 2, 200);
        assert!(!game.game_over);
        assert_eq!(game.pieces, 200);
    }

    #[test]
    fn pilot_plays_in_real_time() {
        let mut player = Player::new(5);
        let mut pilot = Pilot::new(Ai::default(), 0.02);
        for _ in 0..60 * 60 {
            pilot.update(&mut player, 1.0 / 60.0, |p, a| p.act(a));
            player.update(1.0 / 60.0, HeldKeys::default());
        }
        let g = &player.game;
        assert!(!g.game_over, "pilot died after {} pieces", g.pieces);
        assert!(g.pieces > 100 && g.lines > 30, "{} pieces {} lines", g.pieces, g.lines);
    }

    #[test]
    fn pilot_lands_where_it_planned() {
        let mut player = Player::new(8);
        let ai = Ai {
            use_hold: false,
            ..Ai::default()
        };
        let mut pilot = Pilot::new(ai.clone(), 0.0);
        for _ in 0..40 {
            let mv = ai.plan(&player.game).unwrap();
            let mut expected = player.game.clone();
            expected.place(mv.rot, mv.x);
            let before = player.game.pieces;
            while player.game.pieces == before {
                pilot.update(&mut player, 1.0 / 60.0, |p, a| p.act(a));
            }
            assert_eq!(player.game.board, expected.board);
        }
    }
}