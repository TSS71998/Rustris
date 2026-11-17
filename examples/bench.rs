//! Measures how well the AI plays.
//!
//!     cargo run --release --example bench -- [games] [max_pieces] [options]
//!
//!   --no-lookahead   only consider the current piece
//!   --no-hold        never use hold
//!   --weights FILE   use a weights file instead of the shipped one

use std::time::Instant;

use rustris::ai::{Ai, Weights, play_game};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let weights_path = args
        .iter()
        .position(|a| a == "--weights")
        .and_then(|i| args.get(i + 1));
    let nums: Vec<u32> = args
        .iter()
        .filter(|a| Some(*a) != weights_path)
        .filter_map(|a| a.parse().ok())
        .collect();
    let games = nums.first().copied().unwrap_or(10);
    let max_pieces = nums.get(1).copied().unwrap_or(2000);
    let weights = match weights_path {
        Some(path) => {
            let text = std::fs::read_to_string(path).expect("can't read weights file");
            Weights::parse(&text).unwrap_or_else(|e| panic!("{path}: {e}"))
        }
        None => Weights::default(),
    };
    let ai = Ai {
        weights,
        lookahead: !args.iter().any(|a| a == "--no-lookahead"),
        use_hold: !args.iter().any(|a| a == "--no-hold"),
    };
    println!(
        "{games} games, up to {max_pieces} pieces, lookahead {}, hold {}",
        ai.lookahead, ai.use_hold
    );

    let start = Instant::now();
    let (mut score, mut lines, mut tetrises, mut pieces, mut deaths) = (0u64, 0u64, 0u64, 0u64, 0);
    for seed in 0..games as u64 {
        let g = play_game(&ai, seed, max_pieces);
        println!(
            "  seed {seed:3}: score {:>10}  lines {:>5}  tetrises {:>4}  pieces {:>5}{}",
            g.score,
            g.lines,
            g.tetrises,
            g.pieces,
            if g.game_over { "  (died)" } else { "" }
        );
        score += g.score as u64;
        lines += g.lines as u64;
        tetrises += g.tetrises as u64;
        pieces += g.pieces as u64;
        deaths += g.game_over as u32;
    }
    let n = games.max(1) as u64;
    let secs = start.elapsed().as_secs_f64();
    println!(
        "average: score {}  lines {}  tetrises {}  pieces {}  |  died {deaths}/{games}",
        score / n,
        lines / n,
        tetrises / n,
        pieces / n
    );
    println!(
        "tetris rate {:.0}% of lines  |  {:.3} ms per move",
        100.0 * (tetrises * 4) as f64 / lines.max(1) as f64,
        1000.0 * secs / pieces.max(1) as f64
    );
}