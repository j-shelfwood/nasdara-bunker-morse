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
const TIE_RATIO: f32 = 0.7;
/// A new lock code (new batch of bunkers) replaces the old votes when this many confident digits disagree.
const RESET_DISAGREEMENTS: usize = 2;
const CONFIDENT: f32 = 0.6;
const ESTABLISHED: f32 = 1.2;

#[derive(Debug, Clone, Default)]
struct Votes {
    code: [[f32; 10]; CODE_DIGITS],
    count: usize,
}

/// Every transmission of a bunker repeats the same message, and each pass loses different digits to noise, so the
/// lock code is read digit by digit over all passes heard, weighted by how clean each digit was.
#[derive(Debug, Default)]
pub struct BunkerLedger {
    votes: HashMap<usize, Votes>,
    loose: HashMap<String, usize>,
}

impl BunkerLedger {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, r: &BunkerRecord) -> Entry {
        let Some(bunker) = r.snap.bunker() else {
            let key = format!("{}/{}", r.grid, r.code);
            let count = self.loose.entry(key.clone()).or_insert(0);
            *count += 1;
            return Entry {
                key,
                grid: r.grid.clone(),
                code: r.code.clone(),
                bunker: None,
                count: *count,
                unclear: r.code.chars().filter(|&c| c == '?').count(),
                corrected: false,
            };
        };
        let heard = r.digits.get(GRID_DIGITS..).unwrap_or(&[]);
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
                v.code[i][n as usize] += s;
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

fn contradicts(v: &Votes, heard: &[char], support: &[f32]) -> bool {
    let n = heard
        .iter()
        .zip(support)
        .take(CODE_DIGITS)
        .enumerate()
        .filter(|&(i, (&d, &s))| {
            if d == '?' || s < CONFIDENT {
                return false;
            }
            let top = argmax(&v.code[i]);
            v.code[i][top] >= ESTABLISHED && d.to_digit(10) != Some(top as u32)
        })
        .count();
    n >= RESET_DISAGREEMENTS
}

fn argmax(w: &[f32]) -> usize {
    w.iter()
        .enumerate()
        .fold(0, |best, (i, &x)| if x > w[best] { i } else { best })
}

fn consensus(w: &[f32; 10]) -> char {
    let top = argmax(w);
    if w[top] <= 0.0 {
        return '?';
    }
    let second = w
        .iter()
        .enumerate()
        .filter(|&(i, _)| i != top)
        .map(|(_, &x)| x)
        .fold(f32::NEG_INFINITY, f32::max);
    if second > TIE_RATIO * w[top] {
        '?'
    } else {
        char::from_digit(top as u32, 10).unwrap_or('?')
    }
}
