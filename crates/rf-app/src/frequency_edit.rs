use eframe::egui;
use rf_core::frequency::Unit;
pub fn frequency(ui: &mut egui::Ui, label: &str, hz: &mut f64, unit: &mut Unit) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label(label);
        let mut value = *hz / unit.factor();
        if ui
            .add(
                egui::DragValue::new(&mut value)
                    .speed(0.001)
                    .max_decimals(9),
            )
            .changed()
        {
            *hz = value * unit.factor();
            changed = true;
        }
        egui::ComboBox::from_id_salt(("frequency-unit", label))
            .width(65.)
            .selected_text(unit.label())
            .show_ui(ui, |ui| {
                for u in Unit::ALL {
                    ui.selectable_value(unit, u, u.label());
                }
            });
    });
    changed
}

pub fn sweep_step(ui: &mut egui::Ui, c: &mut rf_core::Config) {
    let mut step=(c.stop_hz-c.start_hz)/(c.points.saturating_sub(1).max(1) as f64);
    if frequency(ui,"Pas fréquentiel",&mut step,&mut c.step_unit)
        && let Some(points)=rf_core::frequency::points_for_step(c.start_hz,c.stop_hz,step) { c.points=points; }
    ui.small(format!("Pas effectif : {:.9} {} · {} points / max {}. Extrémités conservées.", (c.stop_hz-c.start_hz)/(c.points.saturating_sub(1).max(1) as f64)/c.step_unit.factor(),c.step_unit.label(),c.points,rf_core::MAX_POINTS));
}
