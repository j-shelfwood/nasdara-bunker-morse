//! group 7: small fixes

use crate::helpers::{levels_of, one_record};
use nasdara_decoder::grid::{DIGITS_PER_RECORD, SHIPPED_TIMING, decode_grid, decode_prefix};
use nasdara_decoder::pipeline::Pipeline;
use nasdara_decoder::synth::{chunks, render};
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
