use nasdara_decoder::bunkers::bunkers;
use nasdara_decoder::pipeline::{Pipeline, Preview};
use nasdara_decoder::synth::{SynthOptions, SynthRecord, chunks, render};

fn one_record(code: &str, repeats: usize) -> SynthOptions {
    SynthOptions {
        records: vec![SynthRecord::new(&bunkers()[0].grid, code)],
        repeats,
        ..SynthOptions::default()
    }
}

#[test]
fn only_the_trailing_preview_digit_is_unsettled_and_settled_digits_never_change() {
    let pcm = render(&one_record("4821", 1)).pcm;
    let mut p = Pipeline::new();
    let mut previews: Vec<Preview> = Vec::new();
    for c in chunks(&pcm) {
        p.push(c);
        if let Some(pv) = p.preview() {
            previews.push(pv);
        }
    }
    assert!(!previews.is_empty());
    let mut settled_seen: Vec<Option<char>> = vec![None; 10];
    let mut unsettled_total = 0;
    for pv in &previews {
        let last = pv.digits.len() - 1;
        for (i, d) in pv.digits.iter().enumerate() {
            if !d.settled {
                assert_eq!(i, last, "unsettled digit {i} is not the trailing one");
                unsettled_total += 1;
            } else if let Some(prev) = settled_seen[i] {
                assert_eq!(prev, d.ch, "settled digit {i} changed");
            } else {
                settled_seen[i] = Some(d.ch);
            }
        }
    }
    assert!(
        unsettled_total > 0,
        "the trailing digit is open at some point"
    );
    let truth = format!("{}4821", bunkers()[0].grid);
    let settled: String = settled_seen.iter().map(|c| c.unwrap_or('-')).collect();
    assert_eq!(settled, truth);
}

// ---- group 2: decode results stay bit-identical while the preview gets cheaper ----

use nasdara_decoder::grid::{DIGITS_PER_RECORD, SHIPPED_TIMING, decode_grid, decode_prefix};
use nasdara_decoder::synth::DEFAULTS;
use nasdara_decoder::tone::ToneDetector;

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

fn levels_of(pcm: &[i16]) -> Vec<i8> {
    let mut d = ToneDetector::new();
    d.push(pcm)
        .iter()
        .map(|f| (f.db.round()).clamp(-30.0, 60.0) as i8)
        .collect()
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
    let want: Vec<&str> = include_str!("fixtures/decode_fingerprint.txt")
        .lines()
        .collect();
    let got = fingerprint();
    assert_eq!(got.len(), want.len());
    for (g, w) in got.iter().zip(&want) {
        assert_eq!(g, w);
    }
}

fn chirp(seconds: usize) -> Vec<i16> {
    (0..44100 * seconds)
        .map(|i| {
            if (i / 8820) % 2 == 0 {
                ((i as f64 * 2.0 * std::f64::consts::PI * 985.0 / 44100.0).sin() * 8000.0) as i16
            } else {
                0
            }
        })
        .collect()
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

// ---- group 3: the preview's open-burst tracking ignores short spikes ----

fn spike_into(pcm: &mut [i16], at_s: f64, samples: usize) {
    let s0 = (at_s * 44100.0) as usize;
    for i in 0..samples {
        if let Some(x) = pcm.get_mut(s0 + i) {
            *x = ((i as f64 * 2.0 * std::f64::consts::PI * 985.0 / 44100.0).sin() * 12000.0) as i16;
        }
    }
}

/// Longest run of consecutive on-frames of the live hysteresis, over a push of `pcm`.
fn longest_live_run(pcm: &[i16]) -> usize {
    let mut p = Pipeline::new();
    let (mut run, mut best) = (0, 0);
    for c in pcm.chunks(256) {
        for _ in p.push(c).frames {
            if p.live().on {
                run += 1;
                best = best.max(run);
            } else {
                run = 0;
            }
        }
    }
    best
}

#[test]
fn a_stray_spike_every_3_s_does_not_join_two_messages_in_the_preview() {
    let one = render(&one_record("4821", 1)).pcm;
    let msg_s = one.len() as f64 / 44100.0;
    // the last beep ends 8 s before the end of the render (6 s bunker pause + 2 s tail); a few-frame spike
    // (under min_on_s) every 3 s from then until the second message starts
    let gap_start = msg_s - 8.0;
    let mut pcm = one.clone();
    pcm.extend(vec![0i16; 44100 * 10]);
    let second_start = pcm.len() as f64 / 44100.0 + 1.0;
    pcm.extend(&one);
    let mut t = gap_start + 2.5;
    while t < second_start - 1.0 {
        spike_into(&mut pcm, t, 200);
        t += 3.0;
    }
    let spike_only = {
        let mut z = vec![0i16; 44100 * 4];
        spike_into(&mut z, 1.0, 200);
        z
    };
    let run = longest_live_run(&spike_only);
    assert!((1..=4).contains(&run), "the spike is {run} live frames");

    let mut p = Pipeline::new();
    let mut in_gap = 0;
    let mut previews_after_close = 0;
    for (k, c) in chunks(&pcm).enumerate() {
        p.push(c);
        let now = (k + 1) as f64 * 0.1;
        if now > gap_start + 7.0 && now < second_start - 1.0 {
            in_gap += 1;
            if p.preview().is_some() {
                previews_after_close += 1;
            }
        }
    }
    assert!(in_gap > 50);
    assert_eq!(
        previews_after_close, 0,
        "{previews_after_close} of {in_gap} pushes still previewed the closed first message"
    );
}
