use crate::{theme::*, visuals};
use eframe::egui::{self, Align2, FontId, Id, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};
use rf_core::{Graph, Kind, Node};
pub const NODE_SIZE: Vec2 = egui::vec2(240., 208.);
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
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum Tool {
    #[default]
    Select,
    Wire,
    Pan,
}
pub struct Canvas {
    pub zoom: f32,
    pub pan: Vec2,
    pub selected: Option<u64>,
    pub wiring: Option<(u64, usize)>,
    pub tool: Tool,
    pub snap: bool,
    pub orthogonal: bool,
    pub completed: Vec<u64>,
    waypoints: Vec<[f32; 2]>,
    drag_before: Option<Graph>,
    fit: bool,
}
impl Default for Canvas {
    fn default() -> Self {
        Self {
            zoom: 1.,
            pan: egui::vec2(12., -24.),
            selected: Some(3),
            wiring: None,
            tool: Tool::Select,
            snap: true,
            orthogonal: true,
            completed: Vec::new(),
            waypoints: Vec::new(),
            drag_before: None,
            fit: true,
        }
    }
}
impl Canvas {
    pub fn configured(snap: bool, orthogonal: bool) -> Self {
        Self {
            snap,
            orthogonal,
            ..Default::default()
        }
    }
    pub fn fit(&mut self) {
        self.fit = true;
    }
    pub fn set_tool(&mut self, tool: Tool) {
        self.cancel_wire();
        self.tool = tool;
    }
    pub fn cancel_wire(&mut self) {
        self.wiring = None;
        self.waypoints.clear();
    }
    fn screen(&self, origin: Pos2, p: [f32; 2]) -> Pos2 {
        origin + self.pan + egui::vec2(p[0], p[1]) * self.zoom
    }
    pub fn connect_to(
        &mut self,
        g: &mut Graph,
        h: &mut History,
        to: u64,
        to_port: usize,
    ) -> Result<(), String> {
        let Some((from, from_port)) = self.wiring else {
            return Ok(());
        };
        let before = g.clone();
        g.connect_ports(from, from_port, to, to_port)
            .map_err(|e| e.to_string())?;
        g.edges.last_mut().expect("new cable").waypoints = std::mem::take(&mut self.waypoints);
        h.record(before);
        self.wiring = None;
        Ok(())
    }
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        g: &mut Graph,
        history: &mut History,
        height: f32,
    ) -> Option<String> {
        if self.wiring.is_some_and(|(id, _)| g.node(id).is_none()) {
            self.cancel_wire();
        }
        if self.selected.is_some_and(|id| g.node(id).is_none()) {
            self.selected = None;
        }
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
                - 35.;
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
                + 35.;
            let max_y = g
                .nodes
                .iter()
                .map(|n| n.position[1] + NODE_SIZE.y)
                .fold(f32::NEG_INFINITY, f32::max)
                + 50.;
            self.zoom = (rect.width() / (max_x - min_x))
                .min(rect.height() / (max_y - min_y))
                .clamp(0.2, 1.3);
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
                self.zoom = (old * (scroll * 0.002).exp()).clamp(0.2, 2.);
                self.pan = (mouse - rect.min) - (mouse - rect.min - self.pan) * (self.zoom / old);
            }
        }
        let mut error = None;
        let mut disconnect = None;
        let mut cable_hover = false;
        for (index, e) in g.edges.iter().enumerate() {
            if let (Some(a), Some(b)) = (g.node(e.from), g.node(e.to)) {
                let from = self.screen(rect.min, a.position)
                    + port_offset(a, true, e.from_port) * self.zoom;
                let to = self.screen(rect.min, b.position)
                    + port_offset(b, false, e.to_port) * self.zoom;
                let waypoints: Vec<_> = e
                    .waypoints
                    .iter()
                    .map(|p| self.screen(rect.min, *p))
                    .collect();
                let points = route(from, to, &waypoints, self.orthogonal, self.zoom);
                let color = port(a.kind.outputs()[e.from_port].port);
                painter.add(egui::Shape::line(
                    points.clone(),
                    Stroke::new(2. * self.zoom.max(0.7), color.gamma_multiply(0.85)),
                ));
                if let Some(mouse) = ui.input(|i| i.pointer.hover_pos())
                    && rect.contains(mouse)
                    && points
                        .windows(2)
                        .any(|w| segment_distance(mouse, w[0], w[1]) < 6.)
                {
                    cable_hover = true;
                    let hit = ui.interact(
                        Rect::from_center_size(mouse, egui::vec2(12., 12.)),
                        Id::new(("wire", index)),
                        Sense::click(),
                    );
                    hit.on_hover_text(format!(
                        "{} → {} · {}\nClic droit : retirer ou réinitialiser le parcours",
                        a.title,
                        b.title,
                        a.kind.outputs()[e.from_port].name
                    ))
                    .context_menu(|ui| {
                        if ui.button("Retirer le câble").clicked() {
                            disconnect = Some((index, true));
                            ui.close();
                        }
                        if ui.button("Réinitialiser le parcours").clicked() {
                            disconnect = Some((index, false));
                            ui.close();
                        }
                    });
                }
            }
        }
        if let Some((i, remove)) = disconnect {
            history.record(g.clone());
            if remove {
                g.edges.remove(i);
            } else {
                g.edges[i].waypoints.clear();
            }
        }
        if let Some((id, index)) = self.wiring
            && let Some(n) = g.node(id)
            && let Some(mouse) = ui.input(|i| i.pointer.hover_pos())
        {
            let from = self.screen(rect.min, n.position) + port_offset(n, true, index) * self.zoom;
            let waypoints: Vec<_> = self
                .waypoints
                .iter()
                .map(|p| self.screen(rect.min, *p))
                .collect();
            painter.add(egui::Shape::line(
                route(from, mouse, &waypoints, self.orthogonal, self.zoom),
                Stroke::new(2., port(n.kind.outputs()[index].port)),
            ));
            for p in waypoints {
                painter.circle_filled(p, 3., TEXT);
            }
        }
        let mut clicked_input = None;
        let mut clicked_output = None;
        let mut delta = None;
        let mut stopped = None;
        let mut hovered_node = false;
        let mut hovered_port = false;
        let mut remove = None;
        for node in &g.nodes {
            let top = self.screen(rect.min, node.position);
            let card = Rect::from_min_size(top, NODE_SIZE * self.zoom);
            if !rect.intersects(card) {
                continue;
            }
            let color = kind(node.kind);
            let selected = self.selected == Some(node.id);
            painter.rect_filled(
                card.translate(egui::vec2(3., 5.)),
                9.,
                egui::Color32::BLACK.gamma_multiply(0.22),
            );
            painter.rect_filled(card, 8., CARD);
            painter.rect_stroke(
                card,
                8.,
                Stroke::new(
                    if selected { 2. } else { 1. },
                    if selected { color } else { BORDER },
                ),
                StrokeKind::Inside,
            );
            painter.rect_filled(
                Rect::from_min_size(top, egui::vec2(NODE_SIZE.x, 4.) * self.zoom),
                2.,
                color,
            );
            painter.text(
                top + egui::vec2(13., 13.) * self.zoom,
                Align2::LEFT_TOP,
                node.kind.tag(),
                FontId::proportional(10. * self.zoom),
                color,
            );
            let title = if node.title.chars().count() > 28 {
                format!("{}…", node.title.chars().take(27).collect::<String>())
            } else {
                node.title.clone()
            };
            painter.text(
                top + egui::vec2(13., 30.) * self.zoom,
                Align2::LEFT_TOP,
                title,
                FontId::proportional(14. * self.zoom),
                TEXT,
            );
            visuals::symbol(
                &painter,
                Rect::from_min_size(
                    top + egui::vec2(33., 55.) * self.zoom,
                    egui::vec2(174., 80.) * self.zoom,
                ),
                node.kind,
            );
            painter.text(
                top + egui::vec2(13., 150.) * self.zoom,
                Align2::LEFT_TOP,
                summary(node),
                FontId::proportional(11. * self.zoom),
                MUTED,
            );
            let done = self.completed.contains(&node.id);
            let state = if done {
                "Exécuté"
            } else if node.kind.is_extended() {
                "Modèle simulé"
            } else if node.config.resource.starts_with("SIM::") {
                "Simulation"
            } else {
                "Matériel"
            };
            painter.circle_filled(
                top + egui::vec2(16., 185.) * self.zoom,
                3. * self.zoom,
                if done { TEAL } else { MUTED },
            );
            painter.text(
                top + egui::vec2(25., 185.) * self.zoom,
                Align2::LEFT_CENTER,
                state,
                FontId::proportional(10. * self.zoom),
                if done { TEAL } else { MUTED },
            );
            let hit = ui.interact(
                card.shrink(12. * self.zoom),
                Id::new(("node", node.id)),
                if self.tool == Tool::Select {
                    Sense::click_and_drag()
                } else {
                    Sense::click()
                },
            );
            hovered_node |= hit.hovered();
            if hit.clicked() {
                self.selected = Some(node.id);
                if self.tool == Tool::Wire {
                    if let Some((source, source_port)) = self.wiring {
                        if let Some(a) = g.node(source) {
                            let matches: Vec<_> = node
                                .kind
                                .inputs()
                                .iter()
                                .enumerate()
                                .filter(|(i, p)| {
                                    p.port == a.kind.outputs()[source_port].port
                                        && !g
                                            .edges
                                            .iter()
                                            .any(|e| e.to == node.id && e.to_port == *i)
                                })
                                .map(|(i, _)| i)
                                .collect();
                            if matches.len() == 1 {
                                clicked_input = Some((node.id, matches[0]));
                            } else {
                                error=Some("Choisir le port d'entrée nommé ; plusieurs choix ou aucun port compatible".into());
                            }
                        }
                    } else if node.kind.outputs().len() == 1 {
                        clicked_output = Some((node.id, 0));
                    } else {
                        error = Some("Choisir un port de sortie nommé".into());
                    }
                }
            }
            if hit.drag_started() {
                self.selected = Some(node.id);
                self.drag_before = Some(g.clone());
            }
            if hit.dragged() {
                delta = Some((node.id, ui.input(|i| i.pointer.delta()) / self.zoom));
            }
            if hit.drag_stopped() {
                stopped = Some(node.id);
            }
            hit.context_menu(|ui| {
                if ui.button("Supprimer le bloc").clicked() {
                    remove = Some(node.id);
                    ui.close();
                }
            });
            for output in [false, true] {
                let ports = if output {
                    node.kind.outputs()
                } else {
                    node.kind.inputs()
                };
                for (index, terminal) in ports.iter().enumerate() {
                    let position = top + port_offset(node, output, index) * self.zoom;
                    let compatible = !output
                        && self.wiring.is_some_and(|(id, p)| {
                            g.node(id)
                                .is_some_and(|a| a.kind.outputs()[p].port == terminal.port)
                        })
                        && !g
                            .edges
                            .iter()
                            .any(|e| e.to == node.id && e.to_port == index);
                    if compatible {
                        painter.circle_stroke(position, 9. * self.zoom, Stroke::new(1.5, TEAL));
                    }
                    painter.circle_filled(position, 5. * self.zoom, port(terminal.port));
                    painter.text(
                        position + egui::vec2(if output { -9. } else { 9. }, -9.) * self.zoom,
                        if output {
                            Align2::RIGHT_BOTTOM
                        } else {
                            Align2::LEFT_BOTTOM
                        },
                        terminal.name,
                        FontId::proportional(9. * self.zoom),
                        port(terminal.port),
                    );
                    let terminal_hit = ui.interact(
                        Rect::from_center_size(position, egui::vec2(20., 20.)),
                        Id::new(("terminal", node.id, output, index)),
                        Sense::click(),
                    );
                    hovered_port |= terminal_hit.hovered();
                    if terminal_hit.clicked() {
                        if output {
                            clicked_output = Some((node.id, index));
                        } else {
                            clicked_input = Some((node.id, index));
                        }
                    }
                    terminal_hit.on_hover_text(format!(
                        "{} {} · {}{}",
                        if output { "Sortie" } else { "Entrée" },
                        terminal.name,
                        terminal.port.label(),
                        if compatible { " · compatible" } else { "" }
                    ));
                }
            }
        }
        if let Some(output) = clicked_output {
            self.cancel_wire();
            self.wiring = Some(output);
        }
        if let Some((id, index)) = clicked_input
            && let Err(e) = self.connect_to(g, history, id, index)
        {
            error = Some(e);
        }
        if let Some((id, d)) = delta
            && let Some(n) = g.nodes.iter_mut().find(|n| n.id == id)
        {
            n.position[0] = (n.position[0] + d.x).clamp(-1e6, 1e6);
            n.position[1] = (n.position[1] + d.y).clamp(-1e6, 1e6);
        }
        if let Some(id) = stopped
            && let Some(old) = self.drag_before.take()
        {
            if self.snap
                && let Some(n) = g.nodes.iter_mut().find(|n| n.id == id)
            {
                n.position = n.position.map(|v| (v / 24.).round() * 24.);
            }
            history.record(old);
        }
        if let Some(id) = remove {
            history.record(g.clone());
            g.remove(id);
            self.selected = None;
            self.cancel_wire();
        }
        if response.dragged_by(egui::PointerButton::Middle)
            || (response.dragged()
                && !hovered_port
                && ((self.tool == Tool::Pan)
                    || (!hovered_node && self.drag_before.is_none() && self.wiring.is_none())))
        {
            self.pan += ui.input(|i| i.pointer.delta());
        }
        if self.tool == Tool::Wire
            && self.wiring.is_some()
            && response.clicked()
            && !hovered_node
            && !hovered_port
            && !cable_hover
            && let Some(mouse) = response.interact_pointer_pos()
            && self.waypoints.len() < 64
        {
            let p = (mouse - rect.min - self.pan) / self.zoom;
            self.waypoints.push([p.x, p.y]);
        }
        if response.secondary_clicked()
            || (!ui.ctx().wants_keyboard_input() && ui.input(|i| i.key_pressed(egui::Key::Escape)))
        {
            self.cancel_wire();
        }
        if rect.contains(ui.input(|i| i.pointer.hover_pos()).unwrap_or(Pos2::ZERO)) {
            ui.ctx().set_cursor_icon(match self.tool {
                Tool::Select => egui::CursorIcon::Default,
                Tool::Wire => egui::CursorIcon::Crosshair,
                Tool::Pan => egui::CursorIcon::Grab,
            });
        }
        painter.text(
            rect.left_bottom() + egui::vec2(12., -10.),
            Align2::LEFT_BOTTOM,
            if self.tool == Tool::Wire {
                "Câblage : sortie → entrée · Clic fond : coude · Échap : annuler"
            } else {
                "Molette : zoom · Glisser : déplacer · W : câblage · H : déplacement"
            },
            FontId::proportional(10.),
            MUTED,
        );
        error
    }
}
fn summary(n: &Node) -> String {
    let c = &n.config;
    match n.kind {
        Kind::Generator => format!("{:.3} GHz / {:.1} dBm", c.frequency_hz / 1e9, c.power_dbm),
        Kind::Dut => format!("Perte {:.1} dB · NF {:.1} dB", c.loss_db, c.noise_figure_db),
        Kind::Analyzer => format!(
            "{:.2}–{:.2} GHz · {} pts",
            c.start_hz / 1e9,
            c.stop_hz / 1e9,
            c.points
        ),
        Kind::Pna | Kind::PnaX => format!(
            "{} · {:.2}–{:.2} GHz",
            c.s_parameter,
            c.start_hz / 1e9,
            c.stop_hz / 1e9
        ),
        Kind::Awg => format!("I/Q · {:.1} MS/s", c.sample_rate_hz / 1e6),
        Kind::Dac | Kind::Adc => format!("{} bits · ±{:.2} V", c.resolution_bits, c.voltage_v),
        Kind::IqModulator => format!("I + jQ · conversion {:.1} dB", c.loss_db),
        Kind::VariableResistor => format!("{:.2} Ω", c.resistance_ohm),
        Kind::Thermometer | Kind::Thermostream => format!("{:.1} °C", c.temperature_c),
        Kind::NoiseFigureMeter => format!("NF {:.2} dB · idéal", c.noise_figure_db),
        Kind::PowerSensor | Kind::PowerMeter => "Puissance RF · dBm".into(),
        Kind::Python => "Traitement de trace · Python".into(),
        Kind::Peak => "Maximum · dBm".into(),
        Kind::Limit => format!("[{:.1}, {:.1}] dBm", c.lower_dbm, c.upper_dbm),
    }
}
fn port_offset(n: &Node, output: bool, index: usize) -> Vec2 {
    let count = if output {
        n.kind.outputs().len()
    } else {
        n.kind.inputs().len()
    };
    egui::vec2(
        if output { NODE_SIZE.x } else { 0. },
        65. + (index + 1) as f32 / (count + 1) as f32 * 105.,
    )
}
fn route(from: Pos2, to: Pos2, waypoints: &[Pos2], orthogonal: bool, zoom: f32) -> Vec<Pos2> {
    if !waypoints.is_empty() {
        let mut points = vec![from];
        let mut last = from;
        for target in waypoints.iter().copied().chain(std::iter::once(to)) {
            if orthogonal {
                points.push(Pos2::new(target.x, last.y));
            }
            points.push(target);
            last = target;
        }
        return points;
    }
    if orthogonal {
        if to.x >= from.x + 24. * zoom {
            let x = (from.x + to.x) * 0.5;
            vec![from, Pos2::new(x, from.y), Pos2::new(x, to.y), to]
        } else {
            let x = from.x + 24. * zoom;
            let y = from.y.max(to.y) + 110. * zoom;
            vec![
                from,
                Pos2::new(x, from.y),
                Pos2::new(x, y),
                Pos2::new(to.x - 24. * zoom, y),
                Pos2::new(to.x - 24. * zoom, to.y),
                to,
            ]
        }
    } else {
        let h = (from.distance(to) * 0.4).clamp(12. * zoom, 120. * zoom);
        let a = from + egui::vec2(h, 0.);
        let b = to - egui::vec2(h, 0.);
        (0..=32)
            .map(|i| {
                let t = i as f32 / 32.;
                let u = 1. - t;
                Pos2::new(
                    u.powi(3) * from.x
                        + 3. * u * u * t * a.x
                        + 3. * u * t * t * b.x
                        + t.powi(3) * to.x,
                    u.powi(3) * from.y
                        + 3. * u * u * t * a.y
                        + 3. * u * t * t * b.y
                        + t.powi(3) * to.y,
                )
            })
            .collect()
    }
}
fn segment_distance(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let d = b - a;
    let t = if d.length_sq() == 0. {
        0.
    } else {
        ((p - a).dot(d) / d.length_sq()).clamp(0., 1.)
    };
    p.distance(a + d * t)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pointer_events_create_custom_wire_through_real_egui_hit_regions() {
        let ctx = egui::Context::default();
        let mut g = Graph::default();
        let from = g.add(Kind::Generator, [0., 0.]);
        let to = g.add(Kind::Analyzer, [420., 0.]);
        let mut canvas = Canvas {
            fit: false,
            pan: Vec2::ZERO,
            tool: Tool::Wire,
            ..Default::default()
        };
        let mut h = History::default();
        let mut draw = |events: Vec<egui::Event>| {
            let mut origin = Pos2::ZERO;
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(900., 600.))),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        origin = ui.available_rect_before_wrap().min;
                        assert!(canvas.show(ui, &mut g, &mut h, 450.).is_none());
                    });
                },
            );
            origin
        };
        let origin = draw(vec![]);
        draw(vec![]);
        for pos in [
            origin + egui::vec2(240., 117.5),
            origin + egui::vec2(330., 300.),
            origin + egui::vec2(420., 117.5),
        ] {
            draw(vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ]);
            draw(vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                },
            ]);
        }
        assert_eq!(g.edges.len(), 1);
        assert_eq!((g.edges[0].from, g.edges[0].to), (from, to));
        assert_eq!(g.edges[0].waypoints, vec![[330., 300.]]);
    }
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
    #[test]
    fn custom_wire_persists_waypoints_and_undo() {
        let mut g = Graph::default();
        let a = g.add(Kind::Generator, [0., 0.]);
        let b = g.add(Kind::Analyzer, [400., 0.]);
        let mut c = Canvas {
            wiring: Some((a, 0)),
            waypoints: vec![[300., 250.]],
            ..Default::default()
        };
        let mut h = History::default();
        c.connect_to(&mut g, &mut h, b, 0).unwrap();
        let loaded = rf_core::Project::from_json(
            &rf_core::Project {
                graph: g.clone(),
                ..Default::default()
            }
            .to_json()
            .unwrap(),
        )
        .unwrap();
        assert_eq!(loaded.graph.edges[0].waypoints, vec![[300., 250.]]);
        h.undo(&mut g);
        assert!(g.edges.is_empty());
        h.redo(&mut g);
        assert_eq!(g.edges.len(), 1);
    }
    #[test]
    fn incompatible_wire_keeps_source_until_cancel() {
        let mut g = Graph::default();
        let a = g.add(Kind::Generator, [0., 0.]);
        let b = g.add(Kind::Dac, [400., 0.]);
        let mut c = Canvas {
            wiring: Some((a, 0)),
            ..Default::default()
        };
        assert!(c.connect_to(&mut g, &mut History::default(), b, 0).is_err());
        assert_eq!(c.wiring, Some((a, 0)));
        assert!(g.edges.is_empty());
        c.cancel_wire();
        assert!(c.wiring.is_none());
    }
}
