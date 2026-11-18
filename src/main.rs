mod ascii;
mod draw;

use macroquad::prelude::*;

use ascii::Ascii;
use draw::*;
use rustris::ai::{Ai, Difficulty, Pilot};
use rustris::effects::{Effects, Rgb, piece_rgb};
use rustris::game::{Game, HEIGHT, WIDTH};
use rustris::player::{Action, HeldKeys, Player};
use rustris::versus::{Event, Fighter, Match};

const GRID_COLS: usize = 178;
const GRID_ROWS: usize = 72;

const SOLO_COLS: usize = 112;
const BX: i32 = 31;
const BY: i32 = 9;
const LEFT: (i32, i32) = (2, 28);
const RIGHT: (i32, i32) = (83, 109);

const VS_COLS: usize = 178;
const VS_BX: [i32; 2] = [31, 97];
const VS_PANEL: [(i32, i32); 2] = [(2, 26), (151, 175)];
const VS_METER: [i32; 2] = [28, 148];
const VS_CENTER: f32 = 89.0;

const PRESSES: [(KeyCode, Action); 7] = [
    (KeyCode::Up, Action::RotateCw),
    (KeyCode::X, Action::RotateCw),
    (KeyCode::Z, Action::RotateCcw),
    (KeyCode::Space, Action::HardDrop),
    (KeyCode::C, Action::Hold),
    (KeyCode::LeftShift, Action::Hold),
    (KeyCode::RightShift, Action::Hold),
];

const SOLO_HELP: &[(&str, &str)] = &[
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
    ("A", "AI on/off"),
    ("S", "AI speed"),
    ("ESC", "menu"),
];

const AI_SPEEDS: [(&str, f32); 3] = [("normal", 0.06), ("fast", 0.015), ("max", 0.0)];

fn apply(player: &mut Player, fx: &mut Effects, action: Action) {
    let game = &player.game;
    let active = !player.paused && !game.game_over;
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

fn human_input(player: &mut Player, fx: &mut Effects) -> HeldKeys {
    for (key, action) in PRESSES {
        if is_key_pressed(key) {
            apply(player, fx, action);
        }
    }
    HeldKeys {
        left: is_key_down(KeyCode::Left),
        right: is_key_down(KeyCode::Right),
        soft_drop: is_key_down(KeyCode::Down),
    }
}

fn clear_effects(fx: &mut Effects, game: &Game, before: (u32, u32)) {
    if game.pieces != before.0 && game.last_clear > 0 {
        let n = game.last_clear as usize;
        let rows: Vec<_> = (0..n)
            .map(|i| (game.cleared_rows[i], game.cleared_cells[i]))
            .collect();
        fx.line_clear(&rows);
    }
    if game.level > before.1 {
        fx.label("LEVEL UP", fx.geom.y + 6.0, [1.0, 0.85, 0.45], 1.6);
    }
}

fn new_seed() -> u64 {
    (macroquad::miniquad::date::now() * 1000.0) as u64
}

struct Menu {
    choice: usize
}

enum MenuAction {
    Stay,
    Start(usize),
    Quit
}

impl Menu {
    fn update(&mut self, difficulty: &mut usize) -> MenuAction {
        if is_key_pressed(KeyCode::Up) || is_key_pressed(KeyCode::Down) {
            self.choice = 1 - self.choice;
        }
        let n = Difficulty::ALL.len();
        if is_key_pressed(KeyCode::Left) {
            *difficulty = (*difficulty + n - 1) % n;
            self.choice = 1;
        }
        if is_key_pressed(KeyCode::Right) {
            *difficulty = (*difficulty + n + 1) % n;
            self.choice = 1;
        }
        if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::KpEnter) || is_key_pressed(KeyCode::Space) {
            return MenuAction::Start(self.choice);
        }
        if is_key_pressed(KeyCode::Escape) {
            return MenuAction::Quit;
        }
        MenuAction::Stay
    }

    fn draw(&self, a: &mut Ascii, difficulty: Difficulty, time: f32) {
        let v = View::fit(VS_COLS, GRID_ROWS, Vec2::ZERO);
        a.begin_scene();
        let cx = VS_CENTER;
        banner(a, &v, 1.0, "RUSTRIS", cx, 12.0, 100.0, time, 1.0, 0.0, title_tint(time));

        let (x0, x1, y0, y1) = (cx as i32 - 32, cx as i32 + 32, 24, 52);
        v.panel(x0, y0, x1, y1);
        frame(a, x0, y0, x1, y1, None);
        let pulse = 0.75 + 0.25 * (time * 5.0).sin();
        let options = [
            ("SOLO".to_string(), "endless marathon  -  press A to let the AI play".to_string()),
            (
                format!("VS AI      <  {:^6}  >", difficulty.name()),
                difficulty.description().to_string(),
            ),
        ];
        for (i, (title, desc)) in options.iter().enumerate() {
            let y = 29 + i as i32 * 7;
            let selected = i == self.choice;
            let (c, marker) = if selected {
                (scale([0.55, 1.0, 0.7], pulse), "> ")
            } else {
                (DIM, "  ")
            };
            a.text(x0 + 6, y, &format!("{marker}{title}"), c);
            a.text(x0 + 8, y + 2, desc, if selected { LABEL } else { DIM });
        }
        text_centered(a, cx, 45, "UP/DOWN  choose      LEFT/RIGHT  difficulty", LABEL);
        text_centered(a, cx, 47, "ENTER  start", TEXT);
        #[cfg(not(target_arch = "wasm32"))]
        text_centered(a, cx, 50, "ESC  quit", DIM);
        a.present(v.origin(), v.cell(), time, 0.0);
    }
}

