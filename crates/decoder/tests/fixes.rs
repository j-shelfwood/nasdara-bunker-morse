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

// ---- group 4: a periodic chirp cannot grow the assembler without bound ----

#[test]
fn a_periodic_chirp_is_emitted_as_ignored_bursts_and_pending_stays_bounded() {
    let mut p = Pipeline::new();
    let mut bursts = Vec::new();
    let mut max_pending = 0;
    for c in chunks(&chirp(450)) {
        bursts.extend(p.push(c).bursts);
        max_pending = max_pending.max(p.pending());
    }
    bursts.extend(p.flush());
    assert!(max_pending <= 520, "pending grew to {max_pending} beeps");
    assert!(bursts.len() >= 2, "{} bursts", bursts.len());
    for b in &bursts {
        assert!(b.span.1 - b.span.0 <= 201.0, "span {:?}", b.span);
    }
    // the capped bursts are ignored; the tail left at the flush is an ordinary (undecodable) burst
    for b in &bursts[..bursts.len() - 1] {
        assert!(b.ignored.is_some() && b.note.is_none() && b.records.is_empty());
    }
    assert!(
        bursts[0].ignored.as_deref().unwrap_or("").contains("200"),
        "{:?}",
        bursts[0].ignored
    );
}

// ---- group 5: reset, and the preview outlives the closing of its burst until the burst is emitted ----

#[test]
fn the_preview_is_held_until_the_push_after_the_burst_was_emitted() {
    let pcm = render(&one_record("4821", 1)).pcm;
    let mut p = Pipeline::new();
    let mut seen_preview = false;
    let mut burst_push = None;
    for (k, c) in chunks(&pcm).enumerate() {
        let out = p.push(c);
        let pv = p.preview();
        match burst_push {
            None if !out.bursts.is_empty() => {
                burst_push = Some(k);
                assert!(
                    pv.is_some(),
                    "no preview on the push that emitted the burst"
                );
            }
            None => {
                if seen_preview {
                    assert!(
                        pv.is_some(),
                        "preview dropped at push {k} before the burst arrived"
                    );
                }
            }
            Some(b) if k == b + 1 => assert!(pv.is_none(), "preview kept after the next push"),
            Some(_) => assert!(pv.is_none()),
        }
        seen_preview |= pv.is_some();
    }
    assert!(burst_push.is_some());
}

#[test]
fn reset_gives_the_state_of_a_new_pipeline() {
    let pcm = render(&one_record("4821", 1)).pcm;
    let run = |p: &mut Pipeline| {
        let mut bursts: Vec<_> = chunks(&pcm).flat_map(|c| p.push(c).bursts).collect();
        bursts.extend(p.flush());
        bursts
    };
    let fresh = run(&mut Pipeline::new());
    let mut p = Pipeline::new();
    // stop mid-message with an open burst, a preview and pending beeps
    for c in chunks(&pcm).take(400) {
        p.push(c);
    }
    assert!(p.pending() > 0 && p.preview().is_some());
    p.reset();
    assert_eq!(p.pending(), 0);
    assert!(p.preview().is_none());
    assert!(!p.live().on);
    let again = run(&mut p);
    assert_eq!(fresh.len(), 1);
    assert_eq!(again, fresh);
}

#[test]
fn flush_still_emits_the_burst_in_progress() {
    let pcm = render(&one_record("4821", 1)).pcm;
    let mut p = Pipeline::new();
    for c in chunks(&pcm).take(600) {
        assert!(p.push(c).bursts.is_empty());
    }
    let b = p.flush();
    assert_eq!(b.len(), 1);
    assert_eq!(p.pending(), 0);
    assert!(p.preview().is_none());
}

// ---- group 6: the ledger votes in f64 like the prototype, and its loose map is bounded ----

use nasdara_decoder::bunkers::snap_grid;
use nasdara_decoder::bursts::BunkerRecord;
use nasdara_decoder::ledger::BunkerLedger;

