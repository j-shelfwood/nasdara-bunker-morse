//! The listening side: takes audio from a source, runs the decoder and keeps the state the window draws.

mod audio;
mod state;
#[cfg(test)]
mod tests;
mod worker;

pub use state::{Beep, Card, Finished, Mode, Note, Shared};

use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex, MutexGuard};

use crate::capture::Source;
use crate::log::DebugLog;

use worker::Engine;

/// How much history the window can draw.
pub const HISTORY_S: f64 = 60.0;

pub enum Command {
    SetSource(Source),
    StartDemo,
    ClearCards,
}

#[derive(Clone)]
pub struct Handle {
    shared: Arc<Mutex<Shared>>,
    commands: Sender<Command>,
}

impl Handle {
    pub fn lock(&self) -> MutexGuard<'_, Shared> {
        self.shared
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn send(&self, c: Command) {
        let _ = self.commands.send(c);
    }
}

pub fn start(source: Source, ctx: egui::Context, log: Option<DebugLog>) -> Handle {
    let shared = Arc::new(Mutex::new(Shared::new(source)));
    let (commands, command_rx) = mpsc::channel();
    let handle = Handle {
        shared: shared.clone(),
        commands,
    };
    let engine = Engine::new(shared, ctx, log, source);
    std::thread::Builder::new()
        .name("engine".into())
        .spawn(move || engine.run(command_rx))
        .expect("engine thread");
    handle
}