struct Solo {
    player: Player,
    fx: Effects,
    over_for: Option<f32>,
    pilot: Option<Pilot>,
    ai_speed: usize
}

impl Solo {
    fn new() -> Self {
        Solo {
            player: Player::new(new_seed()),
            fx: Effects::new(geom(BX, BY), new_seed() ^ 0x5eed),
            over_for: None,
            pilot: None,
            ai_speed: 0
        }
    }

    fn update(&mut self, dt: f32) {
        if is_key_pressed(KeyCode::R) {
            let (pilot, speed) = (self.pilot.is_some(), self.ai_speed);
            *self = Solo::new();
            self.ai_speed = speed;
            if pilot {
                self.pilot = Some(Pilot::new(Ai::default(), AI_SPEEDS[speed].1));
            }
        }
        if is_key_pressed(KeyCode::A) {
            self.pilot = match self.pilot {
                Some(_) => None,
                None => Some(Pilot::new(Ai::default(), AI_SPEEDS[self.ai_speed].1)),
            };
        }
        if is_key_pressed(KeyCode::S) {
            self.ai_speed = (self.ai_speed + 1) % AI_SPEEDS.len();
            if let Some(p) = self.pilot.as_mut() {
                p.step = AI_SPEEDS[self.ai_speed].1;
            }
        }
        if is_key_pressed(KeyCode::P) {
            self.player.toggle_pause();
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
                self.fx.line_clear(&rows);
            }
        }

        let before = (self.player.game.pieces, self.player.game.level);
        let keys = if let Some(p) = self.pilot.as_mut() {
            let fx = &mut self.fx;
            p.update(&mut self.player, dt, |pl, action| apply(pl, fx, action));
            HeldKeys::default()
        } else {
            human_input(&mut self.player, &mut self.fx)
        };
        self.player.update(dt, keys);

