//! Trains the AI's weights with the noisy cross-entropy method
//! (Szita & Lorincz, 2006)
//!
//!     cargo run --release --example train -- [options]
//!
//!   --gens N      generations (default 20)
//!   --pop N       candidates per generation (default 50)
//!   --games N     games per candidate (default 4)
//!   --pieces N    piece limit per game (default 400)
//!   --seed N      random seed (default: time based)
//!   --fast        train without look-ahead (much faster, a bit weaker)
//!   --resume      start from the current weights instead of from scratch

use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use rustris::ai::{Ai, FEATURE_NAMES, N_FEATURES, Weights, play_game};
use rustris::rng::Rng;

const OUT: &str = "assets/ai_weights.txt";

struct Opts {
    gens: u32,
    pop: usize,
    games: usize,
    pieces: u32,
    seed: u64,
    lookahead: bool,
    resume: bool
}

fn opts() -> Opts {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let num = |name: &str, default: u64| -> u64 {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .map(|v| v.parse().unwrap_or_else(|_| panic!("{name} needs a number")))
            .unwrap_or(default)
    };
    let time_seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(1);
    Opts {
        gens: num("--gens", 20) as u32,
        pop: num("--pop", 50) as usize,
        games: num("--games", 4) as usize,
        pieces: num("--pieces", 400) as u32,
        seed: num("--seed", time_seed),
        lookahead: !args.iter().any(|a| a == "--fast"),
        resume: args.iter().any(|a| a == "--resume"),
    }
}

fn gauss(rng: &mut Rng) -> f32 {
    let u1 = rng.f32().max(1e-7);
    let u2 = rng.f32();
    (-2.0 * u1.ln()).sqrt() * (std::f32::consts::TAU * u2).cos()
}

fn evaluate(cards: &[Weights], seeds: &[u64], o: &Opts) -> Vec<f64> {
    let mut scores = vec![0.0; cards.len()];
    let next = AtomicUsize::new(0);
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    let results: Vec<Vec<(usize, f64)>> = std::thread::scope(|s| {
        let handles: Vec<_> = (0..threads)
            .map(|_| {
                s.spawn(|| {
                    let mut out = Vec::new();
                    loop {
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        let Some(w) = cards.get(i) else { break };
                        let ai = Ai {
                            weights: w.clone(),
                            lookahead: o.lookahead,
                            use_hold: true,
                        };
                        let total: f64 = seeds
                            .iter()
                            .map(|&seed| play_game(&ai, seed, o.pieces).score as f64)
                            .sum();
                        out.push((i, total / seeds.len() as f64));
                    }
                    out
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    for (i, s) in results.into_iter().flatten() {
        scores[i] = s;
    }
    scores
}

fn main() {
    let o = opts();
    let mut rng = Rng::new(o.seed);
    let (mut mean, mut std) = if o.resume {
        (Weights::default().0, [3.0; N_FEATURES])
    } else {
        ([0.0; N_FEATURES], [10.0; N_FEATURES])
    };
    let n_elite = (o.pop / 5).max(2);
    println!(
        "training: {} generations x {} candidates x {} games of {} pieces, look-ahead {}, seed {}",
        o.gens, o.pop, o.games, o.pieces, o.lookahead, o.seed
    );

    for generation in 1..=o.gens {
        let t0 = Instant::now();
        let seeds: Vec<u64> = (0..o.games).map(|_| rng.next_u64()).collect();
        let cands: Vec<Weights> = (0..o.pop)
            .map(|_| {
                let mut w = [0.0; N_FEATURES];
                for i in 0..N_FEATURES {
                    w[i] = mean[i] + std[i] * gauss(&mut rng);
                }
                Weights(w)
            })
            .collect();
        let scores = evaluate(&cands, &seeds, &o);

        let mut ranked: Vec<usize> = (0..o.pop).collect();
        ranked.sort_by(|&a, &b| scores[b].total_cmp(&scores[a]));
        let elite = &ranked[..n_elite];
        let noise = (4.0 - generation as f32 * 0.4).max(0.1);
        for i in 0..N_FEATURES {
            let vals: Vec<f32> = elite.iter().map(|&e| cands[e].0[i]).collect();
            mean[i] = vals.iter().sum::<f32>() / n_elite as f32;
            let var = vals.iter().map(|v| (v - mean[i]).powi(2)).sum::<f32>() / n_elite as f32;
            std[i] = (var + noise).sqrt();
        }
        let best = scores[ranked[0]];
        let elite_avg = elite.iter().map(|&e| scores[e]).sum::<f64>() / n_elite as f64;
        let pop_avg = scores.iter().sum::<f64>() / o.pop as f64;
        println!(
            "gen {generation:3} | best {best:>9.0} | elite avg {elite_avg:>9.0} | pop avg {pop_avg:>9.0} score/game | {:.1}s",
            t0.elapsed().as_secs_f32()
        );

        let header = format!(
            "# Rustris AI weights, written by `cargo run --release --example train`.\n\
             # generation {generation}: elite average {elite_avg:.0} points per {}-piece game\n\
             # (look-ahead {}, {} candidates x {} games, seed {}).\n",
            o.pieces, o.lookahead, o.pop, o.games, o.seed
        );
        let text = header + &Weights(mean).to_text();
        let tmp = format!("{OUT}.tmp");
        std::fs::write(&tmp, text).and_then(|_| std::fs::rename(&tmp, OUT)).expect("write weights");
    }

    println!("\nfinal weights (also in {OUT}):");
    for (name, v) in FEATURE_NAMES.iter().zip(mean) {
        println!("  {name:16} {v:9.3}");
    }
}