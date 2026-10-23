use nasdara_decoder::bunkers::{
    SnapKind, bunkers, cell_center, grid_of, is_izurvive_url, izurvive_url, snap_grid,
};
use nasdara_decoder::bursts::{BunkerRecord, Burst};
use nasdara_decoder::ledger::BunkerLedger;
use nasdara_decoder::pipeline::Pipeline;
use nasdara_decoder::synth::{SynthOptions, SynthRecord, chunks, render};
use std::collections::{HashMap, HashSet};

fn records() -> Vec<SynthRecord> {
    let b = bunkers();
    vec![
        SynthRecord::new(&b[0].grid, "4821"),
        SynthRecord::new(&b[9].grid, "0075"),
        SynthRecord::new(&b[16].grid, "9136"),
    ]
}

fn base() -> SynthOptions {
    SynthOptions {
        records: records(),
        ..SynthOptions::default()
    }
}

struct Run {
    bursts: Vec<Burst>,
    records: Vec<BunkerRecord>,
}

fn run(o: &SynthOptions) -> Run {
    let pcm = render(o).pcm;
    let mut p = Pipeline::new();
    let mut bursts: Vec<Burst> = chunks(&pcm).flat_map(|c| p.push(c).bursts).collect();
    bursts.extend(p.flush());
    let records = bursts.iter().flat_map(|b| b.records.clone()).collect();
    Run { bursts, records }
}

fn truth() -> Vec<String> {
    records()
        .iter()
        .map(|r| format!("{}{}", r.grid, r.code))
        .collect()
}

fn expected(reps: usize) -> Vec<String> {
    (0..reps).flat_map(|_| truth()).collect()
}

fn got(rs: &[BunkerRecord]) -> Vec<String> {
    rs.iter().map(|r| r.digits.iter().collect()).collect()
}

/// Lock code per bunker of the records after voting over every record heard ("-" when never heard).
fn voted_codes(rs: &[BunkerRecord]) -> Vec<String> {
    let mut l = BunkerLedger::new();
    let mut last: HashMap<String, String> = HashMap::new();
    for r in rs {
        let e = l.add(r);
        if let Some(b) = e.bunker {
            last.insert(b.grid.clone(), e.code);
        }
    }
    records()
        .iter()
        .map(|x| last.get(&x.grid).cloned().unwrap_or_else(|| "-".into()))
        .collect()
}

fn codes() -> Vec<String> {
    records().iter().map(|r| r.code.clone()).collect()
}

fn wrong_snaps(rs: &[BunkerRecord]) -> usize {
    let grids: HashSet<String> = records().into_iter().map(|r| r.grid).collect();
    rs.iter()
        .filter(|r| r.snap.bunker().is_some_and(|b| !grids.contains(&b.grid)))
        .count()
}

/// Every digit that was reported (not '?') is a digit of some transmitted record.
fn consistent(r: &BunkerRecord) -> bool {
    truth().iter().any(|t| {
        r.digits
            .iter()
            .zip(t.chars())
            .all(|(&d, c)| d == '?' || d == c)
    })
}

#[test]
fn bunkers_have_unique_six_digit_grids() {
    let b = bunkers();
    assert_eq!(b.len(), 23);
    assert_eq!(b.iter().map(|x| &x.grid).collect::<HashSet<_>>().len(), 23);
    for x in b {
        assert!(x.grid.len() == 6 && x.grid.bytes().all(|c| c.is_ascii_digit()));
    }
}

#[test]
fn snap_grid_exact_inferred_corrected_none() {
    let g = bunkers()[3].grid.clone();
    assert_eq!(snap_grid(&g).kind(), SnapKind::Exact);
    assert_eq!(
        snap_grid(&format!("{}??", &g[..4])).kind(),
        SnapKind::Inferred
    );
    let last = g.as_bytes()[5] - b'0';
    assert_eq!(
        snap_grid(&format!("{}{}", &g[..5], (last + 1) % 10)).kind(),
        SnapKind::Corrected
    );
    assert_eq!(snap_grid(&format!("???{}", &g[3..])).kind(), SnapKind::None);
    assert_eq!(snap_grid("12345").kind(), SnapKind::None);
}