        let game = &self.player.game;
        clear_effects(&mut self.fx, game, before);
        if game.game_over && self.over_for.is_none() {
            self.fx.game_over(&game.board);
            self.over_for = Some(0.0);
        }
        if let Some(t) = self.over_for.as_mut() {
            *t += dt;
        }
        self.fx.update(dt);
    }

    fn draw(&self, a: &mut Ascii, time: f32) {
        let game = &self.player.game;
        let fx = &self.fx;
        let v = View::fit(SOLO_COLS, GRID_ROWS, View::shake(fx.shake));
        a.begin_scene();
        let by1 = BY + BOARD_H;

        v.panel(LEFT.0, BY - 1, RIGHT.1, by1);
        board(a, &v, game, BX, BY, time, self.over_for);
        fx_scene(&v, fx);

        frame(a, LEFT.0, BY - 1, LEFT.1, by1, None);
        frame(a, RIGHT.0, BY - 1, RIGHT.1, by1, None);

        let lx = LEFT.0 + 2;
        hold_box(a, &v, game, lx, LEFT.1 - 2, BY + 1);
        for (i, (key, what)) in SOLO_HELP.iter().enumerate() {
            let y = BY + 14 + i as i32 * 2;
            a.text(lx, y, key, LABEL);
            a.text(lx + 12, y, what, DIM);
        }

        let rx = RIGHT.0 + 2;
        next_box(a, &v, game, rx, RIGHT.1 - 2, BY + 1, 5);
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
        match self.pilot {
            Some(_) => {
                let pulse = 0.75 + 0.25 * (time * 4.0).sin();
                let speed = AI_SPEEDS[self.ai_speed].0;
                a.text(rx, by1 - 4, &format!("AI playing ({speed})"), scale([0.55, 1.0, 0.7], pulse));
            }
            None => a.text(rx, by1 - 4, "AI: off  (press A)", DIM),
        }

        let center = (BX + BOARD_W / 2) as f32;
        banner(a, &v, 1.0, "RUSTRIS", center, 1.0, 100.0, time, 1.0, 0.0, title_tint(time));

        fx_glyphs(a, fx);
        fx_labels(a, &v, fx, time);
        if let Some(t) = self.over_for.filter(|&t| t > 0.9) {
            let lines = [
                format!("score {}", group_thousands(game.score)),
                "R to restart".into(),
            ];
            overlay(a, &v, BX, BY, "GAME OVER", [0.95, 0.85, 0.6], &lines, time, t - 0.9);
        } else if self.player.paused {
            overlay(a, &v, BX, BY, "PAUSED", [0.95, 0.85, 0.6], &["P to resume".into()], time, 1.0);
        }
        a.present(v.origin(), v.cell(), time, fx.flash);
    }
}

struct Versus {
    m: Match,
    fx: [Effects; 2],
    pilot: Pilot,
    difficulty: Difficulty,
    countdown: f32,
    over_for: [Option<f32>; 2]
}

const COUNTDOWN: f32 = 3.0;

impl Versus {
    fn new(difficulty: Difficulty) -> Self {
        let seed = new_seed();
        Versus {
            m: Match::new(seed),
            fx: [
                Effects::new(geom(VS_BX[0], BY), seed ^ 1),
                Effects::new(geom(VS_BX[1], BY), seed ^ 2)
            ],
            pilot: difficulty.pilot(seed ^ 3),
            difficulty,
            countdown: COUNTDOWN,
            over_for: [None, None]
        }
    }

    fn paused(&self) -> bool {
        self.m.fighters[0].player.paused
    }

    fn meter_top(&self, i: usize) -> (f32, f32) {
        let pending = self.m.fighters[i].pending().min(HEIGHT as u32) as i32;
        (VS_METER[i] as f32 + 1.0, (BY + BOARD_H - pending * BH) as f32 - 1.0)
    }

