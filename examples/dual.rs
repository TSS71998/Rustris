use rustris::ai::Difficulty;
use rustris::versus::simulate;

fn main() {
    let n: u64 = std::env::args().nth(1).and_then(|a| a.parse().ok()).unwrap_or(10);
    println!("{n} matches per pairing (same seeds for every pairing)\n");
    println!("{:>8} vs {:<8} | wins  draws | avg time | pieces/s (l / r) | lines sent (l / r)", "left", "right");
    for a in Difficulty::ALL {
        for b in Difficulty::ALL {
            let (mut wins, mut draws, mut secs, mut sent, mut pieces) = (0, 0, 0.0, [0u32; 2], [0u32; 2]);
            for seed in 0..n {
                let m = simulate(a, b, seed, 600.0);
                match m.result {
                    Some(Some(0)) => wins += 1,
                    Some(None) | None => draws += 1,
                    _ => {}
                }
                secs += m.time;
                sent[0] += m.fighters[0].sent;
                sent[1] += m.fighters[1].sent;
                pieces[0] += m.fighters[0].player.game.pieces;
                pieces[1] += m.fighters[1].player.game.pieces;
            }
            println!(
                "{:>8} vs {:<8} | {wins:>3}/{n}  {draws:>3}   | {:>6.1}s  |   {:.2} / {:.2}    | {:>5.1} / {:<5.1}",
                a.name(),
                b.name(),
                secs / n as f32,
                pieces[0] as f32 / secs,
                pieces[1] as f32 / secs,
                sent[0] as f32 / n as f32,
                sent[1] as f32 / n as f32
            );
        }
    }
}