//! group 4: a periodic chirp cannot grow the assembler without bound

use crate::helpers::chirp;
use nasdara_decoder::pipeline::Pipeline;
use nasdara_decoder::synth::chunks;
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
