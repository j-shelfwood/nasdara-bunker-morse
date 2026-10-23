use nasdara_decoder::bunkers::{bunkers, is_izurvive_url};
use nasdara_decoder::grid::{DIGITS_PER_RECORD, SHIPPED_TIMING, decode_grid, decode_prefix};
use nasdara_decoder::pipeline::Pipeline;
use nasdara_decoder::synth::{SynthOptions, SynthRecord, chunks, render};
use nasdara_decoder::tone::{FRAME_SECONDS, ToneDetector};
use std::time::Instant;

fn levels(pcm: &[i16]) -> Vec<i8> {
    let mut d = ToneDetector::new();
    d.push(pcm)
        .iter()
        .map(|f| f.db.round().clamp(-30.0, 60.0) as i8)
        .collect()
}

fn one_record(code: &str, repeats: usize) -> SynthOptions {
    SynthOptions {
        records: vec![SynthRecord::new(&bunkers()[0].grid, code)],
        repeats,
        ..SynthOptions::default()
    }
}

#[test]
fn decode_prefix_on_a_cut_transmission_returns_exactly_the_first_four_digits() {
    let pcm = render(&one_record("4821", 1)).pcm;
    let full = levels(&pcm);
    let all = decode_grid(&full, DIGITS_PER_RECORD, &SHIPPED_TIMING).expect("full decode");
    assert_eq!(
        all.digits.iter().collect::<String>(),
        format!("{}4821", bunkers()[0].grid)
    );
    let cut_s = all.ends[3] + 0.05;
    let cut = (cut_s / FRAME_SECONDS) as usize + 1;
    let p = decode_prefix(&full[..cut + 4], &SHIPPED_TIMING, DIGITS_PER_RECORD).expect("prefix");
    assert_eq!(p.digits.iter().collect::<String>(), "0721");
    // well short of the 4th digit's window end (beyond the jitter allowance) it does not count yet
    let short = ((all.ends[3] - 0.3) / FRAME_SECONDS) as usize;
    let q = decode_prefix(&full[..short], &SHIPPED_TIMING, DIGITS_PER_RECORD).expect("prefix");
    // The 4th digit's window is still open, so a look-alike that fits the shorter trace may take its place
    // (here '3' for '1', whose tone had all arrived); the first three are settled.
    let head: String = q.digits.iter().take(3).collect();
    assert_eq!(head, "072");
    assert!(q.digits.len() <= 4);
}

#[test]
fn preview_grows_and_final_equals_the_burst() {
    let pcm = render(&one_record("4821", 1)).pcm;
    let mut p = Pipeline::new();
    let mut counts = Vec::new();
    let mut last = None;
    let mut bursts = Vec::new();
    for c in chunks(&pcm) {
        bursts.extend(p.push(c).bursts);
        if let Some(pv) = p.preview() {
            counts.push(pv.digits.len());
            last = Some(pv);
        }
    }
    let last = last.expect("preview while streaming");
    bursts.extend(p.flush());
    assert!(counts.windows(2).all(|w| w[0] <= w[1]), "{counts:?}");
    assert!(counts.contains(&4), "{counts:?}");
    assert_eq!(*counts.last().unwrap_or(&0), DIGITS_PER_RECORD);
    let digits: Vec<char> = last.digits.iter().map(|d| d.ch).collect();
    assert_eq!(bursts.len(), 1);
    assert_eq!(digits, bursts[0].records[0].digits);
    assert!(
        (last.start - bursts[0].records[0].start).abs() < 0.05,
        "{} vs {}",
        last.start,
        bursts[0].records[0].start
    );
}

#[test]
fn izurvive_allow_list_good_form_and_six_bad_variants() {
    assert!(is_izurvive_url(
        "https://www.izurvive.com/nasdara/#location=7216;5707;7"
    ));
    for bad in [
        "http://www.izurvive.com/nasdara/#location=7216;5707;7",
        "https://www.izurvive.org/nasdara/#location=7216;5707;7",
        "https://www.izurvive.com/nasdara/extra/#location=7216;5707;7",
        "javascript:alert(1)",
        "https://www.izurvive.com/nasdara/#location=7216;5707",
        "https://www.izurvive.com/nasdara/#location=7216;5707;7 ",
    ] {
        assert!(!is_izurvive_url(bad), "{bad}");
    }
}

#[test]
#[ignore = "timing, run with --release -- --ignored --nocapture"]
fn timing_100s_trace() {
    let pcm = render(&one_record("4821", 8)).pcm;
    let mut full = levels(&pcm);
    full.truncate((100.0 / FRAME_SECONDS) as usize);
    let t = Instant::now();
    let g = decode_grid(&full, DIGITS_PER_RECORD, &SHIPPED_TIMING);
    let a = t.elapsed();
    let t = Instant::now();
    let p = decode_prefix(&full, &SHIPPED_TIMING, DIGITS_PER_RECORD);
    let b = t.elapsed();
    println!(
        "frames {} decode_grid {a:?} ({}) decode_prefix {b:?} ({})",
        full.len(),
        g.is_some(),
        p.is_some()
    );
}