#[test]
fn plain_samples_every_digit_of_every_record() {
    let r = run(&base());
    assert_eq!(got(&r.records), expected(2));
    assert!(r.records.iter().all(|x| {
        x.snap.kind() == SnapKind::Exact
            && x.support.iter().copied().fold(f32::INFINITY, f32::min) > 0.9
    }));
}

#[test]
fn beep_length_and_pitch_varying() {
    let r = run(&SynthOptions {
        variance: 1.0,
        seed: 7,
        ..base()
    });
    assert_eq!(got(&r.records), expected(2));
}

#[test]
fn static_bed_twice_as_loud() {
    let r = run(&SynthOptions {
        static_gain: 2.0,
        ..base()
    });
    assert_eq!(got(&r.records), expected(2));
}

#[test]
fn wind_alone_gives_no_bursts() {
    for wind_gain in [1.0, 8.0] {
        let r = run(&SynthOptions {
            records: vec![],
            repeats: 0,
            wind_gain,
            lead_s: 60.0,
            ..base()
        });
        assert_eq!(r.bursts.len(), 0);
    }
}

#[test]
fn wind_under_the_message_does_not_change_a_digit() {
    let r = run(&SynthOptions {
        wind_gain: 3.0,
        ..base()
    });
    assert_eq!(got(&r.records), expected(2));
}

#[test]
fn wind_far_above_half_a_beep_is_unreadable_but_never_wrong_or_false() {
    let mut reported = 0;
    let mut possible = 0;
    for wind_gain in [6.0, 10.0, 25.0] {
        for seed in [1, 2] {
            let r = run(&SynthOptions {
                wind_gain,
                seed,
                repeats: 1,
                ..base()
            });
            possible += 3;
            reported += r.records.len();
            // no record the broadcast did not contain, no digit that was not sent, no snap to a bunker not heard
            assert!(
                r.records.iter().all(consistent),
                "gain {wind_gain} seed {seed}: {:?}",
                got(&r.records)
            );
            assert_eq!(wrong_snaps(&r.records), 0, "gain {wind_gain} seed {seed}");
            assert!(r.records.len() <= 3, "gain {wind_gain} seed {seed}");
            // a record that reads is allowed only with a snap that is none, inferred or exact on a heard bunker
            for x in &r.records {
                if let Some(b) = x.snap.bunker() {
                    assert!(records().iter().any(|t| t.grid == b.grid));
                }
            }
        }
    }
    assert!(reported < possible, "{reported} of {possible} records read");
}

#[test]
fn pings_before_and_after_are_ignored() {
    use nasdara_decoder::synth::PingZone;
    let r = run(&SynthOptions {
        pings: 2,
        seed: 3,
        lead_s: 5.0,
        ping_zone: PingZone::Outside,
        ..base()
    });
    assert_eq!(got(&r.records), expected(2));
}

#[test]
fn clustered_pings_never_produce_a_wrong_digit() {
    use nasdara_decoder::synth::PingZone;
    let r = run(&SynthOptions {
        pings: 3,
        seed: 3,
        lead_s: 5.0,
        ping_zone: PingZone::Cluster,
        ..base()
    });
    assert!(r.records.len() >= 5, "records: {}", r.records.len());
    assert!(r.records.iter().all(consistent));
}

#[test]
fn pings_on_the_message_voted_codes_are_right() {
    use nasdara_decoder::synth::PingZone;
    let r = run(&SynthOptions {
        pings: 8,
        seed: 3,
        ping_zone: PingZone::Anywhere,
        repeats: 3,
        ..base()
    });
    assert_eq!(voted_codes(&r.records), codes());
    assert_eq!(wrong_snaps(&r.records), 0);
}

#[test]
fn dropouts_and_lost_beeps_voted_codes_are_right() {
    for seed in [1, 2] {
        let r = run(&SynthOptions {
            dropout: 0.3,
            lose: 0.04,
            seed,
            repeats: 3,
            ..base()
        });
        assert_eq!(voted_codes(&r.records), codes(), "seed {seed}");
        assert_eq!(wrong_snaps(&r.records), 0, "seed {seed}");
    }
}

#[test]
fn lost_first_dash_single_passes_can_be_wrong_the_vote_is_not() {
    let r = run(&SynthOptions {
        lose: 0.08,
        seed: 3,
        repeats: 3,
        ..base()
    });
    assert_eq!(voted_codes(&r.records), codes());
}

