//! Attack values follow the Tetris guideline (no T-spins here):
//! single 0, double 1, triple 2, Tetris 4, +1 for back-to-back Tetrises,
//! a combo bonus for clearing on consecutive pieces, and 10 for clearing
//! the whole board.

use crate::ai::{Difficulty, Pilot};
use crate::game::WIDTH;
use crate::player::{HeldKeys, Player};
use crate::rng::Rng;

pub const GARBAGE_DELAY: f32 = 0.6;

pub const GARBAGE_CAP: u32 = 8;

const COMBO_BONUS: [u32; 12] = [0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 4, 5];

pub fn attack_lines(lines: u32, combo: u32, b2b: bool, perfect: bool) -> u32 {
    if lines == 0 {
        return 0;
    }
    if perfect {
        return  10;
    }
    let base = [0, 0, 1, 2, 4][lines.min(4) as usize];
    let combo = COMBO_BONUS[(combo.max(1) as usize - 1).min(COMBO_BONUS.len() - 1)];
    base + combo + (b2b && lines >= 4) as u32
}

#[derive(Debug, Clone, PartialEq)]
pub struct Incoming {
    pub lines: u32,
    pub hole: usize,
    pub timer: f32
}

#[derive(Debug, Clone)]
pub struct Fighter {
    pub player: Player,
    pub incoming: Vec<Incoming>,
    pub combo: u32,
    pub b2b: bool,
    pub sent: u32,
    pub received: u32,
    last_pieces: u32,
    rng: Rng
}

impl Fighter {
    fn new(seed: u64, garbage_seed: u64) -> Self {
        Fighter {
            player: Player::new(seed),
            incoming: Vec::new(),
            combo: 0,
            b2b: false,
            sent: 0,
            received: 0,
            last_pieces: 0,
            rng: Rng::new(garbage_seed)
        }
    }

    pub fn pending(&self) -> u32 {
        self.incoming.iter().map(|g| g.lines).sum()
    }

    pub fn ready(&self) -> u32 {
        self.incoming.iter().filter(|g| g.timer <= 0.0).map(|g| g.lines).sum()
    }

    fn cancel(&mut self, mut lines: u32) -> u32 {
        while lines > 0 {
            let Some(first) = self.incoming.first_mut() else { break };
            let k = first.lines.min(lines);
            first.lines -= k;
            lines -= k;
            if first.lines == 0 {
                self.incoming.remove(0);
            }
        }
        lines
    }

    fn take_garbage(&mut self) -> u32 {
        let mut budget = GARBAGE_CAP;
        let mut added = 0;
        while budget > 0 {
            let Some(first) = self.incoming.first_mut() else { break };
            if first.timer > 0.0 {
                break;
            }
            let k = first.lines.min(budget);
            self.player.game.add_garbage(k as usize, first.hole);
            first.lines -= k;
            budget -= k;
            added += k;
            if first.lines == 0 {
                self.incoming.remove(0);
            }
        }
        self.received += added;
        added
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Attack { from: usize, sent: u32, cancelled: u32},
    Garbage { who: usize, lines: u32 },
    KnockOut { winner: Option<usize> }
}

#[derive(Debug, Clone)]
pub struct Match {
    pub fighters: [Fighter; 2],
    pub result: Option<Option<usize>>,
    pub time: f32
}

impl Match {
    pub fn new(seed: u64) -> Self {
        Match {
            fighters: [
                Fighter::new(seed, seed ^ 0x6a09_e667),
                Fighter::new(seed, seed ^ 0xbb67_ae85),
            ],
            result: None,
            time: 0.0
        }
    }

    pub fn is_over(&self) -> bool {
        self.result.is_some()
    }

