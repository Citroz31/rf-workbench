use crate::theme::*;
use eframe::egui::{self, Align2, Pos2, Rect, Sense, Stroke};
use rf_core::Trace;

pub fn series(
    ui: &mut egui::Ui,
    x: &[f64],
    y: &[f64],
    height: f32,
    x_unit: &str,
    y_unit: &str,
    x_scale: f64,
) {
    let id = ui.next_auto_id();
    egui::Resize::default()
        .id_salt(id)
        .default_width(ui.available_width())
        .default_height(height)
        .min_height(100.)
        .max_height(900.)
        .resizable([false, true])
        .with_stroke(true)
        .show(ui, |ui| {
            series_body(
                ui,
                x,
                y,
                ui.available_height().clamp(100., 900.),
                x_unit,
                y_unit,
                x_scale,
            );
        });
}
fn series_body(
    ui: &mut egui::Ui,
    x: &[f64],
    y: &[f64],
    height: f32,
    x_unit: &str,
    y_unit: &str,
    x_scale: f64,
) {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), height), Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 8., bg());
    let area = Rect::from_min_max(
        rect.min + egui::vec2(65., 22.),
        rect.max - egui::vec2(20., 35.),
    );
    if x.len() < 2
        || x.len() != y.len()
        || x.iter().chain(y).any(|v| !v.is_finite())
        || x.windows(2).any(|v| v[1] <= v[0])
        || area.width() < 1.
        || area.height() < 1.
    {
        return;
    }
    let min = y.iter().copied().fold(f64::INFINITY, f64::min);
    let max = y.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let pad = ((max - min) * 0.15).max(0.1);
    let low = min - pad;
    let high = max + pad;
    let first = x[0];
    let last = x[x.len() - 1];
    let point = |x: f64, y: f64| {
        Pos2::new(
            area.left() + ((x - first) / (last - first)) as f32 * area.width(),
            area.bottom() - ((y - low) / (high - low)) as f32 * area.height(),
        )
    };
    for i in 0..=5 {
        let f = i as f64 / 5.;
        let px = area.left() + area.width() * f as f32;
        let py = area.top() + area.height() * f as f32;
        p.line_segment(
            [Pos2::new(px, area.top()), Pos2::new(px, area.bottom())],
            Stroke::new(1., border()),
        );
        p.line_segment(
            [Pos2::new(area.left(), py), Pos2::new(area.right(), py)],
            Stroke::new(1., border()),
        );
        p.text(
            Pos2::new(px, area.bottom() + 14.),
            Align2::CENTER_CENTER,
            format!("{:.3}", (first + (last - first) * f) / x_scale),
            crate::theme::font(10.),
            muted(),
        );
        p.text(
            Pos2::new(area.left() - 7., py),
            Align2::RIGHT_CENTER,
            format!("{:.2}", high - (high - low) * f),
            crate::theme::font(10.),
            muted(),
        );
    }
    let stride = y.len().div_ceil((area.width() as usize * 2).max(200));
    let mut points = Vec::new();
    for start in (0..y.len()).step_by(stride) {
        let end = (start + stride).min(y.len());
        let lo = (start..end).min_by(|a, b| y[*a].total_cmp(&y[*b])).unwrap();
        let hi = (start..end).max_by(|a, b| y[*a].total_cmp(&y[*b])).unwrap();
        for i in [lo.min(hi), lo.max(hi)] {
            points.push(point(x[i], y[i]));
        }
    }
    p.add(egui::Shape::line(points, Stroke::new(1.7, blue())));
    p.text(
        area.left_top() + egui::vec2(8., 8.),
        Align2::LEFT_TOP,
        y_unit,
        crate::theme::font(11.),
        muted(),
    );
    p.text(
        rect.right_bottom() - egui::vec2(5., 5.),
        Align2::RIGHT_BOTTOM,
        x_unit,
        crate::theme::font(11.),
        muted(),
    );
    if let Some(cursor) = response.hover_pos()
        && area.contains(cursor)
    {
        let i = (((cursor.x - area.left()) / area.width()) * (x.len() - 1) as f32).round() as usize;
        response.on_hover_text(format!(
            "{:.6} {x_unit}\n{:.4} {y_unit}",
            x[i] / x_scale,
            y[i]
        ));
    }
}

