use crate::bunkers::Bunker;
use crate::bursts::BunkerRecord;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub key: String,
    pub grid: String,
    pub code: String,
    pub bunker: Option<&'static Bunker>,
    /// Transmissions heard for this bunker.
    pub count: usize,
    /// Code digits still unreadable after voting over every transmission heard.
    pub unclear: usize,
    pub corrected: bool,
}

const CODE_DIGITS: usize = 4;
const GRID_DIGITS: usize = 6;
/// A runner-up digit this close to the leader's weight leaves the position unreadable.
const TIE_RATIO: f64 = 0.7;
/// A new lock code (new batch of bunkers) replaces the old votes when this many confident digits disagree.
const RESET_DISAGREEMENTS: usize = 2;
const CONFIDENT: f64 = 0.6;
/// Most distinct unsnapped records kept; beyond it the least recently heard is dropped.
const MAX_LOOSE: usize = 256;
const ESTABLISHED: f64 = 1.2;
/// Weight a digit needs before it is shown: one pass at the pipeline's `MIN_SUPPORT`, or several weaker passes that
/// agree. Below it the position stays '?'.
const SHOWN: f64 = 0.35;

#[derive(Debug, Clone, Default)]
struct Votes {
    code: [[f64; 10]; CODE_DIGITS],
    count: usize,
}

/// Every transmission of a bunker repeats the same message, and each pass loses different digits to noise, so the
/// lock code is read digit by digit over all passes heard, weighted by how clean each digit was.
#[derive(Debug, Default)]
pub struct BunkerLedger {
    votes: HashMap<usize, Votes>,
    /// Records that snapped to no bunker: key to (times heard, sequence of the last hearing).
    loose: HashMap<String, (usize, u64)>,
    seq: u64,
}

impl BunkerLedger {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, r: &BunkerRecord) -> Entry {
        let Some(bunker) = r.snap.bunker() else {
            let key = format!("{}/{}", r.grid, r.code);
            self.seq += 1;
            let slot = self.loose.entry(key.clone()).or_insert((0, 0));
            slot.0 += 1;
            slot.1 = self.seq;
            let count = slot.0;
            if self.loose.len() > MAX_LOOSE
                && let Some(oldest) = self
                    .loose
                    .iter()
                    .min_by_key(|(_, v)| v.1)
                    .map(|(k, _)| k.clone())
            {
                self.loose.remove(&oldest);
            }
            return Entry {
                key,
                grid: r.grid.clone(),
                code: r.code.clone(),
                bunker: None,
                count,
                unclear: r.code.chars().filter(|&c| c == '?').count(),
                corrected: false,
            };
        };
        // Weak passes vote too: a digit the pipeline showed as '?' still carries its raw reading.
        let digits = if r.raw.is_empty() { &r.digits } else { &r.raw };
        let heard = digits.get(GRID_DIGITS..).unwrap_or(&[]);
        let support = r.support.get(GRID_DIGITS..).unwrap_or(&[]);
        let contradicted = self
            .votes
            .get(&bunker.index)
            .is_some_and(|v| contradicts(v, heard, support));
        if contradicted {
            self.votes.remove(&bunker.index);
        }
        let v = self.votes.entry(bunker.index).or_default();
        for (i, &d) in heard.iter().enumerate().take(CODE_DIGITS) {
            if let (Some(n), Some(&s)) = (d.to_digit(10), support.get(i)) {
                v.code[i][n as usize] += widen(s).max(0.0);
            }
        }
        v.count += 1;
        let code: String = v.code.iter().map(consensus).collect();
        Entry {
            key: format!("b{}", bunker.index),
            grid: bunker.grid.clone(),
            unclear: code.chars().filter(|&c| c == '?').count(),
            code,
            bunker: Some(bunker),
            count: v.count,
            corrected: r.snap.kind() == crate::bunkers::SnapKind::Corrected,
        }
    }
}

/// The record's support as the decimal it prints (`Number(s.toFixed(2))` in the prototype), so that sums and ratio
/// thresholds see 1.05, not the nearest f32 widened to 1.0499999523.
fn widen(s: f32) -> f64 {
    format!("{s}").parse().unwrap_or(f64::from(s))
}

fn contradicts(v: &Votes, heard: &[char], support: &[f32]) -> bool {
    let n = heard
        .iter()
        .zip(support)
        .take(CODE_DIGITS)
        .enumerate()
        .filter(|&(i, (&d, &s))| {
            if d == '?' || widen(s) < CONFIDENT {
                return false;
            }
            let top = argmax(&v.code[i]);
            v.code[i][top] >= ESTABLISHED && d.to_digit(10) != Some(top as u32)
        })
        .count();
    n >= RESET_DISAGREEMENTS
}

fn argmax(w: &[f64]) -> usize {
    w.iter()
        .enumerate()
        .fold(0, |best, (i, &x)| if x > w[best] { i } else { best })
}

fn consensus(w: &[f64; 10]) -> char {
    let top = argmax(w);
    if w[top] < SHOWN {
        return '?';
    }
    let second = w
        .iter()
        .enumerate()
        .filter(|&(i, _)| i != top)
        .map(|(_, &x)| x)
        .fold(f64::NEG_INFINITY, f64::max);
    if second > TIE_RATIO * w[top] {
        '?'
    } else {
        char::from_digit(top as u32, 10).unwrap_or('?')
    }
}
