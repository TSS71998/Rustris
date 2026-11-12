use std::collections::VecDeque;

use crate::rng::Rng;

pub const WIDTH: usize = 10;
pub const HEIGHT: usize = 20;

pub type Row = [u8; WIDTH];
pub type Board = [Row; HEIGHT];

pub const LINE_SCORES: [u32; 5] = [0, 100, 300, 500, 800];

const KICKS: [(i32, i32); 8] = [
    (0, 0),
    (-1, 0),
    (1, 0),
    (-2, 0),
    (2, 0),
    (0, -1),
    (-1, -1),
    (1, -1),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PieceKind {
    I,
    O,
    T,
    S,
    Z,
    J,
    L
}

impl PieceKind {
    pub const ALL: [PieceKind; 7] = [
        PieceKind::I,
        PieceKind::O,
        PieceKind::T,
        PieceKind::S,
        PieceKind::Z,
        PieceKind::J,
        PieceKind::L,
    ];

    pub fn id(self) -> u8 {
        self as u8 + 1
    }

    pub fn from_id(id: u8) -> Option<PieceKind> {
        PieceKind::ALL.get((id as usize).checked_sub(1)?).copied()
    }

    fn shape(self) -> &'static [&'static str] {
        match self {
            PieceKind::I => &["....", "XXXX", "....", "...."],
            PieceKind::O => &["XX", "XX"],
            PieceKind::T => &[".X.", "XXX", "..."],
            PieceKind::S => &[".XX", "XX.", "..."],
            PieceKind::Z => &["XX.", ".XX", "..."],
            PieceKind::J => &["X..", "XXX", "..."],
            PieceKind::L => &["..X", "XXX", "..."],
        }
    }

    pub fn box_size(self) -> i32 {
        self.shape().len() as i32
    }

    /// The four (row, col) cells of this piece in rotation `rot` (0..4,
    /// clockwise), relative to the top-left of its rotation box.
    pub fn cells(self, rot: u8) -> [(i32, i32); 4] {
        let n = self.box_size();
        let mut cells = [(0, 0); 4];
        let mut i = 0;
        for (r, line) in self.shape().iter().enumerate() {
            for (c, ch) in line.chars().enumerate() {
                if ch == 'X' {
                    cells[i] = (r as i32, c as i32);
                    i += 1;
                }
            }
        }
        for _ in 0..rot % 4 {
            for cell in cells.iter_mut() {
                let (r, c) = *cell;
                *cell = (c, n - 1 - r);
            }
        }
        cells
    }

    pub fn name(self) -> char {
        "IOTSZJL".as_bytes()[self as usize] as char
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Piece {
    pub kind: PieceKind,
    pub rot: u8,
    pub x: i32,
    pub y: i32
}

impl Piece {
    pub fn cells(&self) -> [(i32, i32); 4] {
        self.kind.cells(self.rot).map(|(r, c)| (self.y + r, self.x + c))
    }
}

pub fn spawn_position(kind: PieceKind, rot: u8) -> (i32, i32) {
    let min_r = kind.cells(rot).iter().map(|&(r, _)| r).min().unwrap();
    ((WIDTH as i32 - kind.box_size()) / 2, -min_r) 
}

#[derive(Clone, Debug)]
pub struct Game {
    pub board: Board,
    pub current: Piece,
    pub queue: VecDeque<PieceKind>,
    pub hold: Option<PieceKind>,
    pub hold_used: bool,
    pub score: u32,
    pub lines: u32,
    pub level: u32,
    pub pieces: u32,
    pub tetrises: u32,
    pub last_clear: u32,
    pub cleared_rows: [usize; 4],
    pub cleared_cells: [Row; 4],
    pub game_over: bool,
    rng: Rng,
    bag: Vec<PieceKind>
}

impl Game {
    pub const QUEUE_SIZE: usize = 5;

    pub fn new(seed: u64) -> Self {
        let mut game = Game {
            board: [[0; WIDTH]; HEIGHT],
            current: Piece {
                kind: PieceKind::I,
                rot: 0,
                x: 0,
                y: 0
            },
            queue: VecDeque::with_capacity(Self::QUEUE_SIZE + 1),
            hold: None,
            hold_used: false,
            score: 0,
            lines: 0,
            level: 1,
            pieces: 0,
            tetrises: 0,
            last_clear: 0,
            cleared_rows: [0; 4],
            cleared_cells: [[0; WIDTH]; 4],
            game_over: false,
            rng: Rng::new(seed),
            bag: Vec::with_capacity(7),
        };
        
        for _ in 0..Self::QUEUE_SIZE {
            let next = game.next_from_bag();
            game.queue.push_back(next);
        }

        game.spawn(None);
        game
    }

    fn next_from_bag(&mut self) -> PieceKind {
        if self.bag.is_empty() {
            self.bag.extend_from_slice(&PieceKind::ALL);
            self.rng.shuffle(&mut self.bag);
        }
        self.bag.pop().unwrap()
    }

    fn spawn(&mut self, kind: Option<PieceKind>) {
        let kind = kind.unwrap_or_else(|| {
            let next = self.next_from_bag();
            self.queue.push_back(next);
            self.queue.pop_front().unwrap()
        });

        let (x, y) = spawn_position(kind, 0);
        self.current = Piece { kind, rot: 0, x, y };
        if !self.fits(kind, 0, x, y) {
            self.game_over = true;
        }
    }

    pub fn fits(&self, kind: PieceKind, rot: u8, x: i32, y: i32) -> bool {
        kind.cells(rot).iter().all(|&(r, c)| {
            let (rr, cc) = (y + r, x + c);
            if cc < 0 || cc >= WIDTH as i32 || rr >= HEIGHT as i32 {
                return false;
            }
            rr < 0 || self.board[rr as usize][cc as usize] == 0
        })
    }

    fn piece_fits(&self, p: Piece) -> bool {
        self.fits(p.kind, p.rot, p.x, p.y)
    }

    pub fn shift(&mut self, dx: i32) -> bool {
        let moved = Piece {
            x: self.current.x + dx,
            ..self.current
        };
        if !self.game_over && self.piece_fits(moved) {
            self.current = moved;
            true
        } else {
            false
        }
    }

    pub fn rotate(&mut self, dir: i32) -> bool {
        if self.game_over {
            return false;
        }
        let p = self.current;
        let rot = (p.rot as i32 + dir).rem_euclid(4) as u8;
        for (dx, dy) in KICKS {
            let cand = Piece {
                rot,
                x: p.x + dx,
                y: p.y + dy,
                ..p
            };
            if self.piece_fits(cand) {
                self.current = cand;
                return true;
            }
        }
        false
    }

    pub fn on_ground(&self) -> bool {
        let below = Piece {
            y: self.current.y + 1,
            ..self.current
        };
        !self.piece_fits(below)
    }

    pub fn gravity_step(&mut self) -> bool {
        if self.game_over || self.on_ground() {
            return false;
        }
        self.current.y += 1;
        true
    }

    pub fn soft_drop(&mut self) -> bool {
        let moved = self.gravity_step();
        if moved {
            self.score += 1;
        }
        moved
    }

    pub fn ghost_y(&self) -> i32 {
        let p = self.current;
        let mut y = p.y;
        while self.fits(p.kind, p.rot, p.x, y + 1) {
            y += 1;
        }
        y
    }

    pub fn hard_drop(&mut self) -> u32 {
        if self.game_over {
            return 0;
        }
        let y = self.ghost_y();
        self.score += 2 * (y - self.current.y) as u32;
        self.current.y = y;
        self.lock()
    }

    pub fn hold(&mut self) -> bool {
        if self.game_over || self.hold_used {
            return false;
        }
        let kind = self.current.kind;
        let swap = self.hold.replace(kind);
        self.spawn(swap);
        self.hold_used = true;
        true
    }

    pub fn lock(&mut self) -> u32 {
        let p = self.current;
        let cells = p.cells();
        if cells.iter().any(|&(r, _)| r < 0) {
            self.game_over = true;
            return 0;
        }
        
        for (r, c) in cells {
            self.board[r as usize][c as usize] = p.kind.id();
        }

        let mut kept = [[0u8; WIDTH]; HEIGHT];
        let mut dst = HEIGHT;
        let mut n_full = 0;
        for (r, row) in self.board.iter().enumerate().rev() {
            if row.contains(&0) {
                dst -= 1;
                kept[dst] = *row;
            } else {
                self.cleared_rows[n_full] = r;
                self.cleared_cells[n_full] = *row;
                n_full += 1;
            }
        }
        let n = dst as u32;
        self.board = kept;

        self.last_clear = n;
        self.score += LINE_SCORES[n as usize] * self.level;
        self.lines += n;
        if n == 4 {
            self.tetrises += 1;
        }
        self.level = self.lines / 10 + 1;
        self.pieces += 1;
        self.hold_used = false;
        self.spawn(None);
        n
    }

    pub fn place(&mut self, rot: u8, x: i32) -> u32 {
        let kind = self.current.kind;
        let (_, y) = spawn_position(kind, rot);
        if !self.fits(kind, rot, x, y) {
            self.game_over = true;
            return 0;
        }
        self.current = Piece { kind, rot, x, y };
        self.hard_drop()
    }

    pub fn gravity_interval(&self) -> f32 {
        let lvl = self.level.min(20) as f32;
        (0.8 - (lvl - 1.0) * 0.007).powf(lvl - 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fill_row(game: &mut Game, r: usize, except: &[usize]) {
        for c in 0..WIDTH {
            game.board[r][c] = if except.contains(&c) { 0 } else { 9 };
        }
    }

    #[test]
    fn every_rotation_has_four_cells_in_its_box() {
        for kind in PieceKind::ALL {
            for rot in 0..4 {
                let cells = kind.cells(rot);
                let n = kind.box_size();
                for (r, c) in cells {
                    assert!((0..n).contains(&r) && (0..n).contains(&c));
                }
                let mut sorted = cells.to_vec();
                sorted.sort();
                sorted.dedup();
                assert_eq!(sorted.len(), 4, "{kind:?} rot {rot}");
            }
            // four clockwise turns get back to the start
            let mut a = kind.cells(0).to_vec();
            let mut b = kind.cells(4).to_vec();
            a.sort();
            b.sort();
            assert_eq!(a, b);
        }
    }

    #[test]
    fn ids_round_trip() {
        for kind in PieceKind::ALL {
            assert_eq!(PieceKind::from_id(kind.id()), Some(kind));
        }
        assert_eq!(PieceKind::from_id(0), None);
        assert_eq!(PieceKind::from_id(8), None);
    }

    #[test]
    fn seven_bag_gives_each_piece_once_per_bag() {
        let mut g = Game::new(42);
        // new() already drew 6 pieces (5 queued + the current one); start
        // checking from a fresh bag.
        assert_eq!(g.bag.len(), 1);
        g.next_from_bag();
        for _ in 0..5 {
            let mut bag: Vec<_> = (0..7).map(|_| g.next_from_bag()).collect();
            bag.sort_by_key(|k| k.id());
            assert_eq!(bag, PieceKind::ALL.to_vec());
        }
    }

    #[test]
    fn same_seed_same_pieces() {
        let a = Game::new(7);
        let b = Game::new(7);
        assert_eq!(a.current, b.current);
        assert_eq!(a.queue, b.queue);
    }

    #[test]
    fn spawns_centred_on_top_row() {
        for kind in PieceKind::ALL {
            let (x, y) = spawn_position(kind, 0);
            let p = Piece { kind, rot: 0, x, y };
            assert_eq!(p.cells().iter().map(|c| c.0).min(), Some(0));
        }
    }

    #[test]
    fn walls_block_movement() {
        let mut g = Game::new(1);
        while g.shift(-1) {}
        assert_eq!(g.current.cells().iter().map(|c| c.1).min(), Some(0));
        while g.shift(1) {}
        assert_eq!(
            g.current.cells().iter().map(|c| c.1).max(),
            Some(WIDTH as i32 - 1)
        );
    }

    #[test]
    fn hard_drop_lands_on_floor_and_scores() {
        let mut g = Game::new(3);
        let start_y = g.current.y;
        let ghost = g.ghost_y();
        let kind = g.current.kind;
        g.hard_drop();
        assert_eq!(g.score, 2 * (ghost - start_y) as u32);
        assert_eq!(g.pieces, 1);
        let filled = g.board[HEIGHT - 1].iter().filter(|&&v| v != 0).count();
        assert!(filled > 0);
        assert!(g.board[HEIGHT - 1].iter().all(|&v| v == 0 || v == kind.id()));
    }

    #[test]
    fn clearing_four_lines_is_a_tetris() {
        let mut g = Game::new(0);
        for r in HEIGHT - 4..HEIGHT {
            fill_row(&mut g, r, &[0]);
        }
        g.board[HEIGHT - 5][5] = 9; // something that must shift down
        // Vertical I piece into column 0.
        g.current = Piece {
            kind: PieceKind::I,
            rot: 1,
            x: -2,
            y: 0,
        };
        assert!(g.piece_fits(g.current));
        let cleared = g.hard_drop();
        assert_eq!(cleared, 4);
        assert_eq!(g.tetrises, 1);
        assert_eq!(g.lines, 4);
        assert_eq!(g.board[HEIGHT - 1][5], 9);
        assert_eq!(g.board[HEIGHT - 1].iter().filter(|&&v| v != 0).count(), 1);
    }

    #[test]
    fn level_and_score_follow_lines() {
        let mut g = Game::new(0);
        g.lines = 9;
        fill_row(&mut g, HEIGHT - 1, &[0, 1, 2, 3]);
        g.current = Piece {
            kind: PieceKind::I,
            rot: 0,
            x: 0,
            y: 0,
        };
        let before = g.score;
        let drop = (g.ghost_y() - g.current.y) as u32;
        assert_eq!(g.hard_drop(), 1);
        assert_eq!(g.level, 2);
        assert_eq!(g.score - before, 2 * drop + 100);
    }

    #[test]
    fn hold_swaps_once_per_piece() {
        let mut g = Game::new(5);
        let first = g.current.kind;
        let second = g.queue[0];
        assert!(g.hold());
        assert_eq!(g.hold, Some(first));
        assert_eq!(g.current.kind, second);
        assert!(!g.hold(), "hold twice in a row is not allowed");
        g.hard_drop();
        assert!(g.hold());
        assert_eq!(g.current.kind, first);
    }

    #[test]
    fn rotation_kicks_off_the_wall() {
        let mut g = Game::new(0);
        g.current = Piece {
            kind: PieceKind::I,
            rot: 1,
            x: -2,
            y: 5,
        };
        assert!(g.piece_fits(g.current));
        assert!(g.rotate(1), "should kick right instead of failing");
        assert!(g.current.cells().iter().all(|&(_, c)| c >= 0));
    }

    #[test]
    fn stacking_to_the_top_ends_the_game() {
        let mut g = Game::new(9);
        for _ in 0..200 {
            if g.game_over {
                break;
            }
            g.hard_drop();
        }
        assert!(g.game_over);
        assert!(!g.shift(1) && !g.rotate(1) && g.hard_drop() == 0);
    }

    #[test]
    fn place_matches_manual_drop() {
        let mut a = Game::new(11);
        let mut b = a.clone();
        a.place(1, 6);
        let kind = b.current.kind;
        let (_, y) = spawn_position(kind, 1);
        b.current = Piece {
            kind,
            rot: 1,
            x: 6,
            y,
        };
        b.hard_drop();
        assert_eq!(a.board, b.board);
    }

    #[test]
    fn gravity_speeds_up() {
        let mut g = Game::new(0);
        let slow = g.gravity_interval();
        assert!((slow - 1.0).abs() < 1e-6);
        g.level = 10;
        assert!(g.gravity_interval() < slow);
    }

    #[test]
    fn remembers_which_rows_were_cleared() {
        let mut g = Game::new(0);

        fill_row(&mut g, HEIGHT - 3, &[0]);
        fill_row(&mut g, HEIGHT - 2, &[0, 5]);
        fill_row(&mut g, HEIGHT - 1, &[0]);
        g.current = Piece {
            kind: PieceKind::I,
            rot: 1,
            x: -2,
            y: 0,
        };
        assert_eq!(g.hard_drop(), 2);
        assert_eq!(&g.cleared_rows[..2], &[HEIGHT - 1, HEIGHT - 3]);
        assert!(g.cleared_cells[0].iter().all(|&v| v != 0));
        assert_eq!(g.cleared_cells[0][0], PieceKind::I.id());
    }
}