pub fn plot(ui: &mut egui::Ui, trace: &Trace, height: f32, marker: bool) {
    let id = ui.next_auto_id();
    egui::Resize::default()
        .id_salt(id)
        .default_width(ui.available_width())
        .default_height(height)
        .min_height(100.)
        .max_height(900.)
        .resizable([false, true])
        .with_stroke(true)
        .show(ui, |ui| {
            plot_body(ui, trace, ui.available_height().clamp(100., 900.), marker);
        });
}
fn plot_body(ui: &mut egui::Ui, trace: &Trace, height: f32, marker: bool) {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), height), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 8., bg());
    let area = Rect::from_min_max(
        rect.min + egui::vec2(56., 22.),
        rect.max - egui::vec2(22., 35.),
    );
    if trace.validate().is_err() || area.width() < 1. || area.height() < 1. {
        return;
    }
    let start = trace.frequency_hz[0];
    let stop = *trace.frequency_hz.last().unwrap();
    let min = -110.;
    let max = 10.;
    let point = |f: f64, a: f64| {
        Pos2::new(
            area.left() + ((f - start) / (stop - start)) as f32 * area.width(),
            area.bottom() - ((a - min) / (max - min)) as f32 * area.height(),
        )
    };
    for i in 0..=6 {
        let x = area.left() + area.width() * i as f32 / 6.;
        painter.line_segment(
            [Pos2::new(x, area.top()), Pos2::new(x, area.bottom())],
            Stroke::new(1., border().gamma_multiply(0.5)),
        );
        painter.text(
            Pos2::new(x, area.bottom() + 14.),
            Align2::CENTER_CENTER,
            format!("{:.3}", (start + (stop - start) * i as f64 / 6.) / 1e9),
            crate::theme::font(10.),
            muted(),
        );
    }
    let vertical_ticks = ((area.height() / (24. * crate::theme::scale())) as usize).clamp(1, 6);
    for i in 0..=vertical_ticks {
        let fraction = i as f32 / vertical_ticks as f32;
        let y = area.top() + area.height() * fraction;
        painter.line_segment(
            [Pos2::new(area.left(), y), Pos2::new(area.right(), y)],
            Stroke::new(1., border().gamma_multiply(0.5)),
        );
        painter.text(
            Pos2::new(area.left() - 10., y),
            Align2::RIGHT_CENTER,
            format!("{:.0}", max - (max - min) * f64::from(fraction)),
            crate::theme::font(10.),
            muted(),
        );
    }
    // Envelope decimation retains narrow peaks; rendering work is bounded by pixels.
    let budget = (area.width().max(1.) as usize * 2).max(200);
    let stride = trace.amplitude_dbm.len().div_ceil(budget).max(1);
    let mut pts = Vec::with_capacity(budget * 2);
    for begin in (0..trace.amplitude_dbm.len()).step_by(stride) {
        let end = (begin + stride).min(trace.amplitude_dbm.len());
        let slice = &trace.amplitude_dbm[begin..end];
        let low = slice
            .iter()
            .enumerate()
            .min_by(|a, b| a.1.total_cmp(b.1))
            .unwrap()
            .0
            + begin;
        let high = slice
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .unwrap()
            .0
            + begin;
        for index in [low.min(high), low.max(high)] {
            pts.push(point(
                trace.frequency_hz[index],
                trace.amplitude_dbm[index].clamp(min, max),
            ));
        }
    }
    painter.add(egui::Shape::line(pts, Stroke::new(1.7, teal())));
    painter.text(
        area.left_top() + egui::vec2(8., 8.),
        Align2::LEFT_TOP,
        "dBm",
        crate::theme::font(11.),
        muted(),
    );
    painter.text(
        rect.right_bottom() - egui::vec2(7., 5.),
        Align2::RIGHT_BOTTOM,
        "GHz",
        crate::theme::font(11.),
        muted(),
    );
    if marker && let Ok((f, a)) = trace.peak() {
        let p = point(f, a.clamp(min, max));
        painter.circle_filled(p, 4., gold());
        painter.text(
            p + egui::vec2(9., -7.),
            Align2::LEFT_BOTTOM,
            format!("M1  {a:.2} dBm"),
            crate::theme::font(11.),
            gold(),
        );
    }
    if let Some(cursor) = response.hover_pos()
        && area.contains(cursor)
    {
        let fraction = ((cursor.x - area.left()) / area.width()).clamp(0., 1.);
        let i = (fraction * (trace.frequency_hz.len() - 1) as f32).round() as usize;
        painter.line_segment(
            [
                Pos2::new(cursor.x, area.top()),
                Pos2::new(cursor.x, area.bottom()),
            ],
            Stroke::new(1., muted()),
        );
        response.on_hover_text(format!(
            "{:.6} GHz\n{:.3} dBm",
            trace.frequency_hz[i] / 1e9,
            trace.amplitude_dbm[i]
        ));
    }
}
