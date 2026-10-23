//! Optional decoder diagnostics (`--log <file>`): one JSON line per heard burst, beep timings and digits only, no audio.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;

use nasdara_decoder::bursts::Burst;

pub struct DebugLog {
    path: PathBuf,
    trace: bool,
}

impl DebugLog {
    pub fn new(path: PathBuf, trace: bool) -> DebugLog {
        DebugLog { path, trace }
    }

    pub fn burst(&self, b: &Burst, shown: usize) {
        let records: Vec<_> = b
            .records
            .iter()
            .map(|r| {
                serde_json::json!({
                    "digits": r.digits.iter().collect::<String>(),
                    "support": r.support.iter().map(|s| (s * 100.0).round() / 100.0).collect::<Vec<_>>(),
                    "snap": format!("{:?}", r.snap.kind()),
                })
            })
            .collect();
        let line = serde_json::json!({
            "at": unix_seconds(),
            "elements": b.elements,
            "ignored": b.ignored,
            "note": b.note,
            "shown": shown,
            "records": records,
            "beeps": b.beeps.iter().map(|x| [round2(x.start), round2(x.dur)]).collect::<Vec<_>>(),
        });
        append(&self.path, &line.to_string());
        if let (true, Some(trace)) = (self.trace, &b.trace) {
            let t = serde_json::json!({ "at": unix_seconds(), "t0": trace.t0, "db": trace.db });
            let mut name = self.path.clone().into_os_string();
            name.push(".trace.jsonl");
            append(&PathBuf::from(name), &t.to_string());
        }
    }
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

fn unix_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn append(path: &PathBuf, line: &str) {
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "{line}");
    }
}
