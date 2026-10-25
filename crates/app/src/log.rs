//! Optional decoder diagnostics (`--log <file>`): one JSON line per heard burst, beep timings and digits only, no audio.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;

use nasdara_decoder::bursts::Burst;

/// One line bound for a file. Built under the engine's lock, written after it is released: opening a file can take
/// tens of milliseconds when antivirus scans it, and the window waits on that lock.
#[derive(Debug, PartialEq)]
pub struct LogLine {
    path: PathBuf,
    text: String,
}

pub struct DebugLog {
    path: PathBuf,
    trace: bool,
}

impl DebugLog {
    pub fn new(path: PathBuf, trace: bool) -> DebugLog {
        DebugLog { path, trace }
    }

    pub fn burst(&self, b: &Burst, shown: usize) -> Vec<LogLine> {
        let mut out = Vec::new();
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
        let guess = b
            .guess
            .as_ref()
            .map(|g| serde_json::json!({ "digits": g.digits, "support": g.support }));
        let line = serde_json::json!({
            "at": unix_seconds(),
            "elements": b.elements,
            "ignored": b.ignored,
            "note": b.note,
            "shown": shown,
            "records": records,
            "guess": guess,
            "beeps": b.beeps.iter().map(|x| [round2(x.start), round2(x.dur)]).collect::<Vec<_>>(),
        });
        out.push(LogLine {
            path: self.path.clone(),
            text: line.to_string(),
        });
        if let (true, Some(trace)) = (self.trace, &b.trace) {
            let t = serde_json::json!({ "at": unix_seconds(), "t0": trace.t0, "db": trace.db, "rms": trace.rms_db });
            let mut name = self.path.clone().into_os_string();
            name.push(".trace.jsonl");
            out.push(LogLine {
                path: PathBuf::from(name),
                text: t.to_string(),
            });
        }
        out
    }

    /// Appends the lines to their files.
    pub fn write(lines: Vec<LogLine>) {
        for l in lines {
            append(&l.path, &l.text);
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
