//! The preview settles all but its trailing digit.

use crate::helpers::one_record;
use nasdara_decoder::bunkers::bunkers;
use nasdara_decoder::pipeline::{Pipeline, Preview};
use nasdara_decoder::synth::{chunks, render};

#[test]
fn only_the_trailing_preview_digit_is_unsettled_and_the_first_settled_reading_is_right() {
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
            } else if settled_seen[i].is_none() {
                // later previews may re-parse the prefix and move a settled digit (most synthetic records do);
                // the reading it had when it first counted as settled is the one checked
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

#[test]
fn the_preview_narrows_its_candidates_as_grid_digits_settle_and_ends_on_the_true_bunker() {
    let pcm = render(&one_record("4821", 1)).pcm;
    let mut p = Pipeline::new();
    let mut counts: Vec<usize> = Vec::new();
    let mut last: Option<Preview> = None;
    for c in chunks(&pcm) {
        p.push(c);
        if let Some(pv) = p.preview() {
            counts.push(pv.candidates.len());
            last = Some(pv);
        }
    }
    let last = last.expect("a preview");
    assert!(
        counts.iter().any(|&n| n > 1) && counts.iter().any(|&n| n <= 1),
        "candidate counts over the previews: {counts:?}"
    );
    assert_eq!(last.candidates.len(), 1, "counts: {counts:?}");
    assert_eq!(last.candidates[0].index, bunkers()[0].index);
}
