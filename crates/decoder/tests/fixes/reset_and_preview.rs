//! group 5: reset, and the preview outlives the closing of its burst until the burst is emitted

use crate::helpers::one_record;
use nasdara_decoder::pipeline::Pipeline;
use nasdara_decoder::synth::{chunks, render};
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