    pub fn resolve(&mut self, dt: f32) -> Vec<Event> {
        let mut events = Vec::new();
        if self.is_over() {
            return events;
        }
        self.time += dt;
        for f in &mut self.fighters {
            for g in &mut f.incoming {
                g.timer -= dt;
            }
        }

        for i in 0..2 {
            let f = &mut self.fighters[i];
            let game = &f.player.game;
            if game.pieces == f.last_pieces {
                continue;
            }
            f.last_pieces = game.pieces;
            let lines = game.last_clear;
            if lines > 0 {
                f.combo += 1;
                let b2b = lines == 4 && f.b2b;
                f.b2b = lines == 4;
                let attack = attack_lines(lines, f.combo, b2b, game.board_empty());
                let sent = f.cancel(attack);
                let cancelled = attack - sent;
                f.sent += sent;
                if sent > 0 {
                    let other = &mut self.fighters[1 - i];
                    let hole = other.rng.below(WIDTH);
                    other.incoming.push(Incoming {
                        lines: sent,
                        hole,
                        timer: GARBAGE_DELAY
                    });
                }
                if attack > 0 {
                    events.push(Event::Attack { from: i, sent: sent, cancelled });
                }
            } else {
                f.combo = 0;
                let added = f.take_garbage();
                if added > 0 {
                    events.push(Event::Garbage { who: i, lines: added });
                }
            }
        }

        let dead = [0, 1].map(|i| self.fighters[i].player.game.game_over);
        if dead[0] || dead[1] {
            let winner = match dead {
                [true, true] => None,
                [true, false] => Some(1),
                _ => Some(0)
            };
            self.result = Some(winner);
            events.push(Event::KnockOut { winner });
        }
        events
    }
}

pub fn simulate(a: Difficulty, b: Difficulty, seed: u64, max_secs: f32) -> Match {
    let mut m = Match::new(seed);
    let mut pilots: [Pilot; 2] = [a.pilot(seed ^ 11), b.pilot(seed ^ 22)];
    let dt = 1.0 / 60.0;
    while !m.is_over() && m.time < max_secs {
        for (f, pilot) in m.fighters.iter_mut().zip(&mut pilots) {
            pilot.update(&mut f.player, dt, |p, act| p.act(act));
            f.player.update(dt, HeldKeys::default());
        }
        m.resolve(dt);
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{HEIGHT, Piece, PieceKind};

    #[test]
    fn attack_table() {
        assert_eq!(attack_lines(1, 1, false, false), 0);
        assert_eq!(attack_lines(2, 1, false, false), 1);
        assert_eq!(attack_lines(3, 1, false, false), 2);
        assert_eq!(attack_lines(4, 1, false, false), 4);
        assert_eq!(attack_lines(4, 1, true, false), 5, "back-to-back Tetris");
        assert_eq!(attack_lines(1, 4, false, false), 1, "combo bonus");
        assert_eq!(attack_lines(2, 30, false, false), 1 + 5, "combo bonus caps");
        assert_eq!(attack_lines(1, 1, false, true), 10, "perfect clear");
        assert_eq!(attack_lines(0, 5, true, true), 0);
    }

    /// Make fighter `i` clear `n` lines with a vertical I piece.
    fn clear_lines(m: &mut Match, i: usize, n: usize) {
        let g = &mut m.fighters[i].player.game;
        for r in HEIGHT - n..HEIGHT {
            g.board[r] = [3; WIDTH];
            g.board[r][0] = 0;
        }
        g.board[HEIGHT - n - 1][5] = 3; // keep it from being a perfect clear
        g.current = Piece { kind: PieceKind::I, rot: 1, x: -2, y: 0 };
        assert_eq!(g.hard_drop() as usize, n);
    }

    #[test]
    fn tetris_sends_four_lines_that_arrive_after_a_delay() {
        let mut m = Match::new(1);
        clear_lines(&mut m, 0, 4);
        let ev = m.resolve(0.016);
        assert_eq!(ev, vec![Event::Attack { from: 0, sent: 4, cancelled: 0 }]);
        assert_eq!(m.fighters[1].pending(), 4);
        assert_eq!(m.fighters[1].ready(), 0);

        // not ready yet: a placement doesn't let it in
        m.fighters[1].player.game.hard_drop();
        assert!(m.resolve(0.016).is_empty());
        assert_eq!(m.fighters[1].pending(), 4);

        // after the delay it rises on the next placement
        m.resolve(GARBAGE_DELAY);
        m.fighters[1].player.game.hard_drop();
        let ev = m.resolve(0.016);
        assert_eq!(ev, vec![Event::Garbage { who: 1, lines: 4 }]);
        assert_eq!(m.fighters[1].pending(), 0);
        assert_eq!(m.fighters[1].received, 4);
        let bottom = m.fighters[1].player.game.board[HEIGHT - 1];
        assert_eq!(bottom.iter().filter(|&&v| v == crate::game::GARBAGE).count(), WIDTH - 1);
    }

    #[test]
    fn clearing_cancels_incoming_garbage_first() {
        let mut m = Match::new(2);
        m.fighters[0].incoming.push(Incoming { lines: 3, hole: 0, timer: 1.0 });
        clear_lines(&mut m, 0, 4); // attack 4: cancels 3, sends 1
        let ev = m.resolve(0.016);
        assert_eq!(ev, vec![Event::Attack { from: 0, sent: 1, cancelled: 3 }]);
        assert_eq!(m.fighters[0].pending(), 0);
        assert_eq!(m.fighters[1].pending(), 1);
    }

    #[test]
    fn back_to_back_and_combo_are_tracked() {
        let mut m = Match::new(3);
        clear_lines(&mut m, 0, 4);
        m.resolve(0.016);
        clear_lines(&mut m, 0, 4);
        m.resolve(0.016);
        // 4 + (4 + 1 b2b + 0 combo-2 bonus)
        assert_eq!(m.fighters[0].sent, 4 + 5);
        assert_eq!(m.fighters[0].combo, 2);
        m.fighters[0].player.game.hard_drop(); // no clear: combo breaks
        m.resolve(0.016);
        assert_eq!(m.fighters[0].combo, 0);
    }

    #[test]
    fn at_most_eight_rows_enter_per_piece() {
        let mut m = Match::new(4);
        m.fighters[1].incoming.push(Incoming { lines: 12, hole: 2, timer: 0.0 });
        m.fighters[1].player.game.hard_drop();
        m.resolve(0.016);
        assert_eq!(m.fighters[1].received, GARBAGE_CAP);
        assert_eq!(m.fighters[1].pending(), 12 - GARBAGE_CAP);
    }

    #[test]
    fn topping_out_ends_the_match() {
        let mut m = Match::new(5);
        m.fighters[1].player.game.board[0][0] = 1;
        m.fighters[1].incoming.push(Incoming { lines: 2, hole: 3, timer: 0.0 });
        m.fighters[1].player.game.hard_drop();
        let ev = m.resolve(0.016);
        assert!(ev.contains(&Event::KnockOut { winner: Some(0) }));
        assert!(m.is_over());
        assert!(m.resolve(0.016).is_empty(), "nothing happens after the end");
    }

    #[test]
    fn expert_beats_easy() {
        let mut wins = 0;
        for seed in 0..3 {
            let m = simulate(Difficulty::Pain, Difficulty::Easy, seed, 300.0);
            assert!(m.is_over(), "seed {seed}: no winner after {:.0}s", m.time);
            if m.result == Some(Some(0)) {
                wins += 1;
            }
        }
        assert_eq!(wins, 3);
    }
}