    fn update(&mut self, dt: f32) {
        if is_key_pressed(KeyCode::R) {
            *self = Versus::new(self.difficulty);
            return;
        }
        if is_key_pressed(KeyCode::P) && self.countdown <= 0.0 && !self.m.is_over() {
            for f in &mut self.m.fighters {
                f.player.toggle_pause();
            }
        }
        self.countdown -= dt;
        if self.countdown <= 0.0 && !self.m.is_over() && !self.paused() {
            let before = self
                .m
                .fighters
                .each_ref()
                .map(|f| (f.player.game.pieces, f.player.game.level));
            let [you, ai] = &mut self.m.fighters;
            let [fx0, fx1] = &mut self.fx;
            let keys = human_input(&mut you.player, fx0);
            self.pilot.update(&mut ai.player, dt, |pl, action| apply(pl, fx1, action));
            you.player.update(dt, keys);
            ai.player.update(dt, HeldKeys::default());
            for ((fx, f), before) in self.fx.iter_mut().zip(&self.m.fighters).zip(before) {
                clear_effects(fx, &f.player.game, before);
            }

            for event in self.m.resolve(dt) {
                match event {
                    Event::Attack { from, sent, cancelled } => {
                        let g = self.fx[from].geom;
                        let start = (g.center_x(), g.y + g.height() * 0.7);
                        if cancelled >0 {
                            let to = self.meter_top(from);
                            self.fx[from].comet(start, to, cancelled, [0.6, 0.65, 0.7]);
                        }
                        if sent > 0 {
                            let to = self.meter_top(1 - from);
                            let color: Rgb = if sent >= 4 { [1.0, 0.55, 0.95] } else { [0.55, 0.95, 1.0] };
                            self.fx[from].comet(start, to, sent, color);
                        }
                    }
                    Event::Garbage { who, lines } => self.fx[who].garbage_in(lines),
                    Event::KnockOut { .. } => {}
                }
            }
        }
        for i in 0..2 {
            let game = &self.m.fighters[i].player.game;
            if game.game_over && self.over_for[i].is_none() {
                self.fx[i].game_over(&game.board);
                self.over_for[i] = Some(0.0);
            }
            if let Some(t) = self.over_for[i].as_mut() {
                *t += dt;
            }
            self.fx[i].update(dt);
        }
    }

