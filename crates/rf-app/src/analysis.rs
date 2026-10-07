//! RF displays derived from acquired buffers. No synthetic display-only samples.
use crate::theme::*;
use eframe::egui::{self, Align2, Color32, Pos2, Rect, Sense, Stroke};
use rf_core::{NetworkTrace, Trace, Waveform};
use std::collections::VecDeque;

#[derive(Clone, Default)]
pub struct Waterfall {
    source_grid: Vec<f64>,
    rows: VecDeque<Vec<f64>>,
    texture: Option<egui::TextureHandle>,
    dirty: bool,
}
impl Waterfall {
    pub fn push(&mut self, t: &Trace) {
        if t.validate().is_err() {
            return;
        }
        if self.source_grid != t.frequency_hz {
            self.rows.clear();
            self.source_grid = t.frequency_hz.clone();
        }
        let n = t.amplitude_dbm.len().min(512);
        let mut row = Vec::with_capacity(n);
        for i in 0..n {
            let start = i * t.amplitude_dbm.len() / n;
            let end = (i + 1) * t.amplitude_dbm.len() / n;
            row.push(
                t.amplitude_dbm[start..end]
                    .iter()
                    .copied()
                    .fold(f64::NEG_INFINITY, f64::max),
            );
        }
        self.rows.push_front(row);
        self.rows.truncate(128);
        self.dirty = true;
    }
    pub fn show(&mut self, ui: &mut egui::Ui) {
        if self.rows.is_empty() {
            ui.label(crate::i18n::t(
                "Exécuter le banc en continu pour accumuler les acquisitions.",
            ));
            return;
        }
        let n = self.rows[0].len();
        if self.dirty {
            let pixels = self
                .rows
                .iter()
                .flat_map(|r| r.iter().map(|v| heat_color(*v)))
                .collect();
            let image = egui::ColorImage::new([n, self.rows.len()], pixels);
            if let Some(texture) = &mut self.texture {
                texture.set(image, egui::TextureOptions::NEAREST);
            } else {
                self.texture = Some(ui.ctx().load_texture(
                    "acquisition-waterfall",
                    image,
                    egui::TextureOptions::NEAREST,
                ));
            }
            self.dirty = false;
        }
        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(
                ui.available_width(),
                ui.available_height().clamp(160., 600.),
            ),
            Sense::hover(),
        );
        let area = rect.shrink2(egui::vec2(25., 35.));
        let p = ui.painter_at(rect);
        p.rect_filled(rect, 6., bg());
        if let Some(texture) = &self.texture {
            p.image(
                texture.id(),
                area,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1., 1.)),
                Color32::WHITE,
            );
        }
        p.text(
            rect.left_top() + egui::vec2(25., 10.),
            Align2::LEFT_TOP,
            format!(
                "{} acquisitions · récentes en haut · −110…10 dBm",
                self.rows.len()
            ),
            crate::theme::font(11.),
            text_color(),
        );
        p.text(
            rect.left_bottom() + egui::vec2(25., -8.),
            Align2::LEFT_BOTTOM,
            format!("{:.3} GHz", self.source_grid[0] / 1e9),
            crate::theme::font(11.),
            muted(),
        );
        p.text(
            rect.right_bottom() - egui::vec2(25., 8.),
            Align2::RIGHT_BOTTOM,
            format!("{:.3} GHz", self.source_grid.last().unwrap() / 1e9),
            crate::theme::font(11.),
            muted(),
        );
        if let Some(mouse) = response.hover_pos().filter(|p| area.contains(*p)) {
            let col = (((mouse.x - area.left()) / area.width()) * n as f32) as usize;
            let row = (((mouse.y - area.top()) / area.height()) * self.rows.len() as f32) as usize;
            response.on_hover_text(format!(
                "Acquisition −{} · {:.3} dBm (maximum du bin)",
                row,
                self.rows[row.min(self.rows.len() - 1)][col.min(n - 1)]
            ));
        }
    }
}
fn heat_color(dbm: f64) -> Color32 {
    let t = ((dbm + 110.) / 120.).clamp(0., 1.) as f32;
    let stops = [
        [15., 25., 55.],
        [72., 50., 139.],
        [15., 170., 160.],
        [235., 200., 55.],
        [255., 245., 225.],
    ];
    let v = t * 4.;
    let i = (v.floor() as usize).min(3);
    let f = v - i as f32;
    let c =
        std::array::from_fn::<_, 3, _>(|j| (stops[i][j] * (1. - f) + stops[i + 1][j] * f) as u8);
    Color32::from_rgb(c[0], c[1], c[2])
}
pub fn reflection(t: &NetworkTrace) -> Result<Vec<[f64; 2]>, String> {
    if !matches!(t.parameter.as_str(), "S11" | "S22") {
        return Err(
            "Le Smith utilise S11 ou S22. Choisir ce paramètre dans le bloc PNA puis Exécuter."
                .into(),
        );
    }
    if t.magnitude_db.len() != t.phase_deg.len()
        || t.magnitude_db
            .iter()
            .chain(&t.phase_deg)
            .any(|v| !v.is_finite())
    {
        return Err("Paramètres S invalides".into());
    }
    Ok(t.magnitude_db
        .iter()
        .zip(&t.phase_deg)
        .map(|(db, angle)| {
            let radius = 10_f64.powf(db / 20.);
            let phase = angle.to_radians();
            [radius * phase.cos(), radius * phase.sin()]
        })
        .collect())
}
pub fn impedance(gamma: [f64; 2], z0: f64) -> Option<[f64; 2]> {
    let [r, x] = gamma;
    let denom = (1. - r) * (1. - r) + x * x;
    if denom < 1e-20 || !z0.is_finite() || z0 <= 0. {
        None
    } else {
        Some([z0 * (1. - r * r - x * x) / denom, z0 * 2. * x / denom])
    }
}
pub fn smith(ui: &mut egui::Ui, t: &NetworkTrace, z0: f64) {
    let points = match reflection(t) {
        Ok(p) => p,
        Err(e) => {
            ui.label(e);
            return;
        }
    };
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(
            ui.available_width(),
            ui.available_height().clamp(180., 600.),
        ),
        Sense::hover(),
    );
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 6., bg());
    let center = rect.center();
    let radius = (rect.width().min(rect.height()) * 0.43).max(1.);
    let point = |z: [f64; 2]| center + egui::vec2(z[0] as f32, -z[1] as f32) * radius;
    p.circle_stroke(center, radius, Stroke::new(1.5, muted()));
    p.line_segment(
        [
            center - egui::vec2(radius, 0.),
            center + egui::vec2(radius, 0.),
        ],
        Stroke::new(1., border()),
    );
    for resistance in [0., 0.2, 0.5, 1., 2., 5.] {
        let mut pts = Vec::new();
        for i in 0..=200 {
            let reactance = (i as f64 / 200. - 0.5) * 40.;
            let d = (resistance + 1.) * (resistance + 1.) + reactance * reactance;
            let gamma = [
                (resistance * resistance + reactance * reactance - 1.) / d,
                2. * reactance / d,
            ];
            pts.push(point(gamma));
        }
        p.add(egui::Shape::line(pts, Stroke::new(0.6, border())));
    }
    for reactance in [-5., -2., -1., -0.5, -0.2, 0.2, 0.5, 1., 2., 5.] {
        let pts = (0..=100)
            .map(|i| {
                let r = i as f64 * 0.2;
                let d = (r + 1.) * (r + 1.) + reactance * reactance;
                point([(r * r + reactance * reactance - 1.) / d, 2. * reactance / d])
            })
            .collect();
        p.add(egui::Shape::line(pts, Stroke::new(0.6, border())));
    }
    let trace: Vec<_> = points.iter().copied().map(point).collect();
    p.add(egui::Shape::line(trace, Stroke::new(2., teal())));
    p.text(
        rect.min + egui::vec2(12., 12.),
        Align2::LEFT_TOP,
        format!(
            "{} · Z₀ = {z0:.1} Ω · Gamma = 10^(dB/20) angle(phi)",
            t.parameter
        ),
        crate::theme::font(11.),
        text_color(),
    );
    if let Some(mouse) = response.hover_pos()
        && let Some((i, g)) = points.iter().enumerate().min_by(|(_, a), (_, b)| {
            point(**a)
                .distance(mouse)
                .total_cmp(&point(**b).distance(mouse))
        })
    {
        let z = impedance(*g, z0)
            .map(|z| format!("{:.3} + j{:.3} Ω", z[0], z[1]))
            .unwrap_or_else(|| "∞ Ω".into());
        response.on_hover_text(format!(
            "{:.6} GHz\nΓ = {:.4} + j{:.4}\nZ = {z}",
            t.frequency_hz[i] / 1e9,
            g[0],
            g[1]
        ));
    }
}
pub fn iq_points(
    i: &Waveform,
    q: &Waveform,
    step: usize,
    offset: usize,
) -> Result<Vec<[f64; 2]>, String> {
    if i.samples.len() != q.samples.len()
        || i.sample_rate_hz != q.sample_rate_hz
        || i.unit != q.unit
        || step == 0
        || offset >= i.samples.len()
    {
        return Err("I/Q : longueur, cadence et unité identiques requises ; offset valide.".into());
    }
    Ok(i.samples
        .iter()
        .zip(&q.samples)
        .skip(offset)
        .step_by(step)
        .map(|(i, q)| [*i, *q])
        .collect())
}
pub fn constellation(ui: &mut egui::Ui, i: &Waveform, q: &Waveform, step: usize, offset: usize) {
    let points = match iq_points(i, q, step, offset) {
        Ok(p) => p,
        Err(e) => {
            ui.label(e);
            return;
        }
    };
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(
            ui.available_width(),
            ui.available_height().clamp(180., 600.),
        ),
        Sense::hover(),
    );
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 6., bg());
    let range = points
        .iter()
        .flatten()
        .map(|v| v.abs())
        .fold(0.1_f64, f64::max)
        * 1.15;
    let radius = rect.width().min(rect.height()) * 0.4;
    let center = rect.center();
    let point =
        |v: [f64; 2]| center + egui::vec2(v[0] as f32, -v[1] as f32) * (radius / range as f32);
    for v in -2..=2 {
        let f = v as f32 * radius / 2.;
        p.line_segment(
            [
                center + egui::vec2(f, -radius),
                center + egui::vec2(f, radius),
            ],
            Stroke::new(1., border()),
        );
        p.line_segment(
            [
                center + egui::vec2(-radius, f),
                center + egui::vec2(radius, f),
            ],
            Stroke::new(1., border()),
        );
    }
    let stride = points.len().div_ceil(2048).max(1);
    for v in points.iter().step_by(stride) {
        p.circle_filled(point(*v), 2., teal().gamma_multiply(0.8));
    }
    p.text(
        rect.min + egui::vec2(12., 10.),
        Align2::LEFT_TOP,
        format!("I / Q · {} · {} points · ±{range:.3}", i.unit, points.len()),
        crate::theme::font(11.),
        text_color(),
    );
    p.text(
        center + egui::vec2(radius, 12.),
        Align2::CENTER_TOP,
        "I",
        crate::theme::font(12.),
        text_color(),
    );
    p.text(
        center - egui::vec2(0., radius + 12.),
        Align2::CENTER_CENTER,
        "Q",
        crate::theme::font(12.),
        text_color(),
    );
    if let Some(mouse) = response.hover_pos() {
        let d = (mouse - center) / radius;
        response.on_hover_text(format!(
            "I = {:.4} {}\nQ = {:.4} {}",
            d.x as f64 * range,
            i.unit,
            -d.y as f64 * range,
            q.unit
        ));
    }
}
pub fn eye_segments(w: &Waveform, sps: usize, offset: usize) -> Result<Vec<Vec<f64>>, String> {
    if !(2..=256).contains(&sps) || offset >= sps || w.samples.len() < 2 * sps + offset {
        return Err("Eye : 2…256 échantillons/UI et au moins deux UI nécessaires.".into());
    }
    Ok((offset..=w.samples.len() - 2 * sps)
        .step_by(sps)
        .take(48)
        .map(|start| w.samples[start..start + 2 * sps].to_vec())
        .collect())
}
pub fn eye(ui: &mut egui::Ui, w: &Waveform, sps: usize, offset: usize) {
    let segments = match eye_segments(w, sps, offset) {
        Ok(s) => s,
        Err(e) => {
            ui.label(e);
            return;
        }
    };
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(
            ui.available_width(),
            ui.available_height().clamp(180., 600.),
        ),
        Sense::hover(),
    );
    let area = rect.shrink2(egui::vec2(35., 35.));
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 6., bg());
    let min = w.samples.iter().copied().fold(f64::INFINITY, f64::min);
    let max = w.samples.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let pad = ((max - min) * 0.1).max(0.01);
    for k in 0..=4 {
        let x = area.left() + area.width() * k as f32 / 4.;
        p.line_segment(
            [Pos2::new(x, area.top()), Pos2::new(x, area.bottom())],
            Stroke::new(1., border()),
        );
        p.text(
            Pos2::new(x, area.bottom() + 15.),
            Align2::CENTER_CENTER,
            format!("{:.1} UI", k as f64 / 2.),
            crate::theme::font(10.),
            muted(),
        );
    }
    for s in &segments {
        let pts = s
            .iter()
            .enumerate()
            .map(|(i, v)| {
                Pos2::new(
                    area.left() + i as f32 / (2 * sps) as f32 * area.width(),
                    area.bottom()
                        - ((v - min + pad) / (max - min + 2. * pad)) as f32 * area.height(),
                )
            })
            .collect();
        p.add(egui::Shape::line(
            pts,
            Stroke::new(1., teal().gamma_multiply(0.28)),
        ));
    }
    p.text(
        rect.min + egui::vec2(12., 10.),
        Align2::LEFT_TOP,
        format!(
            "{} · {:.3}…{:.3} · {} fenêtres · {:.3} MBd",
            w.unit,
            min,
            max,
            segments.len(),
            w.sample_rate_hz / sps as f64 / 1e6
        ),
        crate::theme::font(11.),
        text_color(),
    );
}
pub fn threshold(w: &Waveform, level: f64) -> Vec<f64> {
    w.samples
        .iter()
        .map(|v| if *v >= level { 1. } else { 0. })
        .collect()
}
pub fn timing(ui: &mut egui::Ui, w: &Waveform, level: f64) {
    let states = threshold(w, level);
    let mut x = Vec::new();
    let mut y = Vec::new();
    for (i, v) in states.iter().enumerate() {
        if i > 0 {
            x.push((i as f64 - 1e-6) / w.sample_rate_hz);
            y.push(states[i - 1]);
        }
        x.push(i as f64 / w.sample_rate_hz);
        y.push(*v);
    }
    crate::plot::series(
        ui,
        &x,
        &y,
        ui.available_height().clamp(180., 600.),
        "µs",
        "État 0/1",
        1e-6,
    );
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reflection_and_impedance_preserve_voltage_ratio_and_reject_transmission() {
        let mut t = NetworkTrace {
            frequency_hz: vec![1e9, 2e9],
            magnitude_db: vec![-20., -20.],
            phase_deg: vec![0., 90.],
            parameter: "S11".into(),
            simulated: true,
        };
        let g = reflection(&t).unwrap();
        assert!((g[0][0] - 0.1).abs() < 1e-12);
        assert!(g[1][0].abs() < 1e-12);
        assert!((impedance(g[0], 50.).unwrap()[0] - 61.1111111111).abs() < 1e-8);
        assert!(impedance([1., 0.], 50.).is_none());
        t.parameter = "S21".into();
        assert!(reflection(&t).is_err());
    }
    #[test]
    fn iq_eye_and_threshold_use_actual_samples() {
        let w = Waveform {
            samples: vec![-1., 0., 1., 0., -1., 0., 1., 0.],
            sample_rate_hz: 8.,
            tone_hz: 1.,
            unit: "V".into(),
            simulated: true,
        };
        let mut q = w.clone();
        assert_eq!(iq_points(&w, &q, 2, 1).unwrap(), vec![[0., 0.]; 4]);
        q.unit = "FS".into();
        assert!(iq_points(&w, &q, 1, 0).is_err());
        assert_eq!(eye_segments(&w, 2, 0).unwrap().len(), 3);
        assert!(eye_segments(&w, 0, 0).is_err());
        assert_eq!(threshold(&w, 0.5), vec![0., 0., 1., 0., 0., 0., 1., 0.]);
    }
    #[test]
    fn waterfall_resets_incompatible_frequency_grids_and_bounds_history() {
        let mut w = Waterfall::default();
        let mut t = Trace {
            frequency_hz: vec![1e9, 2e9],
            amplitude_dbm: vec![-80., -10.],
            simulated: true,
        };
        for _ in 0..150 {
            w.push(&t);
        }
        assert_eq!(w.rows.len(), 128);
        t.frequency_hz[1] = 3e9;
        w.push(&t);
        assert_eq!(w.rows.len(), 1);
        assert_eq!(w.rows[0], t.amplitude_dbm);
    }
}
