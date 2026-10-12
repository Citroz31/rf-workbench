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
