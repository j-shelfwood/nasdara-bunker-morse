//! The options menu behind the gear, the status chip text and the banner strips under the header.

use egui::{Color32, RichText, Stroke, Ui};

use crate::capture::{Source, Status};
use crate::engine::{Command, Handle, Mode, Note, Shared};
use crate::theme::{self, AMBER, BLUE, DIM, FAINT, RED, TEAL, TEXT, alpha};
use crate::ui::UiState;
use crate::ui::widgets::{self, ButtonKind, button};

/// Content of the gear menu: display switches, test mode, attention switches and the two reset actions.
pub fn menu(ui: &mut Ui, engine: &Handle, st: &mut UiState, s: &Shared) {
    ui.set_min_width(300.0);
    let still = st.settings.reduce_motion;
    widgets::toggle(ui, &mut st.settings.show_guide, "Morse cheat sheet", still);
    let mut test = s.mode == Mode::Demo;
    let test_toggle = ui
        .add_enabled_ui(!s.frozen, |ui| {
            widgets::toggle(ui, &mut test, "Test mode: a simulated broadcast", still)
        })
        .inner
        .on_hover_text(
            "Plays a made-up transmission on repeat, with sound on Windows, to try the window \
             without the game. Turning it on or off forgets the bunkers heard so far.",
        )
        .on_disabled_hover_text("Unfreeze the bunkers first");
    if test_toggle.changed() {
        engine.send(if test {
            Command::StartDemo
        } else {
            Command::SetSource(st.settings.source)
        });
        engine.send(Command::ClearCards);
    }
    ui.separator();
    widgets::toggle(
        ui,
        &mut st.settings.flash_taskbar,
        "Flash the taskbar when a bunker is read",
        still,
    );
    widgets::toggle(
        ui,
        &mut st.settings.open_map_for_new,
        "Open the map for each new bunker",
        still,
    );
    widgets::toggle(
        ui,
        &mut st.settings.reduce_motion,
        "Reduce motion (no pulses, glows or blinking)",
        still,
    );
    ui.separator();
    let forget = ui
        .add_enabled(
            !s.frozen,
            egui::Button::new("Forget the bunkers heard so far"),
        )
        .on_disabled_hover_text("Unfreeze the bunkers first");
    if forget.clicked() {
        engine.send(Command::ClearCards);
    }
    if ui.button("Show the walkthrough again").clicked() {
        st.onboarding = Some(0);
    }
}

/// Label, dot colour and halo phase (0 for none) of the status chip.
pub fn status_chip(s: &Shared, time: f32) -> (String, Color32, f32) {
    let breathe = |speed: f32| (time * speed).fract();
    if s.mode == Mode::Demo {
        return ("Test mode, simulated broadcast".into(), BLUE, breathe(0.8));
    }
    if s.receiving() {
        return ("Receiving a transmission".into(), AMBER, breathe(1.6));
    }
    match &s.status {
        Status::Listening => (
            if s.source == Source::Game {
                "Listening to DayZ".into()
            } else {
                "Listening to system audio".into()
            },
            TEAL,
            // an idle window repaints a few times a second, too slowly for a smooth breathing dot
            0.0,
        ),
        Status::WaitingForGame => ("Waiting for DayZ".into(), FAINT, 0.0),
        Status::Failed(_) => ("Cannot listen".into(), RED, 0.0),
    }
}

pub struct Strip {
    color: Color32,
    text: String,
    action: Option<&'static str>,
}

/// The warnings and notes to show under the header, most urgent first.
pub fn strips(s: &Shared) -> Vec<Strip> {
    let silent = s.mode == Mode::Live
        && s.source == Source::Game
        && s.status == Status::Listening
        && s.silent_for > 20.0;
    let mut lines = Vec::new();
    let mut add = |color, text: String, action| {
        lines.push(Strip {
            color,
            text,
            action,
        })
    };
    if silent {
        add(
            AMBER,
            format!(
                "Nothing audible from DayZ for {} seconds. On some PCs Windows does not hand the game's sound to the app.",
                s.silent_for as u64
            ),
            Some("Listen to system audio"),
        );
    }
    if !s.engine_alive {
        add(
            RED,
            "Listening stopped unexpectedly. Close the app and open it again.".into(),
            None,
        );
    }
    if let Status::Failed(e) = &s.status {
        add(RED, e.clone(), None);
    }
    for n in &s.notes {
        match n {
            Note::Info(t) => add(DIM, t.clone(), None),
            Note::Warn(t) => add(AMBER, t.clone(), None),
        }
    }
    lines
}

/// One slim strip per line; the action button of the silent-audio strip switches to system audio.
pub fn banner(ui: &mut Ui, engine: &Handle, lines: &[Strip]) {
    for line in lines {
        let mut act = false;
        egui::Frame::new()
            .fill(alpha(line.color, 0.08))
            .stroke(Stroke::new(1.0, alpha(line.color, 0.35)))
            .corner_radius(6.0)
            .inner_margin(egui::Margin::symmetric(12, 5))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                // the label wraps and the button drops to the next line when the row is full
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        RichText::new(&line.text)
                            .font(theme::text(12.5))
                            .color(TEXT),
                    );
                    if let Some(a) = line.action {
                        act = button(ui, a, ButtonKind::Quiet).clicked();
                    }
                });
            });
        if act {
            engine.send(Command::SetSource(Source::System));
        }
    }
}
