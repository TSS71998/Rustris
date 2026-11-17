use crate::game::Game;

pub const LOCK_DELAY: f32 = 0.5;
pub const MAX_LOCK_RESETS: u32 = 15;
pub const DAS: f32 = 0.170;
pub const ARR: f32 = 0.045;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    RotateCw,
    RotateCcw,
    HardDrop,
    Hold,
    /// One column left/right (used by the AI; humans hold arrow keys).
    ShiftLeft,
    ShiftRight
}

#[derive(Clone, Copy, Debug, Default)]
pub struct HeldKeys {
    pub left: bool,
    pub right: bool,
    pub soft_drop: bool
}

#[derive(Clone, Copy, Debug, Default)]
struct Repeat {
    held_for: Option<f32>
}

impl Repeat {
    fn update(&mut self, down: bool, dt: f32) -> u32 {
        if !down {
            self.held_for = None;
            return 0;
        }
        match self.held_for {
            None => {
                self.held_for = Some(0.0);
                1
            }
            Some(t) => {
                let t2 = t + dt;
                self.held_for = Some(t2);
                let count = |t: f32| {
                    if t < DAS {
                        0
                    } else {
                        ((t - DAS) / ARR) as u32 + 1
                    }
                };
                count(t2) - count(t)
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct Player {
    pub game: Game,
    pub paused: bool,
    fall: f32,
    lock_timer: f32,
    lock_resets: u32,
    left: Repeat,
    right: Repeat,
    down: Repeat
}

impl Player {
    pub fn new(seed: u64) -> Self {
        Player {
            game: Game::new(seed),
            paused: false,
            fall: 0.0,
            lock_timer: 0.0,
            lock_resets: 0,
            left: Repeat::default(),
            right: Repeat::default(),
            down: Repeat::default()
        }
    }

    pub fn toggle_pause(&mut self) {
        if !self.game.game_over {
            self.paused = !self.paused;
        }
    }

    fn active(&self) -> bool {
        !self.paused && !self.game.game_over
    }

    fn moved(&mut self) {
        if self.game.on_ground() && self.lock_resets < MAX_LOCK_RESETS {
            self.lock_timer = 0.0;
            self.lock_resets += 1;
        }
    }

    fn new_piece(&mut self) {
        self.fall = 0.0;
        self.lock_timer = 0.0;
        self.lock_resets = 0;
    }

    pub fn act(&mut self, action: Action) {
        if !self.active() {
            return;
        }
        match action {
            Action::RotateCw => {
                if self.game.rotate(1) {
                    self.moved();
                }
            }
            Action::RotateCcw => {
                if self.game.rotate(-1) {
                    self.moved();
                }
            }
            Action::HardDrop => {
                self.game.hard_drop();
                self.new_piece();
            }
            Action::Hold => {
                if self.game.hold() {
                    self.new_piece();
                }
            }
            Action::ShiftLeft => {
                if self.game.shift(-1) {
                    self.moved();
                }
            }
            Action::ShiftRight => {
                if self.game.shift(1) {
                    self.moved();
                }
            }
        }
    }

    pub fn update(&mut self, dt: f32, keys: HeldKeys) {
        if !self.active() {
            self.left = Repeat::default();
            self.right = Repeat::default();
            self.down = Repeat::default();
            return;
        }

        let (l, r) = (keys.left && !keys.right, keys.right && !keys.left);
        for _ in 0..self.left.update(l, dt) {
            if self.game.shift(-1) {
                self.moved();
            }
        }
        for _ in 0..self.right.update(r, dt) {
            if self.game.shift(1) {
                self.moved();
            }
        }
        for _ in 0..self.down.update(keys.soft_drop, dt) {
            if self.game.soft_drop() {
                self.fall = 0.0;
            }
        }

        if self.game.on_ground() {
            self.lock_timer += dt;
            if self.lock_timer >= LOCK_DELAY {
                self.game.lock();
                self.new_piece();
            }
        } else {
            self.fall += dt;
            let interval = self.game.gravity_interval();
            while self.fall >= interval && self.game.gravity_step() {
                self.fall -= interval;
                self.lock_timer = 0.0;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRAME: f32 = 1.0 / 60.0;

    fn run(p: &mut Player, seconds: f32, keys: HeldKeys) {
        let frames = (seconds / FRAME).round() as u32;
        for _ in 0..frames {
            p.update(FRAME, keys);
        }
    }

    #[test]
    fn gravity_drops_and_locks_on_its_own() {
        let mut p = Player::new(1);
        run(&mut p, 30.0, HeldKeys::default());
        assert!(p.game.pieces >= 1);
    }

    #[test]
    fn tap_moves_exactly_one_column() {
        let mut p = Player::new(1);
        let x = p.game.current.x;
        let keys = HeldKeys {
            left: true,
            ..Default::default()
        };
        p.update(FRAME, keys);
        p.update(FRAME, HeldKeys::default());
        assert_eq!(p.game.current.x, x - 1);
    }

    #[test]
    fn holding_a_direction_reaches_the_wall() {
        let mut p = Player::new(1);
        let keys = HeldKeys {
            right: true,
            ..Default::default()
        };
        run(&mut p, 0.5, keys);
        let max_c = p.game.current.cells().iter().map(|c| c.1).max().unwrap();
        assert_eq!(max_c, crate::game::WIDTH as i32 - 1);
    }

    #[test]
    fn soft_drop_is_faster_than_gravity() {
        let mut a = Player::new(2);
        let mut b = Player::new(2);
        run(&mut a, 0.5, HeldKeys::default());
        let keys = HeldKeys {
            soft_drop: true,
            ..Default::default()
        };
        run(&mut b, 0.5, keys);
        assert!(b.game.current.y > a.game.current.y);
        assert!(b.game.score > 0);
    }

    #[test]
    fn pause_freezes_the_game() {
        let mut p = Player::new(3);
        p.toggle_pause();
        let before = p.game.current;
        run(&mut p, 5.0, HeldKeys::default());
        p.act(Action::HardDrop);
        assert_eq!(p.game.current, before);
        assert_eq!(p.game.pieces, 0);
    }

    #[test]
    fn lock_delay_waits_before_locking() {
        let mut p = Player::new(4);
        // drop to the floor without locking
        while p.game.gravity_step() {}
        run(&mut p, LOCK_DELAY * 0.5, HeldKeys::default());
        assert_eq!(p.game.pieces, 0);
        run(&mut p, LOCK_DELAY, HeldKeys::default());
        assert_eq!(p.game.pieces, 1);
    }
}