#[test]
fn heavy_dropouts_alone_every_pass_reads_a_bunker() {
    let r = run(&SynthOptions {
        dropout: 0.4,
        seed: 5,
        repeats: 3,
        ..base()
    });
    assert!(r.records.len() >= 8, "records: {}", r.records.len());
    assert_eq!(voted_codes(&r.records), codes());
}

#[test]
fn capture_starting_mid_message_never_snaps_to_a_wrong_bunker() {
    let r = run(&SynthOptions {
        skip_elements: 37,
        repeats: 1,
        ..base()
    });
    let b = bunkers();
    for x in &r.records {
        if let Some(snapped) = x.snap.bunker() {
            let want = [0, 9, 16].into_iter().find(|&i| b[i].grid == snapped.grid);
            assert_eq!(Some(snapped.index - 1), want);
        }
    }
}

#[test]
fn loud_noise_only_gives_no_bursts() {
    let r = run(&SynthOptions {
        records: vec![],
        repeats: 0,
        static_gain: 6.0,
        lead_s: 20.0,
        ..base()
    });
    assert_eq!(r.bursts.len(), 0);
}

fn record(bunker: usize, digits: &str, support: f32) -> BunkerRecord {
    BunkerRecord {
        digits: digits.chars().collect(),
        support: digits
            .chars()
            .map(|d| if d == '?' { 0.0 } else { support })
            .collect(),
        margins: vec![],
        grid: digits[..6].into(),
        code: digits[6..].into(),
        snap: snap_grid(&bunkers()[bunker].grid),
        start: 0.0,
        digit_starts: vec![],
    }
}

fn g(i: usize) -> String {
    bunkers()[i].grid.clone()
}

#[test]
fn digits_lost_in_one_pass_are_filled_in_by_another() {
    let mut l = BunkerLedger::new();
    assert_eq!(l.add(&record(3, &(g(3) + "5?0?"), 0.9)).code, "5?0?");
    let e = l.add(&record(3, &(g(3) + "?2?7"), 0.9));
    assert_eq!(e.code, "5207");
    assert_eq!(e.unclear, 0);
    assert_eq!(e.count, 2);
}

#[test]
fn a_new_lock_code_replaces_the_old_votes() {
    let mut l = BunkerLedger::new();
    l.add(&record(3, &(g(3) + "5207"), 0.9));
    l.add(&record(3, &(g(3) + "5207"), 0.9));
    assert_eq!(l.add(&record(3, &(g(3) + "1984"), 0.9)).code, "1984");
}

#[test]
fn one_confident_wrong_digit_does_not_flip_an_established_code() {
    let mut l = BunkerLedger::new();
    l.add(&record(3, &(g(3) + "5207"), 0.9));
    l.add(&record(3, &(g(3) + "5207"), 0.9));
    assert_eq!(l.add(&record(3, &(g(3) + "5287"), 0.4)).code, "5207");
}

#[test]
fn bunker_one_is_column_072_row_106() {
    assert_eq!(bunkers()[0].grid, "072106");
    assert_eq!(grid_of(7216.4, 5707.0), "072106");
}

#[test]
fn link_carries_game_x_z_and_zoom() {
    assert_eq!(
        izurvive_url(7216.4, 5706.95, 7),
        "https://www.izurvive.com/nasdara/#location=7216;5707;7"
    );
}

#[test]
fn cell_centre_round_trips_into_the_same_cell() {
    let (x, z) = cell_center("072106").expect("six digits");
    assert_eq!(grid_of(x, z), "072106");
}

#[test]
fn map_link_allow_list_accepts_built_links_refuses_others() {
    for b in bunkers() {
        assert!(is_izurvive_url(&izurvive_url(b.x, b.z, 7)));
    }
    for bad in [
        "https://evil.example/",
        "https://www.izurvive.com/chernarusplus/#location=1;2;7",
        "https://www.izurvive.com/nasdara/#location=1;2;7&x=calc.exe",
        "file:///C:/Windows/System32/calc.exe",
        "",
        "https://www.izurvive.com.evil.example/nasdara/#location=1;2;7",
    ] {
        assert!(!is_izurvive_url(bad), "{bad}");
    }
}
