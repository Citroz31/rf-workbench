//! PNA-style shared grids: every trace uses its own vertical transform.
use crate::{
    results::{Format, Panel, Trace, Window},
    theme::*,
};
use eframe::egui::{self, Align2, Pos2, Rect, Sense, Stroke, StrokeKind};
use rf_core::network::Curve;

pub fn color(index: usize) -> egui::Color32 {
    [
        gold(),
        blue(),
        teal(),
        red(),
        purple(),
        egui::Color32::from_rgb(239, 143, 72),
    ][index % 6]
}
fn curve<'a>(t: &Trace, data: &'a [Curve]) -> Option<&'a Curve> {
    data.iter()
        .rev()
        .find(|c| c.node == t.node && c.name == t.parameter && c.corrected == t.corrected)
}
fn values<'a>(t: &Trace, c: &'a Curve) -> Option<(&'static str, &'a [f64])> {
    match t.format {
        Format::Phase => c.phase_deg.as_deref().map(|v| ("°", v)),
        Format::Smith => None,
        _ => Some(("", &c.y)),
    }
}
pub fn scale_controls(ui: &mut egui::Ui, w: &mut Window, data: &[Curve]) {
    w.ensure_traces();
    let t = &mut w.traces[w.active_trace];
    ui.horizontal_wrapped(|ui| {
        ui.colored_label(
            color(t.color),
            format!(
                "Graphique {} · {}{}",
                w.id,
                t.parameter,
                if t.corrected { " corrigé" } else { "" }
            ),
        );
        egui::ComboBox::from_id_salt(("trace-format", w.id))
            .selected_text(format!("{:?}", t.format))
            .show_ui(ui, |ui| {
                for f in [
                    Format::Magnitude,
                    Format::Phase,
                    Format::Smith,
                    Format::Linear,
                ] {
                    ui.selectable_value(&mut t.format, f, format!("{f:?}"));
                }
            });
        if t.format == Format::Smith {
            return;
        }
        let c = curve(t, data);
        let y = c.and_then(|c| values(t, c).map(|(_, y)| y));
        let bounds = t.scale.bounds(y);
        let mut per_division = if t.scale.automatic {
            (bounds.high - bounds.low) / 10.
        } else {
            t.scale.per_division
        };
        let mut reference = if t.scale.automatic {
            (bounds.low + bounds.high) / 2.
        } else {
            t.scale.reference
        };
        let mut position = if t.scale.automatic {
            5.
        } else {
            t.scale.reference_division
        };
        let units = if t.format == Format::Phase {
            "°"
        } else {
            c.map_or("dB", |c| c.y_unit.as_str())
        };
        ui.label("Échelle/div");
        let changed = ui
            .add(
                egui::DragValue::new(&mut per_division)
                    .speed(0.1)
                    .range(1e-9..=1e12)
                    .suffix(format!(" {units}")),
            )
            .changed();
        ui.label("Référence");
        let changed = ui
            .add(
                egui::DragValue::new(&mut reference)
                    .speed(0.1)
                    .range(-1e15..=1e15),
            )
            .changed()
            || changed;
        ui.label("Position/div");
        let changed = ui
            .add(
                egui::DragValue::new(&mut position)
                    .speed(0.1)
                    .range(0. ..=10.),
            )
            .changed()
            || changed;
        if changed {
            t.scale.automatic = false;
            t.scale.per_division = per_division;
            t.scale.reference = reference;
            t.scale.reference_division = position;
        }
        if ui
            .selectable_label(t.scale.automatic, "Auto · cette trace")
            .clicked()
        {
            t.scale.automatic = true;
        }
    });
}
pub fn graph(ui: &mut egui::Ui, w: &mut Window, data: &[Curve], height: f32) {
    w.ensure_traces();
    ui.horizontal_wrapped(|ui| {
        ui.strong(format!("{}", w.id));
        for (i, t) in w.traces.iter().enumerate() {
            let label = format!(
                "{}{}{}",
                t.parameter,
                if t.corrected { "*" } else { "" },
                if t.visible { "" } else { " (masquée)" }
            );
            if ui
                .selectable_label(
                    w.active_trace == i,
                    egui::RichText::new(label).color(color(t.color)),
                )
                .clicked()
            {
                w.active_trace = i;
            }
        }
    });
    let active = &w.traces[w.active_trace];
    let ac = curve(active, data);
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), (height - 35.).max(70.)),
        Sense::click(),
    );
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 4., bg());
    if active.format == Format::Smith {
        smith(&p, rect, w, data);
        return;
    }
    let area = Rect::from_min_max(
        rect.min + egui::vec2(63., 20.),
        rect.max - egui::vec2(16., 28.),
    );
    if area.width() < 20. || area.height() < 20. {
        return;
    }
    let axis_curve = ac.or_else(|| w.traces.iter().filter_map(|t| curve(t, data)).next());
    let ay = ac.and_then(|c| values(active, c).map(|(_, y)| y));
    let bounds = active.scale.bounds(ay);
    let unit = if active.format == Format::Phase {
        "°"
    } else {
        ac.map_or("dB", |c| c.y_unit.as_str())
    };
    let (first, last, x_unit, x_scale) =
        axis_curve
            .filter(|c| c.x.len() >= 2)
            .map_or((0., 1., "Hz", 1.), |c| {
                (
                    c.x[0],
                    c.x[c.x.len() - 1],
                    c.x_unit.as_str(),
                    if c.x_unit == "Hz" { 1e9 } else { 1. },
                )
            });
    for i in 0..=10 {
        let fraction = i as f32 / 10.;
        let y = area.top() + area.height() * fraction;
        p.line_segment(
            [Pos2::new(area.left(), y), Pos2::new(area.right(), y)],
            Stroke::new(0.6, border()),
        );
        if i % 2 == 0 {
            p.text(
                Pos2::new(area.left() - 5., y),
                Align2::RIGHT_CENTER,
                format!(
                    "{:.2}",
                    bounds.high - (bounds.high - bounds.low) * fraction as f64
                ),
                crate::theme::font(10.),
                color(active.color),
            );
        }
        let x = area.left() + area.width() * fraction;
        p.line_segment(
            [Pos2::new(x, area.top()), Pos2::new(x, area.bottom())],
            Stroke::new(0.6, border()),
        );
        if i % 2 == 0 {
            p.text(
                Pos2::new(x, area.bottom() + 13.),
                Align2::CENTER_CENTER,
                format!(
                    "{:.3}",
                    (first + (last - first) * fraction as f64) / x_scale
                ),
                crate::theme::font(9.),
                muted(),
            );
        }
    }
    p.text(
        area.left_top() + egui::vec2(6., 4.),
        Align2::LEFT_TOP,
        format!(
            "{} · {unit}{}",
            active.parameter,
            if ac.is_some_and(|c| c.simulated) {
                " · SIM"
            } else {
                ""
            }
        ),
        crate::theme::font(10.),
        color(active.color),
    );
    p.text(
        rect.right_bottom(),
        Align2::RIGHT_BOTTOM,
        if x_unit == "Hz" { "GHz" } else { x_unit },
        crate::theme::font(10.),
        muted(),
    );
    let cp = p.with_clip_rect(area);
    let mut unavailable = 0;
    for t in w.traces.iter().filter(|t| t.visible) {
        let Some(c) = curve(t, data) else {
            unavailable += 1;
            continue;
        };
        let Some((_, y)) = values(t, c) else {
            unavailable += 1;
            continue;
        };
        if c.x_unit != x_unit
            || c.x.len() != y.len()
            || y.len() < 2
            || y.iter().chain(&c.x).any(|v| !v.is_finite())
            || last <= first
        {
            unavailable += 1;
            continue;
        }
        // The active trace supplies labels only. Each trace maps through its own bounds.
        let own = t.scale.bounds(Some(y));
        let point = |i: usize| {
            Pos2::new(
                area.left() + ((c.x[i] - first) / (last - first)) as f32 * area.width(),
                area.bottom() - own.normalized(y[i]) as f32 * area.height(),
            )
        };
        let stride = y.len().div_ceil((area.width() as usize * 2).max(100));
        let mut points = Vec::new();
        for start in (0..y.len()).step_by(stride) {
            let end = (start + stride).min(y.len());
            let lo = (start..end).min_by(|a, b| y[*a].total_cmp(&y[*b])).unwrap();
            let hi = (start..end).max_by(|a, b| y[*a].total_cmp(&y[*b])).unwrap();
            points.push(point(lo.min(hi)));
            points.push(point(lo.max(hi)));
        }
        cp.add(egui::Shape::line(
            points,
            Stroke::new(
                if std::ptr::eq(t, active) { 2.2 } else { 1.6 },
                color(t.color),
            ),
        ));
    }
    if unavailable > 0 || ac.is_none() {
        p.text(
            area.center_bottom() - egui::vec2(0., 5.),
            Align2::CENTER_BOTTOM,
            "Traces absentes / formats ou axes incompatibles",
            crate::theme::font(10.),
            muted(),
        );
    }
    if let Some(mouse) = response.hover_pos().filter(|p| area.contains(*p)) {
        let target = first + (last - first) * ((mouse.x - area.left()) / area.width()) as f64;
        let mut hint = String::new();
        for t in w.traces.iter().filter(|t| t.visible) {
            if let Some(c) = curve(t, data).filter(|c| c.x_unit == x_unit)
                && let Some((_, y)) = values(t, c)
                && let Some((i, _)) =
                    c.x.iter().enumerate().min_by(|(_, a), (_, b)| {
                        (*a - target).abs().total_cmp(&(*b - target).abs())
                    })
                && let Some(v) = y.get(i)
            {
                let unit = if t.format == Format::Phase {
                    "°"
                } else {
                    &c.y_unit
                };
                hint.push_str(&format!(
                    "{} : {:.4} {unit} à {:.6} {}\n",
                    t.parameter,
                    v,
                    c.x[i] / x_scale,
                    if x_unit == "Hz" { "GHz" } else { x_unit }
                ));
            }
        }
        response.on_hover_text(hint);
    }
}
fn smith(p: &egui::Painter, rect: Rect, w: &Window, data: &[Curve]) {
    let center = rect.center();
    let radius = (rect.width().min(rect.height()) * 0.44).max(5.);
    let circle = Rect::from_center_size(center, egui::vec2(radius * 2., radius * 2.));
    p.circle_stroke(center, radius, Stroke::new(1., muted()));
    p.line_segment(
        [
            center - egui::vec2(radius, 0.),
            center + egui::vec2(radius, 0.),
        ],
        Stroke::new(0.8, border()),
    );
    let grid = p.with_clip_rect(circle);
    for r in [0.2, 0.5, 1., 2., 5.] {
        grid.circle_stroke(
            center + egui::vec2(radius * r / (1. + r), 0.),
            radius / (1. + r),
            Stroke::new(0.6, border()),
        );
    }
    for x in [0.2, 0.5, 1., 2., 5.] {
        for sign in [-1., 1.] {
            let points = (0..=240)
                .filter_map(|i| {
                    let r = i as f32 / 8.;
                    let reactance = x * sign;
                    let d = (r + 1.) * (r + 1.) + reactance * reactance;
                    let re = (r * r + reactance * reactance - 1.) / d;
                    let im = 2. * reactance / d;
                    ((re * re + im * im) <= 1.0001)
                        .then_some(center + egui::vec2(re * radius, -im * radius))
                })
                .collect();
            grid.add(egui::Shape::line(points, Stroke::new(0.6, border())));
        }
    }
    for t in w
        .traces
        .iter()
        .filter(|t| t.visible && t.format == Format::Smith)
    {
        if let Some(c) = curve(t, data).filter(|c| {
            c.x_unit == "Hz" && matches!(c.name.as_str(), "S11" | "S22" | "S33" | "S44")
        }) && let Some(phase) = &c.phase_deg
        {
            let points =
                c.y.iter()
                    .zip(phase)
                    .map(|(db, degrees)| {
                        let magnitude = 10f64.powf(db / 20.);
                        let angle = degrees.to_radians();
                        center
                            + egui::vec2(
                                (magnitude * angle.cos()) as f32 * radius,
                                -(magnitude * angle.sin()) as f32 * radius,
                            )
                    })
                    .collect();
            grid.add(egui::Shape::line(points, Stroke::new(1.8, color(t.color))));
        }
    }
    p.text(
        rect.left_top() + egui::vec2(8., 3.),
        Align2::LEFT_TOP,
        "Smith · Γ · réflexion complexe",
        crate::theme::font(10.),
        muted(),
    );
}
pub struct Output {
    pub configure: Option<u64>,
    pub closed: Vec<u64>,
}
pub fn show(
    ctx: &egui::Context,
    panel: &mut Panel,
    windows: &mut [Window],
    data: &[Curve],
    revision: u64,
) -> Output {
    let mut configure = None;
    let mut closed = Vec::new();
    if windows.is_empty() || !panel.open {
        return Output { configure, closed };
    }
    for w in windows.iter_mut() {
        w.ensure_traces();
    }
    if panel.floating {
        for w in windows.iter_mut() {
            let mut open = true;
            egui::Window::new(format!("VNA · graphique {}", w.id))
                .id(egui::Id::new(("vna-floating", w.id, revision)))
                .open(&mut open)
                .default_pos([w.rect[0], w.rect[1]])
                .default_size([w.rect[2], w.rect[3]])
                .resizable(true)
                .show(ctx, |ui| {
                    scale_controls(ui, w, data);
                    if ui.button("Configurer les traces…").clicked() {
                        configure = Some(w.id);
                    }
                    graph(ui, w, data, (ui.available_height() - 5.).max(150.));
                });
            if !open {
                closed.push(w.id);
            }
        }
        return Output { configure, closed };
    }
    let mut open = panel.open;
    let shown = egui::Window::new("VNA · panneau de résultats")
        .id(egui::Id::new(("vna-panel", revision)))
        .open(&mut open)
        .resizable(true)
        .default_pos([panel.rect[0], panel.rect[1]])
        .default_size([panel.rect[2], panel.rect[3]])
        .min_size([650., 430.])
        .show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label("Disposition");
                egui::ComboBox::from_id_salt("vna-layout")
                    .selected_text(format!("{} × {}", panel.columns, panel.rows))
                    .show_ui(ui, |ui| {
                        for (label, cols, rows) in [
                            ("1 graphique", 1, 1),
                            ("2 côte à côte", 2, 1),
                            ("2 superposés", 1, 2),
                            ("4 graphiques", 2, 2),
                            ("6 graphiques", 3, 2),
                            ("8 graphiques", 4, 2),
                        ] {
                            if ui
                                .selectable_label(
                                    panel.columns == cols && panel.rows == rows,
                                    label,
                                )
                                .clicked()
                            {
                                panel.columns = cols;
                                panel.rows = rows;
                                panel.page = 0;
                            }
                        }
                    });
                let pages = windows.len().div_ceil(panel.columns * panel.rows);
                panel.page = panel.page.min(pages - 1);
                if ui
                    .add_enabled(panel.page > 0, egui::Button::new("◀"))
                    .clicked()
                {
                    panel.page -= 1;
                }
                ui.label(format!("Page {}/{}", panel.page + 1, pages));
                if ui
                    .add_enabled(panel.page + 1 < pages, egui::Button::new("▶"))
                    .clicked()
                {
                    panel.page += 1;
                }
                if ui.button("Ajouter / configurer les traces…").clicked() {
                    configure = Some(panel.selected);
                }
            });
            let selected = windows
                .iter()
                .position(|w| w.id == panel.selected)
                .unwrap_or(0);
            panel.selected = windows[selected].id;
            scale_controls(ui, &mut windows[selected], data);
            ui.separator();
            let count = panel.columns * panel.rows;
            let (area, _) = ui.allocate_exact_size(
                egui::vec2(ui.available_width(), ui.available_height().max(160.)),
                Sense::hover(),
            );
            let width = (area.width() - 8. * (panel.columns - 1) as f32) / panel.columns as f32;
            let height = (area.height() - 8. * (panel.rows - 1) as f32) / panel.rows as f32;
            for slot in 0..count {
                let Some(w) = windows.get_mut(panel.page * count + slot) else {
                    continue;
                };
                let rect = Rect::from_min_size(
                    area.min
                        + egui::vec2(
                            (slot % panel.columns) as f32 * (width + 8.),
                            (slot / panel.columns) as f32 * (height + 8.),
                        ),
                    egui::vec2(width, height),
                );
                ui.painter().rect_stroke(
                    rect,
                    3.,
                    Stroke::new(
                        1.,
                        if panel.selected == w.id {
                            color(w.traces[w.active_trace].color)
                        } else {
                            border()
                        },
                    ),
                    StrokeKind::Inside,
                );
                let mut clicked = false;
                ui.scope_builder(
                    egui::UiBuilder::new()
                        .max_rect(rect.shrink(5.))
                        .id_salt(("vna-graph", w.id)),
                    |ui| {
                        let before = w.active_trace;
                        graph(ui, w, data, height - 10.);
                        clicked = w.active_trace != before
                            || ui.input(|i| {
                                i.pointer.primary_clicked()
                                    && i.pointer.hover_pos().is_some_and(|p| rect.contains(p))
                            });
                    },
                );
                if clicked {
                    panel.selected = w.id;
                }
            }
        });
    panel.open = open;
    if let Some(shown) = shown {
        let r = shown.response.rect;
        panel.rect = [r.min.x, r.min.y, r.width(), r.height()];
    }
    Output { configure, closed }
}