    fn draw(&self, a: &mut Ascii, time: f32) {
        let shake = self.fx[0].shake.max(self.fx[1].shake);
        let v = View::fit(VS_COLS, GRID_ROWS, View::shake(shake));
        a.begin_scene();
        let by1 = BY + BOARD_H;

        for i in 0..2 {
            let (x0, x1) = VS_PANEL[i];
            v.panel(x0, BY - 1, x1, by1);
            v.panel(VS_METER[i] - 1, BY - 1, VS_METER[i] + 2, by1);
            board(a, &v, &self.m.fighters[i].player.game, VS_BX[i], BY, time, self.over_for[i]);
            fx_scene(&v, &self.fx[i]);
        }
        for i in 0..2 {
            self.side_panel(a, &v, i, time);
            self.meter(a, i, time);
        }

        let pulse = 0.85 + 0.15 * (time * 3.0).sin();
        let names = ["YOU".to_string(), format!("AI - {}", self.difficulty.name())];
        let tints: [Rgb; 2] = [[0.55, 1.0, 0.7], [1.0, 0.6, 0.55]];
        for i in 0..2 {
            let cx = (VS_BX[i] + BOARD_W / 2) as f32;
            let w = names[i].len() as f32 + 4.0;
            v.backing(cx - w / 2.0, (BY - 4) as f32, w, 3.0, 1.0);
            text_centered(a, cx, BY - 3, &names[i], scale(tints[i], pulse));
        }
        banner(a, &v, 1.0, "VS" , VS_CENTER, 1.0, 40.0, time, 1.0, 0.0, |_, y| {
            scale([0.95, 0.85, 0.6], 1.0 - 0.3 * y)
        });
        let secs = self.m.time as u32;
        let clock = format!("{}:{:02}", secs / 60, secs % 60);
        v.backing(VS_CENTER - 4.0, (BY + 1) as f32, 8.0, 1.0, 1.0);
        text_centered(a, VS_CENTER, BY + 1, &clock, TEXT);

        for fx in &self.fx {
            fx_glyphs(a, fx);
            fx_labels(a, &v, fx, time);
        }

        let mid = (BY + BOARD_H / 2) as f32;
        if self.countdown > 0.0 {
            let n = self.countdown.ceil() as usize;
            let digit = ["", "1", "2", "3"][n.min(3)];
            let t = 1.0 - self.countdown.fract();
            for bx in VS_BX {
                let cx = (bx + BOARD_W / 2) as f32;
                big_text(a, &v, digit, cx, mid, 3, [0.95, 0.85, 0.6], time, t / 0.2);
                text_centered(a, cx, mid as i32 + 10, "get ready", LABEL);
            }
        } else if self.countdown > -0.7 && !self.m.is_over() {
            let t = -self.countdown;
            for bx in VS_BX {
                let cx = (bx + BOARD_W / 2) as f32;
                big_text(a, &v, "GO!", cx, mid, 2, [0.55, 1.0, 0.7], time, t / 0.15);
            }
        }
        if let Some(winner) = self.m.result {
            let t = self.over_for.iter().flatten().fold(0.0f32, |a, &b| a.max(b));
            if t > 0.9 {
                let you = &self.m.fighters[0];
                let secs = self.m.time as u32;
                let lines = [
                    format!("{}:{:02}  |  sent {}  received {}", secs / 60, secs % 60, you.sent, you.received),
                    "R  rematch      ESC  menu".to_string(),
                ];
                let (texts, tint): ([&str; 2], [Rgb; 2]) = match winner {
                    Some(0) => (["YOU WIN", "KO"], [[0.55, 1.0, 0.7], [1.0, 0.5, 0.45]]),
                    Some(_) => (["YOU LOSE", "AI WINS"], [[1.0, 0.5, 0.45], [0.55, 1.0, 0.7]]),
                    None => (["DRAW", "DRAW"], [[0.95, 0.85, 0.6]; 2]),
                };
                overlay(a, &v, VS_BX[0], BY, texts[0], tint[0], &lines, time, t - 0.9);
                overlay(a, &v, VS_BX[1], BY, texts[1], tint[1], &[], time, t - 0.9);
            }
        } else if self.paused() {
            for bx in VS_BX {
                overlay(a, &v, bx, BY, "PAUSED", [0.95, 0.85, 0.6], &["P to resume".into()], time, 1.0);
            }
        }
        let flash = self.fx[0].flash.max(self.fx[1].flash);
        a.present(v.origin(), v.cell(), time, flash);
    }

    fn side_panel(&self, a: &mut Ascii, v: &View, i: usize, time: f32) {
        let f: &Fighter = &self.m.fighters[i];
        let game = &f.player.game;
        let (x0, x1) = VS_PANEL[i];
        let by1 = BY + BOARD_H;
        frame(a, x0, BY - 1, x1, by1, None);
        hold_box(a, v, game, x0 + 2, x1 - 2, BY + 1);
        next_box(a, v, game, x0 + 2, x1 - 2, BY + 12, 3);

        let pps = if self.m.time > 1.0 { game.pieces as f32 / self.m.time } else { 0.0 };
        let stats = [
            ("SENT", f.sent.to_string()),
            ("RECEIVED", f.received.to_string()),
            ("LINES", game.lines.to_string()),
            ("PIECES/S", format!("{pps:.2}")),
        ];
        let x = x0 + 2;
        for (k, (label, value)) in stats.iter().enumerate() {
            let y = BY + 35 + k as i32 * 3;
            a.text(x, y, label, LABEL);
            a.text(x + 10, y, value, TEXT);
        }
        
        let pulse = 0.7 + 0.3 * (time * 8.0).sin();
        if f.combo >= 2 {
            a.text(x, BY + 48, &format!("{} COMBO", f.combo - 1), scale([1.0, 0.85, 0.45], pulse));
        }
        if f.b2b {
            a.text(x, BY + 50, "BACK-TO-BACK", scale([1.0, 0.6, 0.95], pulse));
        }
        if i == 0 {
            let help = ["ARROWS move/rotate", "SPACE drop  C hold", "P pause  R restart", "ESC menu"];
            for (k, line) in help.iter().enumerate() {
                a.text(x, by1 - 8 + k as i32 * 2, line, DIM);
            }
        } else {
            for (k, line) in wrap(self.difficulty.description(), (x1 - x0 - 3) as usize).iter().enumerate() {
                a.text(x, by1 - 6 + k as i32 * 2, line, DIM);
            }
        }
    }

