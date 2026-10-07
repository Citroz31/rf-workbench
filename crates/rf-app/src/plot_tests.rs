use eframe::egui::{self, Event, Modifiers, PointerButton, Pos2, Rect};
#[test]
fn chart_corner_drag_changes_height() {
    let ctx = egui::Context::default();
    let frame = |events: Vec<Event>| {
        let mut rect = Rect::NOTHING;
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1000., 900.))),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let origin = ui.cursor().min;
                    let width = ui.available_width();
                    crate::plot::series(ui, &[1., 2., 3.], &[2., 3., 4.], 240., "Hz", "V", 1.);
                    rect = Rect::from_min_max(
                        origin,
                        Pos2::new(
                            origin.x + width,
                            ui.cursor().top() - ui.spacing().item_spacing.y,
                        ),
                    );
                });
            },
        );
        rect
    };
    frame(vec![]);
    let before = frame(vec![]);
    let corner = before.right_bottom() - egui::vec2(4., 4.);
    frame(vec![Event::PointerMoved(corner)]);
    frame(vec![Event::PointerButton {
        pos: corner,
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    }]);
    let next = corner + egui::vec2(0., 90.);
    frame(vec![Event::PointerMoved(next)]);
    frame(vec![Event::PointerButton {
        pos: next,
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    }]);
    let after = frame(vec![]);
    assert!(
        after.height() > before.height() + 60.,
        "{before:?} → {after:?}"
    );
}
