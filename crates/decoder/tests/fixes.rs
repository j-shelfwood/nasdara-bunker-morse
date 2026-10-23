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
