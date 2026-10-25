//! group 3: the preview's open-burst tracking ignores short spikes

use crate::helpers::one_record;
use nasdara_decoder::pipeline::Pipeline;
use nasdara_decoder::synth::{chunks, render};
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
