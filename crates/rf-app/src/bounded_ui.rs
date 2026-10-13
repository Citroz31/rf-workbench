//! Canvas-like children must not enlarge their resizable parent from their contents.
use eframe::egui::{self, Rect, Ui};

pub fn region<R>(ui: &mut Ui, rect: Rect, id: impl std::hash::Hash, render: impl FnOnce(&mut Ui) -> R) -> R {
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect).id_salt(id));
    child.set_clip_rect(rect.intersect(ui.clip_rect()));
    render(&mut child)
}

pub fn fill<R>(ui: &mut Ui, id: impl std::hash::Hash, render: impl FnOnce(&mut Ui) -> R) -> (egui::Vec2, R) {
    let size = ui.available_size().max(egui::vec2(1., 1.));
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    (size, region(ui, rect, id, render))
}

pub fn library(ctx: &egui::Context, width: f32, revision: u64, render: impl FnOnce(&mut Ui)) -> f32 {
    egui::SidePanel::left(egui::Id::new(("library", revision)))
        .resizable(true).default_width(width).width_range(150. ..=600.)
        .show(ctx, |ui| { fill(ui, "palette-content", render); })
        .response.rect.width().clamp(150., 600.)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn palette_can_shrink_by_pointer_drag_despite_wide_children() {
        let ctx = egui::Context::default();
        let mut width = 480.;
        let mut render = |events| {
            let _ = ctx.run(egui::RawInput {
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280., 800.))),
                events, ..Default::default()
            }, |ctx| {
                width = library(ctx, width, 0, |ui| {
                    ui.label("A deliberately long component category that used to force the palette width");
                    ui.add_sized([560., 24.], egui::Button::new("Long component search control"));
                    egui::ScrollArea::vertical().show(ui, |ui| { ui.label("Modulation / synchronisation"); });
                });
            });
            width
        };
        for _ in 0..5 { assert!((render(vec![]) - 480.).abs() < 2.); }
        let start = egui::pos2(480., 400.);
        render(vec![egui::Event::PointerMoved(start)]);
        render(vec![egui::Event::PointerButton { pos: start, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE }]);
        let end = egui::pos2(180., 400.);
        render(vec![egui::Event::PointerMoved(end)]);
        render(vec![egui::Event::PointerButton { pos: end, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE }]);
        for _ in 0..60 { assert!(render(vec![]) <= 182., "Palette grew after manual shrink"); }
    }
}
