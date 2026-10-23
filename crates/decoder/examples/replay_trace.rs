//! Replays the tone-level traces written by `bunker-morse --log <file> --trace` through the grid decoder and prints
//! what it makes of each: `cargo run --release --example replay_trace -p nasdara_decoder -- <file>.trace.jsonl [line...]`.
//! Lines are counted from 0; without any, every line is replayed.

use nasdara_decoder::grid::{DIGITS_PER_RECORD, SHIPPED_TIMING, decode_grid};
use nasdara_decoder::pipeline::MIN_SUPPORT;
use nasdara_decoder::tone::FRAME_SECONDS;

fn field<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let at = line.find(&format!("\"{key}\":"))? + key.len() + 3;
    let rest = &line[at..];
    let end = if rest.starts_with('[') {
        rest.find(']')? + 1
    } else {
        rest.find([',', '}'])?
    };
    Some(&rest[..end])
}

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("trace file");
    let only: Vec<usize> = args.filter_map(|a| a.parse().ok()).collect();
    let text = std::fs::read_to_string(&path).expect("read trace");
    for (n, line) in text.lines().enumerate() {
        if !only.is_empty() && !only.contains(&n) {
            continue;
        }
        let t0: f64 = field(line, "t0")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.0);
        let db: Vec<i8> = field(line, "db")
            .map(|v| {
                v.trim_matches(['[', ']'])
                    .split(',')
                    .filter_map(|x| x.trim().parse().ok())
                    .collect()
            })
            .unwrap_or_default();
        let Some(g) = decode_grid(&db, DIGITS_PER_RECORD, &SHIPPED_TIMING) else {
            println!("line {n}: no decode ({} frames)", db.len());
            continue;
        };
        let shown: String = g
            .digits
            .iter()
            .zip(&g.support)
            .map(|(&d, &s)| if s < MIN_SUPPORT { '?' } else { d })
            .collect();
        let raw: String = g.digits.iter().collect();
        println!(
            "line {n}: t0 {t0:.1} frames {} ({:.0} s)  shown {shown}  best {raw}",
            db.len(),
            db.len() as f64 * FRAME_SECONDS
        );
        let row = |label: &str, v: &[f32]| {
            println!(
                "   {label:8}{}",
                v.iter().map(|x| format!("{x:6.2}")).collect::<String>()
            )
        };
        row("support", &g.support);
        row("margin", &g.margins);
        println!(
            "   starts  {}",
            g.starts
                .iter()
                .map(|s| format!("{s:6.1}"))
                .collect::<String>()
        );
    }
}