    fn meter(&self, a: &mut Ascii, i: usize, time: f32) {
        let x = VS_METER[i];
        let by1 = BY + BOARD_H;
        frame(a, x - 1, BY - 1, x + 2, by1, None);
        let mut y = by1 - 1;
        let blink = (time * 10.0).sin() > 0.0;
        for g in &self.m.fighters[i].incoming {
            let ready = g.timer <= 0.0;
            let c: Rgb = match (ready, blink) {
                (true, true) => [1.0, 0.25, 0.25],
                (true, false) => [0.75, 0.15, 0.15],
                _ => [1.0, 0.7, 0.3]
            };
            for k in 0..g.lines as i32 * BH {
                if y < BY {
                    return;
                }
                let ch = if k == g.lines as i32 * BH - 1 { b'=' } else { b'#' };
                a.put(x, y, ch, c);
                a.put(x + 1, y, ch, c);
                y -= 1;
            }
        }
    }
}

fn wrap(s: &str, width: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for word in s.split_whitespace() {
        match lines.last_mut() {
            Some(l) if l.len() + 1 + word.len() <= width => {
                l.push(' ');
                l.push_str(word);
            }
            _ => lines.push(word.to_string()),
        }
    }
    lines
}

enum Screen {
    Menu(Menu),
    Solo(Box<Solo>),
    Versus(Box<Versus>)
}

fn window_conf() -> Conf {
    Conf {
        window_title: "Rustris".to_owned(),
        window_width: (VS_COLS as f32 * CW) as i32,
        window_height: (GRID_ROWS as f32 * CH) as i32,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let mut ascii = Ascii::new(GRID_COLS, GRID_ROWS);
    let mut screen = Screen::Menu(Menu { choice: 0 });
    let mut difficulty = 1;
    let mut time = 0.0f32;

    loop {
        let dt = get_frame_time().min(0.1);
        time += dt;
        if is_key_pressed(KeyCode::V) {
            ascii.mode = ascii.mode.next();
        }
        match &mut screen {
            Screen::Menu(menu) => match menu.update(&mut difficulty) {
                MenuAction::Start(0) => screen = Screen::Solo(Box::new(Solo::new())),
                MenuAction::Start(_) => {
                    screen = Screen::Versus(Box::new(Versus::new(Difficulty::ALL[difficulty])))
                }
                MenuAction::Quit => {
                    #[cfg(not(target_arch = "wasm32"))]
                    break;
                }
                MenuAction::Stay => {}
            },
            Screen::Solo(_) | Screen::Versus(_) if is_key_pressed(KeyCode::Escape) => {
                let choice = matches!(screen, Screen::Versus(_)) as usize;
                screen = Screen::Menu(Menu { choice })
            }
            Screen::Solo(solo) => solo.update(dt),
            Screen::Versus(vs) => vs.update(dt),
        }
        match &screen {
            Screen::Menu(menu) => menu.draw(&mut ascii, Difficulty::ALL[difficulty], time),
            Screen::Solo(solo) => solo.draw(&mut ascii, time),
            Screen::Versus(vs) => vs.draw(&mut ascii, time),
        }
        next_frame().await;
    }
}

#[cfg(test)]
mod tests {
    use super::wrap;

    #[test]
    fn wrap_breaks_at_spaces() {
        assert_eq!(wrap("one two three", 7), vec!["one two", "three"]);
        assert_eq!(wrap("", 5), Vec::<String>::new());
    }
}