use egui::{Align2, Color32, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui, Vec2};

use crate::theme::{self, AMBER, DIM, FAINT, STROKE, SURFACE, SURFACE_HI, TEXT, alpha};

pub fn card(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    egui::Frame::new()
        .fill(SURFACE)
        .stroke(Stroke::new(1.0, STROKE))
        .corner_radius(14.0)
        .inner_margin(egui::Margin::symmetric(20, 16))
        .show(ui, add);
}

/// Small uppercase caption in the dim colour, with the letter spacing a label needs.
pub fn caption(ui: &mut Ui, text: &str) {
    let galley = ui
        .painter()
        .layout_no_wrap(spaced(text), theme::bold(11.0), DIM);
    let (rect, _) = ui.allocate_exact_size(galley.size(), Sense::hover());
    ui.painter().galley(rect.min, galley, DIM);
}

pub fn spaced(text: &str) -> String {
    let mut out = String::new();
    for (i, c) in text.to_uppercase().chars().enumerate() {
        if i > 0 {
            out.push('\u{200a}');
        }
        out.push(c);
    }
    out
}

pub fn paint_caption(
    painter: &egui::Painter,
    pos: Pos2,
    align: Align2,
    text: &str,
    color: Color32,
) {
    painter.text(pos, align, spaced(text), theme::bold(11.0), color);
}

pub struct Pill {
    pub dot: Color32,
    pub pulse: f32,
    pub label: String,
}

/// Rounded status chip with a coloured dot; `pulse` (0..1) breathes a halo around the dot.
pub fn pill(ui: &mut Ui, p: &Pill) -> Response {
    let galley = ui
        .painter()
        .layout_no_wrap(p.label.clone(), theme::text(13.5), TEXT);
    let size = Vec2::new(galley.size().x + 44.0, 30.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, 15.0, SURFACE_HI);
    painter.rect_stroke(rect, 15.0, Stroke::new(1.0, STROKE), StrokeKind::Inside);
    let c = Pos2::new(rect.left() + 18.0, rect.center().y);
    if p.pulse > 0.0 {
        painter.circle_filled(c, 4.0 + 5.0 * p.pulse, alpha(p.dot, 0.35 * (1.0 - p.pulse)));
    }
    painter.circle_filled(c, 4.0, p.dot);
    painter.galley(
        Pos2::new(rect.left() + 32.0, rect.center().y - galley.size().y / 2.0),
        galley,
        TEXT,
    );
    response
}

/// Two-option switch. Returns the new selection when clicked.
pub fn segmented(ui: &mut Ui, labels: [&str; 2], selected: usize) -> Option<usize> {
    let widths: Vec<f32> = labels
        .iter()
        .map(|l| {
            ui.painter()
                .layout_no_wrap((*l).into(), theme::text(13.0), TEXT)
                .size()
                .x
                + 28.0
        })
        .collect();
    let size = Vec2::new(widths.iter().sum::<f32>() + 6.0, 30.0);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter().rect_filled(rect, 15.0, SURFACE_HI);
    ui.painter()
        .rect_stroke(rect, 15.0, Stroke::new(1.0, STROKE), StrokeKind::Inside);
    let mut x = rect.left() + 3.0;
    let mut clicked = None;
    for (i, l) in labels.iter().enumerate() {
        let r = Rect::from_min_size(Pos2::new(x, rect.top() + 3.0), Vec2::new(widths[i], 24.0));
        let resp = ui.interact(r, ui.make_persistent_id(("segment", i)), Sense::click());
        let on = i == selected;
        if on {
            ui.painter().rect_filled(r, 12.0, alpha(AMBER, 0.18));
            ui.painter().rect_stroke(
                r,
                12.0,
                Stroke::new(1.0, alpha(AMBER, 0.55)),
                StrokeKind::Inside,
            );
        } else if resp.hovered() {
            ui.painter().rect_filled(r, 12.0, alpha(TEXT, 0.06));
        }
        ui.painter().text(
            r.center(),
            Align2::CENTER_CENTER,
            *l,
            theme::text(13.0),
            if on { AMBER } else { DIM },
        );
        if resp.clicked() && !on {
            clicked = Some(i);
        }
        x += widths[i];
    }
    clicked
}

pub fn icon_button(
    ui: &mut Ui,
    active: bool,
    tip: &str,
    paint: impl FnOnce(&egui::Painter, Rect, Color32),
) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(30.0), Sense::click());
    let hot = response.hovered();
    let fill = if active {
        alpha(AMBER, 0.18)
    } else if hot {
        SURFACE_HI
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, 15.0, fill);
    let color = if active {
        AMBER
    } else if hot {
        TEXT
    } else {
        DIM
    };
    paint(ui.painter(), rect.shrink(8.0), color);
    response.on_hover_text(tip)
}

pub enum ButtonKind {
    Primary,
    Quiet,
}

pub fn button(ui: &mut Ui, label: &str, kind: ButtonKind) -> Response {
    let galley = ui
        .painter()
        .layout_no_wrap(label.into(), theme::bold(13.0), TEXT);
    let size = Vec2::new(galley.size().x + 28.0, 32.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let (fill, stroke, ink) = match kind {
        ButtonKind::Primary => (
            if response.hovered() {
                theme::mix(AMBER, Color32::WHITE, 0.15)
            } else {
                AMBER
            },
            Stroke::NONE,
            Color32::from_rgb(0x1a, 0x12, 0x02),
        ),
        ButtonKind::Quiet => (
            if response.hovered() {
                theme::mix(SURFACE_HI, TEXT, 0.08)
            } else {
                SURFACE_HI
            },
            Stroke::new(1.0, if response.hovered() { FAINT } else { STROKE }),
            TEXT,
        ),
    };
    ui.painter().rect_filled(rect, 9.0, fill);
    if stroke != Stroke::NONE {
        ui.painter()
            .rect_stroke(rect, 9.0, stroke, StrokeKind::Inside);
    }
    ui.painter()
        .galley(rect.center() - galley.size() / 2.0, galley, ink);
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Width of `symbols` drawn at scale 1.
pub fn symbols_width(symbols: &str) -> f32 {
    let dashes = symbols.chars().filter(|&c| c == '-').count() as f32;
    let all = symbols.chars().count() as f32;
    dashes * 12.0 + (all - dashes) * 4.4 + 3.6 * (all - 1.0).max(0.0)
}

/// Morse symbols as small shapes: `.` a dot, `-` a dash.
pub fn paint_symbols(
    painter: &egui::Painter,
    center: Pos2,
    symbols: &str,
    scale: f32,
    color: Color32,
) {
    let (dot, dash, gap) = (4.4 * scale, 12.0 * scale, 3.6 * scale);
    let widths: Vec<f32> = symbols
        .chars()
        .map(|c| if c == '-' { dash } else { dot })
        .collect();
    let total = widths.iter().sum::<f32>() + gap * (widths.len().saturating_sub(1)) as f32;
    let mut x = center.x - total / 2.0;
    for w in widths {
        let r = Rect::from_min_size(Pos2::new(x, center.y - dot / 2.0), Vec2::new(w, dot));
        painter.rect_filled(r, dot / 2.0, color);
        x += w + gap;
    }
}