fn code_record(bunker: usize, code_digit: char, support: f32) -> BunkerRecord {
    let grid = bunkers()[bunker].grid.clone();
    let mut digits: Vec<char> = grid.chars().collect();
    digits.extend([code_digit, '?', '?', '?']);
    let mut sup = vec![1.0; 6];
    sup.extend([support, 0.0, 0.0, 0.0]);
    BunkerRecord {
        code: digits[6..].iter().collect(),
        digits,
        support: sup,
        margins: vec![],
        snap: snap_grid(&grid),
        grid,
        start: 0.0,
        digit_starts: vec![],
    }
}

/// `n` hundredths as the f32 the decoder reports (`Number(s.toFixed(2))`).
fn hundredths(n: u32) -> f32 {
    format!("{}.{:02}", n / 100, n % 100).parse().unwrap_or(0.0)
}

#[test]
fn exact_ratio_ties_agree_with_an_f64_reference() {
    let mut mismatches = Vec::new();
    let mut pairs = 0;
    for top in 1..=300u32 {
        for second in 1..=top {
            pairs += 1;
            let mut l = BunkerLedger::new();
            l.add(&code_record(3, '1', hundredths(top)));
            let e = l.add(&code_record(3, '2', hundredths(second)));
            let (t, s) = (f64::from(top) / 100.0, f64::from(second) / 100.0);
            let want = if s > 0.7 * t { '?' } else { '1' };
            let got = e.code.chars().next().unwrap_or('!');
            if got != want {
                mismatches.push((top, second, got, want));
            }
        }
    }
    assert!(pairs > 40_000);
    assert!(
        mismatches.is_empty(),
        "{} {:?}",
        mismatches.len(),
        &mismatches[..mismatches.len().min(5)]
    );
}

#[test]
fn loose_records_are_capped_and_the_oldest_is_dropped() {
    let loose = |n: usize| BunkerRecord {
        digits: "??????".chars().chain(format!("{n:04}").chars()).collect(),
        support: vec![1.0; 10],
        margins: vec![],
        grid: "??????".into(),
        code: format!("{n:04}"),
        snap: snap_grid("??????"),
        start: 0.0,
        digit_starts: vec![],
    };
    assert!(loose(0).snap.bunker().is_none());
    let mut l = BunkerLedger::new();
    for n in 0..300 {
        assert_eq!(l.add(&loose(n)).count, 1);
    }
    assert_eq!(l.add(&loose(299)).count, 2, "a recent entry is kept");
    assert_eq!(l.add(&loose(0)).count, 1, "the oldest entry was dropped");
}

// ---- group 7: small fixes ----

#[test]
fn counts_above_a_record_are_clamped_to_ten_digits() {
    let pcm = render(&one_record("4821", 1)).pcm;
    let lv = levels_of(&pcm);
    let want = decode_grid(&lv, DIGITS_PER_RECORD, &SHIPPED_TIMING).expect("ten digits");
    assert_eq!(want.digits.len(), 10);
    assert_eq!(decode_grid(&lv, 100_000, &SHIPPED_TIMING), Some(want));
    let pre = decode_prefix(&lv, &SHIPPED_TIMING, DIGITS_PER_RECORD).expect("prefix");
    assert_eq!(decode_prefix(&lv, &SHIPPED_TIMING, 100_000), Some(pre));
    assert_eq!(decode_grid(&lv, 0, &SHIPPED_TIMING), None);
}

#[test]
fn one_long_push_decodes_a_message_in_its_first_minute() {
    let mut pcm = render(&one_record("4821", 1)).pcm;
    pcm.resize(44100 * 400, 0);
    let mut one = Pipeline::new();
    let mut got = one.push(&pcm).bursts;
    got.extend(one.flush());
    let mut chunked = Pipeline::new();
    let mut want: Vec<_> = chunks(&pcm).flat_map(|c| chunked.push(c).bursts).collect();
    want.extend(chunked.flush());
    assert_eq!(want.len(), 1);
    assert_eq!(want[0].records.len(), 1);
    assert_eq!(got, want, "note: {:?}", got.first().map(|b| &b.note));
}
