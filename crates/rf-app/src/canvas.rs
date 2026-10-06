use crate::theme::*;
use eframe::egui::{self, Align2, FontId, Id, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};
use rf_core::{Graph, Kind};

pub const NODE_SIZE: Vec2 = egui::vec2(220., 144.);
#[derive(Default)]
pub struct History {
    undo: Vec<Graph>,
    redo: Vec<Graph>,
}
impl History {
    pub fn record(&mut self, before: Graph) {
        self.undo.push(before);
        if self.undo.len() > 100 {
            self.undo.remove(0);
        }
        self.redo.clear();
    }
    pub fn undo(&mut self, g: &mut Graph) {
        if let Some(old) = self.undo.pop() {
            self.redo.push(g.clone());
            *g = old;
        }
    }
    pub fn redo(&mut self, g: &mut Graph) {
        if let Some(new) = self.redo.pop() {
            self.undo.push(g.clone());
            *g = new;
        }
    }
    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }
}
pub struct Canvas {
    pub zoom: f32,
    pub pan: Vec2,
    pub selected: Option<u64>,
    pub wiring: Option<u64>,
    drag_before: Option<Graph>,
    pub completed: Vec<u64>,
    fit: bool,
}
impl Default for Canvas {
    fn default() -> Self {
        Self {
            zoom: 1.,
            pan: egui::vec2(12., -24.),
            selected: Some(3),
            wiring: None,
            drag_before: None,
            completed: Vec::new(),
            fit: true,
        }
    }
}
impl Canvas {
    pub fn fit(&mut self) {
        self.fit = true;
    }
    fn screen(&self, origin: Pos2, p: [f32; 2]) -> Pos2 {
        origin + self.pan + egui::vec2(p[0], p[1]) * self.zoom
    }
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        g: &mut Graph,
        history: &mut History,
        height: f32,
    ) -> Option<String> {
        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), height),
            Sense::click_and_drag(),
        );
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 8., BG);
        if self.fit && !g.nodes.is_empty() {
            let min_x = g
                .nodes
                .iter()
                .map(|n| n.position[0])
                .fold(f32::INFINITY, f32::min)
                - 30.;
            let min_y = g
                .nodes
                .iter()
                .map(|n| n.position[1])
                .fold(f32::INFINITY, f32::min)
                - 35.;
            let max_x = g
                .nodes
                .iter()
                .map(|n| n.position[0] + NODE_SIZE.x)
                .fold(f32::NEG_INFINITY, f32::max)
                + 30.;
            let max_y = g
                .nodes
                .iter()
                .map(|n| n.position[1] + NODE_SIZE.y)
                .fold(f32::NEG_INFINITY, f32::max)
                + 35.;
            self.zoom = (rect.width() / (max_x - min_x))
                .min(rect.height() / (max_y - min_y))
                .clamp(0.25, 1.3);
            self.pan = egui::vec2(
                (rect.width() - (max_x - min_x) * self.zoom) / 2. - min_x * self.zoom,
                (rect.height() - (max_y - min_y) * self.zoom) / 2. - min_y * self.zoom,
            );
            self.fit = false;
        }
        let spacing = 24. * self.zoom;
        if spacing >= 6. {
            let offset = egui::vec2(
                self.pan.x.rem_euclid(spacing),
                self.pan.y.rem_euclid(spacing),
            );
            for x in 0..=(rect.width() / spacing) as usize {
                for y in 0..=(rect.height() / spacing) as usize {
                    painter.circle_filled(
                        rect.min + offset + egui::vec2(x as f32 * spacing, y as f32 * spacing),
                        1.,
                        BORDER.gamma_multiply(0.75),
                    );
                }
            }
        }
        if response.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.
                && let Some(mouse) = ui.input(|i| i.pointer.hover_pos())
            {
                let old = self.zoom;
                self.zoom = (old * (scroll * 0.002).exp()).clamp(0.25, 2.);
                self.pan = (mouse - rect.min) - (mouse - rect.min - self.pan) * (self.zoom / old);
            }
        }
        let mut error = None;
        let mut disconnect = None;
        for (i, e) in g.edges.iter().enumerate() {
            if let (Some(a), Some(b)) = (g.node(e.from), g.node(e.to)) {
                let (out, out_normal) = port_offset(g, a, true);
                let (input, in_normal) = port_offset(g, b, false);
                let from = self.screen(rect.min, a.position) + out * self.zoom;
                let to = self.screen(rect.min, b.position) + input * self.zoom;
                let color = port(a.kind.output());
                wire(
                    &painter,
                    from,
                    to,
                    [out_normal, in_normal],
                    color,
                    self.zoom,
                );
                let midpoint = from + (to - from) * 0.5;
                let hit = ui.interact(
                    Rect::from_center_size(midpoint, egui::vec2(30., 20.)),
                    Id::new(("wire", i)),
                    Sense::click(),
                );
                hit.on_hover_text("Clic droit : retirer ce câble")
                    .context_menu(|ui| {
                        if ui.button("Retirer le câble").clicked() {
                            disconnect = Some(i);
                            ui.close();
                        }
                    });
            }
        }
        if let Some(i) = disconnect {
            history.record(g.clone());
            g.edges.remove(i);
        }
        if let Some(id) = self.wiring
            && let Some(n) = g.node(id)
            && let Some(mouse) = ui.input(|i| i.pointer.hover_pos())
        {
            let (offset, normal) = port_offset(g, n, true);
            let from = self.screen(rect.min, n.position) + offset * self.zoom;
            wire(
                &painter,
                from,
                mouse,
                [normal, -normal],
                port(n.kind.output()),
                self.zoom,
            );
        }
        let mut clicked_port = None;
        let mut clicked_output = None;
        let mut drag_delta = None;
        let mut any_node_hovered = false;
        let mut remove = None;
        for node in &g.nodes {
            let top = self.screen(rect.min, node.position);
            let card = Rect::from_min_size(top, NODE_SIZE * self.zoom);
            if !rect.intersects(card) {
                continue;
            }
            let color = kind(node.kind);
            let selected = self.selected == Some(node.id);
            painter.rect_filled(card, 8., CARD);
            painter.rect_stroke(
                card,
                8.,
                Stroke::new(
                    if selected { 1.8 } else { 1. },
                    if selected { color } else { BORDER },
                ),
                StrokeKind::Inside,
            );
            painter.rect_filled(
                Rect::from_min_size(top, egui::vec2(4., NODE_SIZE.y) * self.zoom),
                2.,
                color,
            );
            let title = if node.title.chars().count() > 26 {
                format!("{}…", node.title.chars().take(25).collect::<String>())
            } else {
                node.title.clone()
            };
            painter.text(
                top + egui::vec2(17., 17.) * self.zoom,
                Align2::LEFT_TOP,
                node.kind.tag(),
                FontId::proportional(10. * self.zoom),
                color,
            );
            painter.text(
                top + egui::vec2(17., 38.) * self.zoom,
                Align2::LEFT_TOP,
                title,
                FontId::proportional(14. * self.zoom),
                TEXT,
            );
            let summary = match node.kind {
                Kind::Generator => format!(
                    "{:.3} GHz   /   {:.1} dBm",
                    node.config.frequency_hz / 1e9,
                    node.config.power_dbm
                ),
                Kind::Dut => format!("Perte d'insertion   {:.1} dB", node.config.loss_db),
                Kind::Analyzer => format!(
                    "{:.2} – {:.2} GHz   ·   {} pts",
                    node.config.start_hz / 1e9,
                    node.config.stop_hz / 1e9,
                    node.config.points
                ),
                Kind::Python => "Traitement de trace · Python".into(),
                Kind::Peak => "Maximum de la trace · dBm".into(),
                Kind::Limit => format!(
                    "[{:.1}, {:.1}] dBm",
                    node.config.lower_dbm, node.config.upper_dbm
                ),
            };
            painter.text(
                top + egui::vec2(17., 67.) * self.zoom,
                Align2::LEFT_TOP,
                summary,
                FontId::proportional(11. * self.zoom),
                MUTED,
            );
            let done = self.completed.contains(&node.id);
            let state = if done {
                "Exécuté"
            } else if matches!(node.kind, Kind::Generator | Kind::Analyzer) {
                if node.config.resource.starts_with("SIM::") {
                    "Simulé"
                } else {
                    "Instrument réel"
                }
            } else {
                "Prêt"
            };
            painter.circle_filled(
                top + egui::vec2(20., 123.) * self.zoom,
                3. * self.zoom,
                if done { TEAL } else { MUTED },
            );
            painter.text(
                top + egui::vec2(30., 123.) * self.zoom,
                Align2::LEFT_CENTER,
                state,
                FontId::proportional(10. * self.zoom),
                if done { TEAL } else { MUTED },
            );
            let hit = ui.interact(
                card.shrink(10. * self.zoom),
                Id::new(("node", node.id)),
                Sense::click_and_drag(),
            );
            any_node_hovered |= hit.hovered();
            if hit.clicked() {
                self.selected = Some(node.id);
            }
            if hit.drag_started() {
                self.selected = Some(node.id);
                self.drag_before = Some(g.clone());
            }
            if hit.dragged() {
                drag_delta = Some((node.id, ui.input(|i| i.pointer.delta()) / self.zoom));
            }
            if hit.drag_stopped()
                && let Some(old) = self.drag_before.take()
            {
                history.record(old);
            }
            hit.context_menu(|ui| {
                if ui.button("Supprimer le bloc").clicked() {
                    remove = Some(node.id);
                    ui.close();
                }
            });
            let out = top + port_offset(g, node, true).0 * self.zoom;
            painter.circle_filled(out, 5. * self.zoom, port(node.kind.output()));
            let o = ui.interact(
                Rect::from_center_size(out, egui::vec2(22., 22.)),
                Id::new(("out", node.id)),
                Sense::click(),
            );
            if o.clicked() {
                clicked_output = Some(node.id);
            }
            o.on_hover_text(format!(
                "Sortie {} : cliquer puis choisir une entrée",
                node.kind.output().label()
            ));
            if let Some(input) = node.kind.input() {
                let p = top + port_offset(g, node, false).0 * self.zoom;
                painter.circle_filled(p, 5. * self.zoom, port(input));
                let hit = ui.interact(
                    Rect::from_center_size(p, egui::vec2(22., 22.)),
                    Id::new(("in", node.id)),
                    Sense::click(),
                );
                if hit.clicked() {
                    clicked_port = Some(node.id);
                }
                hit.on_hover_text(format!("Entrée {}", input.label()));
            }
        }
        if let Some(id) = clicked_output {
            self.wiring = Some(id);
        }
        if let (Some(from), Some(to)) = (self.wiring, clicked_port) {
            let before = g.clone();
            match g.connect(from, to) {
                Ok(()) => history.record(before),
                Err(e) => error = Some(e.to_string()),
            }
            self.wiring = None;
        }
        if let Some((id, delta)) = drag_delta
            && let Some(n) = g.nodes.iter_mut().find(|n| n.id == id)
        {
            n.position[0] += delta.x;
            n.position[1] += delta.y;
        }
        if let Some(id) = remove {
            history.record(g.clone());
            g.remove(id);
            self.selected = None;
        }
        if response.dragged_by(egui::PointerButton::Middle)
            || (response.dragged() && !any_node_hovered && self.drag_before.is_none())
        {
            self.pan += ui.input(|i| i.pointer.delta());
        }
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.wiring = None;
        }
        painter.text(
            rect.left_bottom() + egui::vec2(14., -12.),
            Align2::LEFT_BOTTOM,
            "Molette : zoom · Glisser le fond : déplacer · Ports : relier · Clic droit : retirer",
            FontId::proportional(10.),
            MUTED,
        );
        error
    }
}
fn port_offset(g: &Graph, node: &rf_core::Node, output: bool) -> (Vec2, Vec2) {
    let neighbor = if output {
        g.edges
            .iter()
            .find(|e| e.from == node.id)
            .and_then(|e| g.node(e.to))
    } else {
        g.edges
            .iter()
            .find(|e| e.to == node.id)
            .and_then(|e| g.node(e.from))
    };
    let direction = neighbor
        .map(|n| {
            egui::vec2(
                n.position[0] - node.position[0],
                n.position[1] - node.position[1],
            )
        })
        .unwrap_or(egui::vec2(if output { 1. } else { -1. }, 0.));
    if direction.y.abs() > direction.x.abs() {
        if direction.y > 0. {
            (
                egui::vec2(NODE_SIZE.x * 0.5, NODE_SIZE.y),
                egui::vec2(0., 1.),
            )
        } else {
            (egui::vec2(NODE_SIZE.x * 0.5, 0.), egui::vec2(0., -1.))
        }
    } else if direction.x > 0. {
        (
            egui::vec2(NODE_SIZE.x, NODE_SIZE.y * 0.63),
            egui::vec2(1., 0.),
        )
    } else {
        (egui::vec2(0., NODE_SIZE.y * 0.63), egui::vec2(-1., 0.))
    }
}
fn wire(
    p: &egui::Painter,
    from: Pos2,
    to: Pos2,
    normals: [Vec2; 2],
    color: egui::Color32,
    zoom: f32,
) {
    let handle = (from.distance(to) * 0.4).clamp(12. * zoom, 120. * zoom);
    let c1 = from + normals[0] * handle;
    let c2 = to + normals[1] * handle;
    let points = (0..=32)
        .map(|i| {
            let t = i as f32 / 32.;
            let u = 1. - t;
            Pos2::new(
                u.powi(3) * from.x
                    + 3. * u * u * t * c1.x
                    + 3. * u * t * t * c2.x
                    + t.powi(3) * to.x,
                u.powi(3) * from.y
                    + 3. * u * u * t * c1.y
                    + 3. * u * t * t * c2.y
                    + t.powi(3) * to.y,
            )
        })
        .collect();
    p.add(egui::Shape::line(
        points,
        Stroke::new(2. * zoom.max(0.7), color.gamma_multiply(0.85)),
    ));
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn history_restores_connections() {
        let mut g = Graph::demo();
        let old = g.clone();
        let mut h = History::default();
        h.record(old.clone());
        g.remove(3);
        h.undo(&mut g);
        assert_eq!(g, old);
        h.redo(&mut g);
        assert!(g.node(3).is_none());
    }
    #[test]
    fn new_edit_discards_redo() {
        let mut g = Graph::demo();
        let mut h = History::default();
        h.record(g.clone());
        g.remove(3);
        h.undo(&mut g);
        h.record(g.clone());
        g.remove(1);
        h.redo(&mut g);
        assert!(g.node(1).is_none());
    }
}
