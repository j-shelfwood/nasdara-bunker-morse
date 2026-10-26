//! group 2: decode results stay bit-identical while the preview gets cheaper

use crate::helpers::{chirp, levels_of};
use nasdara_decoder::bunkers::bunkers;
use nasdara_decoder::grid::{DIGITS_PER_RECORD, SHIPPED_TIMING, decode_grid, decode_prefix};
use nasdara_decoder::pipeline::Pipeline;
use nasdara_decoder::synth::DEFAULTS;
use nasdara_decoder::synth::{SynthOptions, SynthRecord, chunks, render};

fn noisy() -> SynthOptions {
    let b = bunkers();
    SynthOptions {
        records: vec![
            SynthRecord::new(&b[0].grid, "4821"),
            SynthRecord::new(&b[9].grid, "0075"),
        ],
        variance: 1.0,
        dropout: 0.2,
        lose: 0.03,
        seed: 4,
        repeats: 2,
        ..DEFAULTS
    }
}

fn describe(g: &nasdara_decoder::grid::GridDecode) -> String {
    format!(
        "{} {:?} {:?} {:?} {:?}",
        g.digits.iter().collect::<String>(),
        g.support,
        g.margins,
        g.starts,
        g.score
    )
}

fn fingerprint() -> Vec<String> {
    let pcm = render(&noisy()).pcm;
    let lv = levels_of(&pcm);
    let mut out = Vec::new();
    let whole = decode_grid(
        &lv[..lv.len().min(14000)],
        DIGITS_PER_RECORD,
        &SHIPPED_TIMING,
    );
    out.push(whole.as_ref().map_or("none".into(), describe));
    let pre = decode_prefix(
        &lv[..lv.len().min(5200)],
        &SHIPPED_TIMING,
        DIGITS_PER_RECORD,
    );
    out.push(pre.as_ref().map_or("none".into(), describe));
    let mut p = Pipeline::new();
    let mut bursts: Vec<_> = chunks(&pcm).flat_map(|c| p.push(c).bursts).collect();
    bursts.extend(p.flush());
    for r in bursts.iter().flat_map(|b| &b.records) {
        out.push(format!(
            "{} {:?} {:?} {:?}",
            r.digits.iter().collect::<String>(),
            r.support,
            r.margins,
            r.digit_starts
        ));
    }
    out
}

/// Expected output recorded from the decoder before the preview was optimised (decode_grid, decode_prefix, and the
/// final records of the pipeline, every float printed in full).
#[test]
fn decode_results_are_bit_identical_to_the_recorded_run() {
    let want: Vec<&str> = include_str!("../fixtures/decode_fingerprint.txt")
        .lines()
        .collect();
    let got = fingerprint();
    assert_eq!(got.len(), want.len());
    for (g, w) in got.iter().zip(&want) {
        assert_eq!(g, w);
    }
}

#[test]
#[ignore = "timing, run with --release -- --ignored --nocapture"]
fn preview_cache_miss_is_under_30_ms_on_a_full_trace() {
    let mut worst = 0.0f64;
    let mut sum = 0.0f64;
    let mut n = 0;
    let mut p = Pipeline::new();
    for (i, c) in chunks(&chirp(150)).enumerate() {
        p.push(c);
        if i > 1000 {
            let t = std::time::Instant::now();
            let pv = p.preview();
            let ms = t.elapsed().as_secs_f64() * 1000.0;
            if ms > 1.0 {
                worst = worst.max(ms);
                sum += ms;
                n += 1;
            }
            assert!(pv.is_some());
        }
    }
    println!(
        "preview cache misses {n}, mean {:.1} ms, worst {worst:.1} ms",
        sum / f64::from(n)
    );
    assert!(worst < 30.0, "worst cache-miss preview {worst:.1} ms");
}

#[test]
fn preview_decodes_at_most_the_newest_100_seconds_of_an_open_burst() {
    let mut p = Pipeline::new();
    for c in chunks(&chirp(150)) {
        p.push(c);
    }
    let pv = p.preview().expect("preview of the chirp");
    assert!(
        pv.start >= 150.0 - 101.0,
        "preview starts at {:.1} s, older than 100 s",
        pv.start
    );
}
