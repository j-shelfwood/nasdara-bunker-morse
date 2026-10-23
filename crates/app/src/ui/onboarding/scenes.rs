use egui::{Align2, Pos2, Rect, Sense, Stroke, Ui, Vec2};

use crate::theme::{self, AMBER, DIM, FAINT, RULE, SURFACE_HI, TEAL, TEXT, alpha};
use crate::ui::widgets::paint_symbols;

/// A looping picture for each page, painted from the same shapes the live ribbon uses.
pub(super) fn illustration(ui: &mut Ui, page: usize, width: f32, still: bool) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 150.0), Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(
        rect,
        egui::CornerRadius {
            nw: 16,
            ne: 16,
            sw: 0,
            se: 0,
        },
        theme::BG,
    );
    // a still picture holds the frame where the pictures read best
    let t = if still {
        1.2
    } else {
        ui.input(|i| i.time) as f32
    };
    if !still {
        ui.ctx().request_repaint();
    }
    match page {
        0 => morse_loop(&p, rect, t),
        1 => radio_scene(&p, rect, t),
        _ => shield(&p, rect, t),
    }
}

fn morse_loop(p: &egui::Painter, rect: Rect, t: f32) {
    // a message sliding in as Morse and settling into digits
    let message = [('1', ".----"), ('2', "..---"), ('9', "----.")];
    let cycle = (t * 0.35).fract();
    let y = rect.center().y;
    for (i, (digit, symbols)) in message.iter().enumerate() {
        let cx = rect.center().x + (i as f32 - 1.0) * 150.0;
        let local = ((cycle * 3.0 - i as f32 * 0.55) / 1.0).clamp(0.0, 1.0);
        paint_symbols(
            p,
            Pos2::new(cx, y - 18.0),
            symbols,
            1.7,
            alpha(AMBER, 0.35 + 0.65 * (1.0 - local)),
        );
        p.text(
            Pos2::new(cx, y + 28.0),
            Align2::CENTER_CENTER,
            digit.to_string(),
            theme::digits(44.0),
            alpha(TEXT, local),
        );
    }
}

fn radio_scene(p: &egui::Painter, rect: Rect, t: f32) {
    let c = Pos2::new(rect.center().x - 120.0, rect.center().y + 18.0);
    // radio body
    let body = Rect::from_center_size(c, Vec2::new(70.0, 52.0));
    p.rect_filled(body, 9.0, SURFACE_HI);
    p.rect_stroke(body, 9.0, Stroke::new(1.5, DIM), egui::StrokeKind::Inside);
    p.line_segment(
        [
            body.right_top() - Vec2::new(14.0, 0.0),
            body.right_top() + Vec2::new(-6.0, -30.0),
        ],
        Stroke::new(2.0, DIM),
    );
    p.circle_filled(body.center() - Vec2::new(14.0, 0.0), 10.0, theme::BG);
    p.circle_stroke(
        body.center() - Vec2::new(14.0, 0.0),
        10.0,
        Stroke::new(1.2, RULE),
    );
    p.rect_filled(
        Rect::from_center_size(body.center() + Vec2::new(16.0, -8.0), Vec2::new(18.0, 6.0)),
        2.0,
        alpha(TEAL, 0.5 + 0.4 * (t * 2.0).sin().abs()),
    );
    // waves from a bunker on the right travelling to the radio
    let origin = Pos2::new(rect.center().x + 150.0, rect.center().y + 10.0);
    p.rect_filled(
        Rect::from_center_size(origin + Vec2::new(0.0, 8.0), Vec2::new(44.0, 26.0)),
        4.0,
        SURFACE_HI,
    );
    p.rect_stroke(
        Rect::from_center_size(origin + Vec2::new(0.0, 8.0), Vec2::new(44.0, 26.0)),
        4.0,
        Stroke::new(1.5, RULE),
        egui::StrokeKind::Inside,
    );
    p.text(
        origin + Vec2::new(0.0, -18.0),
        Align2::CENTER_CENTER,
        "bunker",
        theme::text(12.0),
        FAINT,
    );
    for k in 0..4 {
        let phase = (t * 0.6 + k as f32 * 0.25).fract();
        let x = origin.x - 40.0 - phase * 190.0;
        p.line_segment(
            [
                Pos2::new(x, origin.y - 14.0 - 12.0 * phase),
                Pos2::new(x, origin.y + 14.0 + 12.0 * phase),
            ],
            Stroke::new(2.0, alpha(AMBER, 0.8 * (1.0 - phase))),
        );
    }
}

fn shield(p: &egui::Painter, rect: Rect, t: f32) {
    let c = rect.center();
    // sound in, digits out, nothing leaving: a closed box
    let boxr = Rect::from_center_size(c, Vec2::new(150.0, 86.0));
    p.rect_filled(boxr, 12.0, SURFACE_HI);
    p.rect_stroke(
        boxr,
        12.0,
        Stroke::new(1.5, alpha(TEAL, 0.6)),
        egui::StrokeKind::Inside,
    );
    p.text(c, Align2::CENTER_CENTER, "your PC", theme::bold(15.0), TEXT);
    for k in 0..3 {
        let phase = (t * 0.5 + k as f32 / 3.0).fract();
        let x = boxr.left() - 130.0 + phase * 120.0;
        p.circle_filled(
            Pos2::new(x, c.y),
            4.0,
            alpha(AMBER, (1.0 - (phase - 0.5).abs() * 2.0).max(0.0)),
        );
    }
    p.text(
        Pos2::new(boxr.left() - 70.0, c.y - 28.0),
        Align2::CENTER_CENTER,
        "game audio",
        theme::text(12.0),
        FAINT,
    );
    p.line_segment(
        [
            boxr.right_center() + Vec2::new(14.0, 0.0),
            boxr.right_center() + Vec2::new(70.0, 0.0),
        ],
        Stroke::new(1.5, alpha(RULE, 0.9)),
    );
    p.text(
        boxr.right_center() + Vec2::new(44.0, -16.0),
        Align2::CENTER_CENTER,
        "no network",
        theme::text(12.0),
        FAINT,
    );
    p.line_segment(
        [
            boxr.right_center() + Vec2::new(64.0, -8.0),
            boxr.right_center() + Vec2::new(78.0, 8.0),
        ],
        Stroke::new(2.0, theme::RED),
    );
    p.line_segment(
        [
            boxr.right_center() + Vec2::new(64.0, 8.0),
            boxr.right_center() + Vec2::new(78.0, -8.0),
        ],
        Stroke::new(2.0, theme::RED),
    );
}
