use crate::{theme::*, visuals};
use eframe::egui::{self, Align2, Id, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};
use rf_core::{Graph, Kind, Node, PhysicalPortKind};
// Approximately 1 mm in egui screen points at the usual 96 logical DPI.
const PIN_MAGNET_MARGIN: f32 = 96. / 25.4;
use std::collections::BTreeSet;
pub const NODE_SIZE: Vec2 = egui::vec2(240., 208.);
#[derive(Clone, Copy)]
struct SnapTarget {
    node: u64,
    index: usize,
    physical: bool,
    position: Pos2,
}
fn occupied_physical(g: &Graph, node: u64, index: usize) -> bool {
    g.physical_connections
        .iter()
        .any(|e| e.from == node && e.from_port == index || e.to == node && e.to_port == index)
}
fn physical_color(kind: rf_core::PhysicalPortKind) -> egui::Color32 {
    match kind {
        rf_core::PhysicalPortKind::Rf => gold(),
        rf_core::PhysicalPortKind::DcPositive => red(),
        rf_core::PhysicalPortKind::DcReturn => blue(),
    }
}
pub(crate) fn physical_port_offset(n: &Node, index: usize) -> Vec2 {
    let ports = n.physical_ports();
    if n.kind.max_rf_ports().is_some() {
        return egui::vec2(
            0.,
            55. + (index + 1) as f32 / (ports.len() + 1) as f32 * 94.,
        );
    }
    let rf = ports
        .iter()
        .take_while(|p| p.kind == rf_core::PhysicalPortKind::Rf)
        .count();
    if index < rf {
        return match index {
            0 => egui::vec2(0., 90.),
            1 => egui::vec2(NODE_SIZE.x, 90.),
            _ => egui::vec2(NODE_SIZE.x, 120.),
        };
    }
    let count = ports.len() - rf;
    egui::vec2(
        20. + (index - rf) as f32 * 200. / (count.saturating_sub(1).max(1)) as f32,
        NODE_SIZE.y,
    )
}
pub(crate) fn physical_port_exit(n: &Node, index: usize) -> Vec2 {
    let p = physical_port_offset(n, index);
    if p.y == NODE_SIZE.y {
        egui::vec2(0., 1.)
    } else if p.x == 0. {
        egui::vec2(-1., 0.)
    } else {
        egui::vec2(1., 0.)
    }
}
fn physical_route(
    from: Pos2,
    to: Pos2,
    points: &[Pos2],
    orthogonal: bool,
    zoom: f32,
    a: Vec2,
    b: Vec2,
) -> Vec<Pos2> {
    if !orthogonal || !points.is_empty() {
        return route(from, to, points, orthogonal, zoom);
    }
    let start = from + a * 24. * zoom;
    let end = to + b * 24. * zoom;
    let middle = if a.y > 0. || b.y > 0. {
        let y = start.y.max(end.y) + 24. * zoom;
        vec![Pos2::new(start.x, y), Pos2::new(end.x, y)]
    } else if a.x == b.x {
        let x = if a.x < 0. {
            start.x.min(end.x)
        } else {
            start.x.max(end.x)
        };
        vec![Pos2::new(x, start.y), Pos2::new(x, end.y)]
    } else {
        let x = (start.x + end.x) * 0.5;
        vec![Pos2::new(x, start.y), Pos2::new(x, end.y)]
    };
    std::iter::once(from)
        .chain(std::iter::once(start))
        .chain(middle)
        .chain([end, to])
        .collect()
}
#[derive(Default, Clone)]
pub struct History {
    undo: Vec<Graph>,
    redo: Vec<Graph>,
}
impl History {
    pub fn record(&mut self, before: Graph) {
        self.undo.push(before);
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
    pub selected_ids: BTreeSet<u64>,
    pub annotation: Option<u64>,
    pub help_request: Option<Kind>,
    pub configure_request: Option<u64>,
    pub physical_wiring: Option<(u64, usize)>,
    pub active_node: Option<u64>,
    pub auto_route: bool,
    pub routes_dirty: bool,
    marquee: Option<Pos2>,
    focus_port: Option<(bool, usize)>,
    keyboard_active: bool,
    pub wiring: Option<(u64, usize)>,
    pub tool: Tool,
    pub snap: bool,
    pub orthogonal: bool,
    pub completed: Vec<u64>,
    waypoints: Vec<[f32; 2]>,
    drag_before: Option<Graph>,
    fit: bool,
    auto_fit: bool,
    last_size: Option<Vec2>,
}
impl Default for Canvas {
    fn default() -> Self {
        Self {
            zoom: 1.,
            pan: egui::vec2(12., -24.),
            selected: Some(3),
            selected_ids: [3].into_iter().collect(),
            annotation: None,
            help_request: None,
            configure_request: None,
            physical_wiring: None,
            active_node: None,
            auto_route: true,
            routes_dirty: false,
            marquee: None,
            focus_port: None,
            keyboard_active: true,
            wiring: None,
            tool: Tool::Select,
            snap: true,
            orthogonal: true,
            completed: Vec::new(),
            waypoints: Vec::new(),
            drag_before: None,
            fit: true,
            auto_fit: true,
            last_size: None,
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
    pub fn select_only(&mut self, id: u64) {
        self.selected = Some(id);
        self.selected_ids = [id].into_iter().collect();
        self.annotation = None;
        self.focus_port = None;
    }
    pub fn select(&mut self, id: u64, toggle: bool) {
        if toggle {
            if !self.selected_ids.remove(&id) {
                self.selected_ids.insert(id);
            }
            self.selected = if self.selected_ids.contains(&id) {
                Some(id)
            } else {
                self.selected_ids.iter().next().copied()
            };
            self.annotation = None;
        } else {
            self.select_only(id);
        }
    }
    pub fn fit(&mut self) {
        self.fit = true;
        self.auto_fit = true;
    }
    pub fn restore_view(&mut self, pan: [f32; 2], zoom: f32) {
        self.pan = Vec2::from(pan);
        self.zoom = zoom;
        self.fit = false;
        self.auto_fit = false;
    }
    pub fn set_tool(&mut self, tool: Tool) {
        self.cancel_wire();
        self.tool = tool;
    }
    pub fn cancel_wire(&mut self) {
        self.wiring = None;
        self.physical_wiring = None;
        self.waypoints.clear();
    }
    pub fn is_wiring(&self) -> bool {
        self.wiring.is_some() || self.physical_wiring.is_some()
    }
    pub fn connect_physical_to(
        &mut self,
        g: &mut Graph,
        h: &mut History,
        to: u64,
        to_port: usize,
    ) -> Result<(), String> {
        let Some((from, from_port)) = self.physical_wiring else {
            return Ok(());
        };
        let before = g.clone();
        g.connect_physical(from, from_port, to, to_port)
            .map_err(|e| e.to_string())?;
        let automatic = self.auto_route && self.orthogonal && self.waypoints.is_empty();
        let cable = g.physical_connections.last_mut().unwrap();
        cable.waypoints = std::mem::take(&mut self.waypoints);
        cable.auto_routed = automatic;
        h.record(before);
        self.physical_wiring = None;
        self.routes_dirty |= automatic;
        Ok(())
    }
    fn nearest_target(&self, g: &Graph, origin: Pos2, mouse: Pos2) -> Option<SnapTarget> {
        let mut nearest = None;
        let mut distance = f32::INFINITY;
        for n in &g.nodes {
            if let Some((id, index)) = self.physical_wiring {
                let source = g.node(id)?.physical_ports().get(index)?.clone();
                for (i, p) in n.physical_ports().iter().enumerate() {
                    if n.id == id
                        || p.kind != source.kind
                        || (p.kind != PhysicalPortKind::Rf && p.direction == source.direction)
                        || occupied_physical(g, n.id, i)
                    {
                        continue;
                    }
                    let pos =
                        self.screen(origin, n.position) + physical_port_offset(n, i) * self.zoom;
                    let d = pos.distance(mouse);
                    if d <= 5. * self.zoom.max(0.7) + PIN_MAGNET_MARGIN && d < distance {
                        distance = d;
                        nearest = Some(SnapTarget {
                            node: n.id,
                            index: i,
                            physical: true,
                            position: pos,
                        });
                    }
                }
            } else if let Some((id, index)) = self.wiring {
                let p = g.node(id)?.kind.outputs().get(index)?;
                for (i, target) in n.kind.inputs().iter().enumerate() {
                    if n.id == id
                        || target.port != p.port
                        || g.edges.iter().any(|e| e.to == n.id && e.to_port == i)
                    {
                        continue;
                    }
                    let pos =
                        self.screen(origin, n.position) + port_offset(n, false, i) * self.zoom;
                    let d = pos.distance(mouse);
                    if d <= 5. * self.zoom.max(0.7) + PIN_MAGNET_MARGIN && d < distance {
                        distance = d;
                        nearest = Some(SnapTarget {
                            node: n.id,
                            index: i,
                            physical: false,
                            position: pos,
                        });
                    }
                }
            }
        }
        nearest
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
        let automatic = self.auto_route && self.orthogonal && self.waypoints.is_empty();
        let points = std::mem::take(&mut self.waypoints);
        self.routes_dirty |= automatic;
        g.edges.last_mut().expect("new cable").waypoints = points;
        g.edges.last_mut().unwrap().auto_routed = automatic;
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
        if self
            .wiring
            .is_some_and(|(id, p)| g.node(id).is_none_or(|n| p >= n.kind.outputs().len()))
            || self
                .physical_wiring
                .is_some_and(|(id, p)| g.node(id).is_none_or(|n| p >= n.physical_ports().len()))
        {
            self.cancel_wire();
        }
        self.selected_ids.retain(|id| g.node(*id).is_some());
        if self.selected_ids.len() <= 1
            && let Some(id) = self.selected.filter(|id| g.node(*id).is_some())
        {
            self.selected_ids.insert(id);
        }
        if self.selected.is_some_and(|id| g.node(id).is_none()) {
            self.selected = None;
        }
        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), height),
            Sense::click_and_drag(),
        );
        if ui.input(|i| i.pointer.any_pressed()) {
            self.keyboard_active = ui
                .input(|i| i.pointer.interact_pos())
                .is_some_and(|p| rect.contains(p));
            if self.keyboard_active {
                response.request_focus();
            }
        }
        if self.last_size.is_some_and(|size| size != rect.size()) && self.auto_fit {
            self.fit = true;
        }
        self.last_size = Some(rect.size());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 8., bg());
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
                        border().gamma_multiply(0.75),
                    );
                }
            }
        }
        if response.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.
                && let Some(mouse) = ui.input(|i| i.pointer.hover_pos())
            {
                self.auto_fit = false;
                let old = self.zoom;
                self.zoom = (old * (scroll * 0.002).exp()).clamp(0.2, 2.);
                self.pan = (mouse - rect.min) - (mouse - rect.min - self.pan) * (self.zoom / old);
            }
        }
        let snap_target = ui
            .input(|i| i.pointer.hover_pos())
            .filter(|p| rect.contains(*p))
            .and_then(|p| self.nearest_target(g, rect.min, p));
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
                        if ui.button(crate::i18n::t("Retirer le câble")).clicked() {
                            disconnect = Some((index, true));
                            ui.close();
                        }
                        if ui
                            .button(crate::i18n::t("Réinitialiser le parcours"))
                            .clicked()
                        {
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
        let mut physical_disconnect = None;
        for (index, e) in g.physical_connections.iter().enumerate() {
            if let (Some(a), Some(b)) = (g.node(e.from), g.node(e.to)) {
                let from = self.screen(rect.min, a.position)
                    + physical_port_offset(a, e.from_port) * self.zoom;
                let to = self.screen(rect.min, b.position)
                    + physical_port_offset(b, e.to_port) * self.zoom;
                let points: Vec<_> = e
                    .waypoints
                    .iter()
                    .map(|p| self.screen(rect.min, *p))
                    .collect();
                let points = physical_route(
                    from,
                    to,
                    &points,
                    self.orthogonal,
                    self.zoom,
                    physical_port_exit(a, e.from_port),
                    physical_port_exit(b, e.to_port),
                );
                let color = physical_color(a.physical_ports()[e.from_port].kind);
                painter.add(egui::Shape::line(
                    points.clone(),
                    Stroke::new(2.2 * self.zoom.max(0.7), color),
                ));
                if let Some(mouse) = ui
                    .input(|i| i.pointer.hover_pos())
                    .filter(|p| rect.contains(*p))
                    && points
                        .windows(2)
                        .any(|w| segment_distance(mouse, w[0], w[1]) < 6.)
                {
                    cable_hover = true;
                    ui.interact(
                        Rect::from_center_size(mouse, egui::vec2(12., 12.)),
                        Id::new(("physical-cable", index)),
                        Sense::click(),
                    )
                    .on_hover_text(format!("{} ↔ {} · liaison physique", a.title, b.title))
                    .context_menu(|ui| {
                        if ui.button("Retirer le câble").clicked() {
                            physical_disconnect = Some((index, true));
                            ui.close();
                        }
                        if ui.button("Réinitialiser le parcours").clicked() {
                            physical_disconnect = Some((index, false));
                            ui.close();
                        }
                    });
                }
            }
        }
        if let Some((i, remove)) = physical_disconnect {
            history.record(g.clone());
            if remove {
                g.remove_physical(i);
            } else {
                g.physical_connections[i].waypoints.clear();
                g.physical_connections[i].auto_routed = true;
                self.routes_dirty = true;
            }
        }
        if let Some((id, index)) = self.physical_wiring
            && let Some(n) = g.node(id)
            && let Some(mouse) = ui.input(|i| i.pointer.hover_pos())
        {
            let from =
                self.screen(rect.min, n.position) + physical_port_offset(n, index) * self.zoom;
            let to = snap_target.map_or(mouse, |s| s.position);
            let points: Vec<_> = self
                .waypoints
                .iter()
                .map(|p| self.screen(rect.min, *p))
                .collect();
            let exit = snap_target
                .and_then(|s| g.node(s.node).map(|n| physical_port_exit(n, s.index)))
                .unwrap_or(egui::vec2(-1., 0.));
            painter.add(egui::Shape::line(
                physical_route(
                    from,
                    to,
                    &points,
                    self.orthogonal,
                    self.zoom,
                    physical_port_exit(n, index),
                    exit,
                ),
                Stroke::new(2., physical_color(n.physical_ports()[index].kind)),
            ));
        }
        if let Some(target) = snap_target {
            painter.circle_stroke(target.position, 9., Stroke::new(2., teal()));
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
                route(
                    from,
                    snap_target.map_or(mouse, |s| s.position),
                    &waypoints,
                    self.orthogonal,
                    self.zoom,
                ),
                Stroke::new(2., port(n.kind.outputs()[index].port)),
            ));
            for p in waypoints {
                painter.circle_filled(p, 3., text_color());
            }
        }
        let mut clicked_input = None;
        let mut clicked_output = None;
        let mut clicked_physical = None;
        if ui.input(|i| i.pointer.primary_clicked())
            && let Some(target) = snap_target
        {
            if target.physical {
                clicked_physical = Some((target.node, target.index));
            } else {
                clicked_input = Some((target.node, target.index));
            }
        }
        let mut delta = None;
        let mut stopped = None;
        let mut hovered_node = false;
        let mut hovered_port = false;
        let mut remove = None;
        let mut debug_change = None;
        if self.keyboard_active && !crate::shortcuts::text_editing(ui.ctx()) {
            if ui.input(|i| i.key_pressed(egui::Key::Tab)) && !g.nodes.is_empty() {
                ui.input_mut(|i| {
                    i.consume_key(egui::Modifiers::NONE, egui::Key::Tab);
                    i.consume_key(egui::Modifiers::SHIFT, egui::Key::Tab);
                });
                response.request_focus();
                let current = g
                    .nodes
                    .iter()
                    .position(|n| Some(n.id) == self.selected)
                    .unwrap_or(if ui.input(|i| i.modifiers.shift) {
                        0
                    } else {
                        g.nodes.len() - 1
                    });
                let next = if ui.input(|i| i.modifiers.shift) {
                    (current + g.nodes.len() - 1) % g.nodes.len()
                } else {
                    (current + 1) % g.nodes.len()
                };
                self.select_only(g.nodes[next].id);
                self.focus_port = Some((self.wiring.is_none(), 0));
            }
            if let Some(n) = self.selected.and_then(|id| g.node(id)) {
                let ports: Vec<_> = n
                    .kind
                    .inputs()
                    .iter()
                    .enumerate()
                    .map(|(i, _)| (false, i))
                    .chain(n.kind.outputs().iter().enumerate().map(|(i, _)| (true, i)))
                    .collect();
                if !ports.is_empty()
                    && ui.input(|i| {
                        i.key_pressed(egui::Key::ArrowRight) || i.key_pressed(egui::Key::ArrowLeft)
                    })
                {
                    let old = ports
                        .iter()
                        .position(|p| Some(*p) == self.focus_port)
                        .unwrap_or(0);
                    let next = if ui.input(|i| i.key_pressed(egui::Key::ArrowLeft)) {
                        (old + ports.len() - 1) % ports.len()
                    } else {
                        (old + 1) % ports.len()
                    };
                    self.focus_port = Some(ports[next]);
                    ui.input_mut(|i| {
                        i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowLeft);
                        i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight);
                    });
                }
                if ui.input(|i| i.key_pressed(egui::Key::Enter))
                    && let Some((output, p)) = self.focus_port
                {
                    ui.input_mut(|i| {
                        i.consume_key(egui::Modifiers::NONE, egui::Key::Enter);
                    });
                    if output {
                        clicked_output = Some((n.id, p));
                    } else {
                        clicked_input = Some((n.id, p));
                    }
                }
            }
        }
        for node in &g.nodes {
            let top = self.screen(rect.min, node.position);
            let card = Rect::from_min_size(top, NODE_SIZE * self.zoom);
            if !rect.intersects(card) {
                continue;
            }
            let color = kind(node.kind);
            let selected = self.selected_ids.contains(&node.id);
            painter.rect_filled(
                card.translate(egui::vec2(3., 5.)),
                9.,
                egui::Color32::BLACK.gamma_multiply(0.22),
            );
            painter.rect_filled(card, 8., card_fill());
            painter.rect_stroke(
                card,
                8.,
                Stroke::new(
                    if selected { 2. } else { 1. },
                    if selected { color } else { border() },
                ),
                StrokeKind::Inside,
            );
            if self.active_node == Some(node.id) {
                painter.rect_stroke(
                    card.expand(5.),
                    8.,
                    Stroke::new(3., gold()),
                    StrokeKind::Outside,
                );
            }
            if node.breakpoint {
                painter.circle_filled(
                    top + egui::vec2(221., 18.) * self.zoom,
                    6. * self.zoom,
                    red(),
                );
            }
            if node.probe {
                painter.text(
                    top + egui::vec2(211., 187.) * self.zoom,
                    Align2::CENTER_CENTER,
                    "P",
                    crate::theme::font(13. * self.zoom),
                    purple(),
                );
            }
            painter.rect_filled(
                Rect::from_min_size(top, egui::vec2(NODE_SIZE.x, 4.) * self.zoom),
                2.,
                color,
            );
            painter.text(
                top + egui::vec2(13., 13.) * self.zoom,
                Align2::LEFT_TOP,
                crate::i18n::t(node.kind.tag()),
                crate::theme::font(10. * self.zoom),
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
                crate::i18n::t(&title),
                crate::theme::font(14. * self.zoom),
                text_color(),
            );
            visuals::symbol_with_ports(
                &painter,
                Rect::from_min_size(
                    top + egui::vec2(33., 55.) * self.zoom,
                    egui::vec2(174., 80.) * self.zoom,
                ),
                node.kind,
                node.config.instrument.port_count,
            );
            painter.text(
                top + egui::vec2(13., 150.) * self.zoom,
                Align2::LEFT_TOP,
                summary(node),
                crate::theme::font(11. * self.zoom),
                muted(),
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
                if done { teal() } else { muted() },
            );
            painter.text(
                top + egui::vec2(25., 185.) * self.zoom,
                Align2::LEFT_CENTER,
                crate::i18n::t(state),
                crate::theme::font(10. * self.zoom),
                if done { teal() } else { muted() },
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
                self.select(
                    node.id,
                    ui.input(|i| i.modifiers.shift || i.modifiers.command),
                );
            }
            if hit.double_clicked() {
                self.configure_request = Some(node.id);
            }
            if hit.drag_started() {
                if !self.selected_ids.contains(&node.id) {
                    self.select_only(node.id);
                }
                self.drag_before = Some(g.clone());
            }
            if hit.dragged() {
                delta = Some((node.id, ui.input(|i| i.pointer.delta()) / self.zoom));
            }
            if hit.drag_stopped() {
                stopped = Some(node.id);
            }
            if !node.comment.is_empty() {
                hit.clone().on_hover_text(&node.comment);
            }
            hit.context_menu(|ui| {
                if ui.button("Configurer le bloc…").clicked() {
                    self.configure_request = Some(node.id);
                    ui.close();
                }
                if ui.button(crate::i18n::t("Aide du bloc")).clicked() {
                    self.help_request = Some(node.kind);
                    ui.close();
                }
                if ui
                    .button(if node.breakpoint {
                        "Retirer le breakpoint"
                    } else {
                        "Ajouter un breakpoint"
                    })
                    .clicked()
                {
                    debug_change = Some((node.id, true));
                    ui.close();
                }
                if ui
                    .button(if node.probe {
                        "Retirer la sonde"
                    } else {
                        "Ajouter une sonde"
                    })
                    .clicked()
                {
                    debug_change = Some((node.id, false));
                    ui.close();
                }
                if ui.button(crate::i18n::t("Supprimer le bloc")).clicked() {
                    remove = Some(node.id);
                    ui.close();
                }
            });
            for (index, terminal) in node.physical_ports().iter().enumerate() {
                let position = top + physical_port_offset(node, index) * self.zoom;
                let color = physical_color(terminal.kind);
                painter.circle_filled(position, 5. * self.zoom.max(0.7), color);
                painter.circle_stroke(position, 5. * self.zoom.max(0.7), Stroke::new(1., bg()));
                let exit = physical_port_exit(node, index);
                let (offset, align) = if exit.y > 0. {
                    (egui::vec2(0., 9.), Align2::CENTER_TOP)
                } else if exit.x < 0. {
                    (egui::vec2(8., -8.), Align2::LEFT_BOTTOM)
                } else {
                    (egui::vec2(-8., -8.), Align2::RIGHT_BOTTOM)
                };
                painter.text(
                    position + offset * self.zoom,
                    align,
                    &terminal.name,
                    crate::theme::font(9. * self.zoom.max(0.7)),
                    color,
                );
                let hit = ui.interact(
                    Rect::from_center_size(position, egui::vec2(18., 18.)),
                    Id::new(("physical-pin", node.id, index)),
                    Sense::click(),
                );
                hovered_port |= hit.hovered();
                if hit.clicked() && self.tool == Tool::Wire && self.wiring.is_none() {
                    if self.physical_wiring.is_none()
                        || snap_target
                            .is_some_and(|s| s.physical && s.node == node.id && s.index == index)
                    {
                        clicked_physical = Some((node.id, index));
                    } else if self.physical_wiring != Some((node.id, index)) {
                        error = Some("Broche incompatible ou déjà câblée".into());
                    }
                }
                hit.on_hover_text(format!(
                    "{} · liaison physique · W pour câbler{}",
                    terminal.name,
                    if occupied_physical(g, node.id, index) {
                        " · occupée"
                    } else {
                        ""
                    }
                ));
            }
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
                    if self.selected == Some(node.id) && self.focus_port == Some((output, index)) {
                        painter.circle_stroke(
                            position,
                            12. * self.zoom,
                            Stroke::new(2., text_color()),
                        );
                    }
                    if compatible {
                        painter.circle_stroke(position, 9. * self.zoom, Stroke::new(1.5, teal()));
                    }
                    if node.physical_ports().is_empty() {
                        painter.circle_filled(position, 5. * self.zoom, port(terminal.port));
                    } else {
                        painter.rect_filled(
                            Rect::from_center_size(position, egui::vec2(8., 8.) * self.zoom),
                            1.,
                            port(terminal.port),
                        );
                    }
                    let physical_block = !node.physical_ports().is_empty();
                    painter.text(
                        position
                            + if physical_block {
                                egui::vec2(if output { 9. } else { -9. }, 0.) * self.zoom
                            } else {
                                egui::vec2(if output { -9. } else { 9. }, -9.) * self.zoom
                            },
                        if physical_block && output {
                            Align2::LEFT_CENTER
                        } else if physical_block {
                            Align2::RIGHT_CENTER
                        } else if output {
                            Align2::RIGHT_BOTTOM
                        } else {
                            Align2::LEFT_BOTTOM
                        },
                        terminal.name,
                        crate::theme::font(9. * self.zoom),
                        port(terminal.port),
                    );
                    let terminal_hit = ui.interact(
                        Rect::from_center_size(position, egui::vec2(20., 20.)),
                        Id::new(("terminal", node.id, output, index)),
                        Sense::click(),
                    );
                    hovered_port |= terminal_hit.hovered();
                    if terminal_hit.clicked()
                        && self.tool != Tool::Pan
                        && self.physical_wiring.is_none()
                    {
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
        if let Some((id, p)) = clicked_physical {
            if self.physical_wiring.is_some() {
                if let Err(e) = self.connect_physical_to(g, history, id, p) {
                    error = Some(e);
                }
            } else if !occupied_physical(g, id, p) {
                self.cancel_wire();
                self.physical_wiring = Some((id, p));
            } else {
                error = Some("Broche déjà câblée ; retirer son câble par clic droit.".into());
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
        if let Some((_, d)) = delta {
            for n in g
                .nodes
                .iter_mut()
                .filter(|n| self.selected_ids.contains(&n.id))
            {
                n.position[0] = (n.position[0] + d.x).clamp(-1e6, 1e6);
                n.position[1] = (n.position[1] + d.y).clamp(-1e6, 1e6);
            }
        }
        if stopped.is_some()
            && let Some(old) = self.drag_before.take()
        {
            if self.snap {
                for n in g
                    .nodes
                    .iter_mut()
                    .filter(|n| self.selected_ids.contains(&n.id))
                {
                    n.position = n.position.map(|v| (v / 24.).round() * 24.);
                }
            }
            if old != *g {
                history.record(old);
                self.routes_dirty = self.auto_route
                    && (g.edges.iter().any(|e| e.auto_routed)
                        || g.physical_connections.iter().any(|e| e.auto_routed));
            }
        }
        if let Some((id, breakpoint)) = debug_change {
            history.record(g.clone());
            let n = g.nodes.iter_mut().find(|n| n.id == id).unwrap();
            if breakpoint {
                n.breakpoint = !n.breakpoint;
            } else {
                n.probe = !n.probe;
            }
        }
        let mut annotation_move = None;
        let mut annotation_remove = None;
        for note in &g.annotations {
            let top = self.screen(rect.min, note.position);
            let card = Rect::from_min_size(top, egui::vec2(240., 84.) * self.zoom);
            if !rect.intersects(card) {
                continue;
            }
            painter.rect_filled(card, 5., gold().gamma_multiply(0.18));
            painter.rect_stroke(card, 5., Stroke::new(1., gold()), StrokeKind::Inside);
            let short = note.text.chars().take(95).collect::<String>();
            painter.text(
                card.min + egui::vec2(10., 10.) * self.zoom,
                Align2::LEFT_TOP,
                short,
                crate::theme::font(12. * self.zoom),
                text_color(),
            );
            let hit = ui.interact(
                card,
                Id::new(("annotation", note.id)),
                Sense::click_and_drag(),
            );
            hovered_node |= hit.hovered();
            if hit.clicked() {
                self.annotation = Some(note.id);
                self.selected = None;
                self.selected_ids.clear();
            }
            if hit.drag_started() {
                self.drag_before = Some(g.clone());
            }
            if hit.dragged() {
                annotation_move = Some((note.id, ui.input(|i| i.pointer.delta()) / self.zoom));
            }
            if hit.drag_stopped()
                && let Some(old) = self.drag_before.take()
            {
                history.record(old);
            }
            hit.context_menu(|ui| {
                if ui
                    .button(crate::i18n::t("Supprimer l'annotation"))
                    .clicked()
                {
                    annotation_remove = Some(note.id);
                    ui.close();
                }
            });
        }
        if let Some((id, d)) = annotation_move
            && let Some(n) = g.annotations.iter_mut().find(|n| n.id == id)
        {
            n.position = [
                (n.position[0] + d.x).clamp(-1e6, 1e6),
                (n.position[1] + d.y).clamp(-1e6, 1e6),
            ];
        }
        if let Some(id) = annotation_remove {
            history.record(g.clone());
            g.annotations.retain(|a| a.id != id);
            self.annotation = None;
        }
        if let Some(id) = remove {
            history.record(g.clone());
            g.remove(id);
            self.selected = None;
            self.cancel_wire();
        }
        if response.drag_started()
            && self.tool == Tool::Select
            && !hovered_node
            && !hovered_port
            && !self.is_wiring()
        {
            self.marquee = ui.input(|i| i.pointer.press_origin());
        }
        if let Some(start) = self.marquee
            && let Some(end) = ui.input(|i| i.pointer.interact_pos())
        {
            let selection = Rect::from_two_pos(start, end).intersect(rect);
            painter.rect_filled(selection, 0., blue().gamma_multiply(0.08));
            painter.rect_stroke(selection, 0., Stroke::new(1., blue()), StrokeKind::Inside);
            if response.drag_stopped() {
                if !ui.input(|i| i.modifiers.shift || i.modifiers.command) {
                    self.selected_ids.clear();
                }
                for n in &g.nodes {
                    if selection.intersects(Rect::from_min_size(
                        self.screen(rect.min, n.position),
                        NODE_SIZE * self.zoom,
                    )) {
                        self.selected_ids.insert(n.id);
                    }
                }
                self.selected = self.selected_ids.iter().next().copied();
                self.marquee = None;
            }
        }
        if response.dragged_by(egui::PointerButton::Middle)
            || (response.dragged() && self.tool == Tool::Pan && !hovered_port)
        {
            self.auto_fit = false;
            self.pan += ui.input(|i| i.pointer.delta());
        }
        if response.clicked()
            && ui.input(|i| i.pointer.primary_clicked())
            && self.tool == Tool::Select
            && !hovered_node
            && !hovered_port
            && !cable_hover
            && !ui.input(|i| i.modifiers.shift)
        {
            self.selected = None;
            self.selected_ids.clear();
            self.annotation = None;
        }
        if self.tool == Tool::Wire
            && self.is_wiring()
            && response.clicked()
            && ui.input(|i| i.pointer.primary_clicked())
            && !hovered_node
            && !hovered_port
            && !cable_hover
            && snap_target.is_none()
            && let Some(mouse) = response.interact_pointer_pos()
            && self.waypoints.len() < 64
        {
            let p = (mouse - rect.min - self.pan) / self.zoom;
            self.waypoints.push([p.x, p.y]);
        }
        if response.secondary_clicked()
            || (!crate::shortcuts::text_editing(ui.ctx())
                && ui.input(|i| i.key_pressed(egui::Key::Escape)))
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
        if self.tool == Tool::Wire {
            painter.text(
                rect.left_bottom() + egui::vec2(12., -10.),
                Align2::LEFT_BOTTOM,
                "WIRE · Broche → broche · Aimantation légère · Clic fond : coude · Échap : annuler",
                crate::theme::font(10.),
                muted(),
            );
        }
        error
    }
}
fn summary(n: &Node) -> String {
    let c = &n.config;
    match n.kind {
        Kind::Dsp(_) => format!("{:.1} kS/s · {} pts", c.dsp.rate / 1000., c.dsp.samples),
        Kind::Generator => format!("{:.3} GHz / {:.1} dBm", c.frequency_hz / 1e9, c.power_dbm),
        Kind::Dut => format!(
            "Gain {:.1} dB · NF {:.1} dB",
            -c.loss_db - c.attenuation_db,
            c.noise_figure_db
        ),
        Kind::Analyzer => format!(
            "{:.2}–{:.2} GHz · {} pts",
            c.start_hz / 1e9,
            c.stop_hz / 1e9,
            c.points
        ),
        Kind::Pna | Kind::PnaX | Kind::UsbVna => format!(
            "{} · {:.2}–{:.2} GHz",
            c.s_parameter,
            c.start_hz / 1e9,
            c.stop_hz / 1e9
        ),
        Kind::DcSupplyE3631A | Kind::DcSupplyE36313A => format!(
            "CH{} · {:.2} V · limite {:.3} A",
            c.instrument.dc.channel, c.instrument.dc.voltage_v, c.instrument.dc.current_limit_a
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
pub(crate) fn port_offset(n: &Node, output: bool, index: usize) -> Vec2 {
    let count = if output {
        n.kind.outputs().len()
    } else {
        n.kind.inputs().len()
    };
    egui::vec2(
        if output { NODE_SIZE.x } else { 0. },
        if n.physical_ports().is_empty() {
            65. + (index + 1) as f32 / (count + 1) as f32 * 105.
        } else {
            160. + index as f32 * 22.
        },
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
    fn physical_frame(
        ctx: &egui::Context,
        canvas: &mut Canvas,
        g: &mut Graph,
        h: &mut History,
        events: Vec<egui::Event>,
    ) -> (Pos2, egui::CursorIcon) {
        let mut origin = Pos2::ZERO;
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(900., 600.))),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    origin = ui.available_rect_before_wrap().min;
                    assert!(canvas.show(ui, g, h, 450.).is_none());
                });
            },
        );
        (origin, output.platform_output.cursor_icon)
    }
    fn physical_click(
        ctx: &egui::Context,
        c: &mut Canvas,
        g: &mut Graph,
        h: &mut History,
        pos: Pos2,
    ) {
        for pressed in [true, false] {
            physical_frame(
                ctx,
                c,
                g,
                h,
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
        }
    }
    #[test]
    fn physical_wire_magnet_preview_click_and_undo_use_real_pointer_events() {
        let ctx = egui::Context::default();
        let mut g = Graph::default();
        let v = g.add(Kind::PnaX, [60., 50.]);
        let d = g.add(Kind::Dut, [450., 50.]);
        let mut c = Canvas {
            fit: false,
            auto_fit: false,
            pan: Vec2::ZERO,
            tool: Tool::Wire,
            selected: None,
            selected_ids: BTreeSet::new(),
            ..Default::default()
        };
        let mut h = History::default();
        let origin = physical_frame(&ctx, &mut c, &mut g, &mut h, vec![]).0;
        physical_frame(&ctx, &mut c, &mut g, &mut h, vec![]);
        let from = origin
            + Vec2::from(g.node(v).unwrap().position)
            + physical_port_offset(g.node(v).unwrap(), 0);
        physical_click(&ctx, &mut c, &mut g, &mut h, from);
        assert_eq!(c.physical_wiring, Some((v, 0)));
        let to = origin
            + Vec2::from(g.node(d).unwrap().position)
            + physical_port_offset(g.node(d).unwrap(), 0);
        let near = to + egui::vec2(-8., 0.);
        let (_, cursor) = physical_frame(
            &ctx,
            &mut c,
            &mut g,
            &mut h,
            vec![egui::Event::PointerMoved(near)],
        );
        assert!(g.physical_connections.is_empty());
        assert_eq!(cursor, egui::CursorIcon::Crosshair);
        assert_eq!(c.nearest_target(&g, origin, near).unwrap().node, d);
        physical_click(&ctx, &mut c, &mut g, &mut h, near);
        assert_eq!(g.physical_connections.len(), 1);
        assert!(!c.is_wiring());
        assert!(c.tool == Tool::Wire);
        h.undo(&mut g);
        assert!(g.physical_connections.is_empty());
        h.redo(&mut g);
        assert_eq!(g.physical_connections.len(), 1);
        c.physical_wiring = Some((v, 1));
        assert!(c.nearest_target(&g, origin, to).is_none()); // Occupied RF input.
        let supply = g.add(Kind::DcSupplyE3631A, [300., 300.]);
        let dc = origin
            + Vec2::from(g.node(supply).unwrap().position)
            + physical_port_offset(g.node(supply).unwrap(), 0);
        assert!(c.nearest_target(&g, origin, dc).is_none()); // RF never snaps to DC.
        let out = origin
            + Vec2::from(g.node(d).unwrap().position)
            + physical_port_offset(g.node(d).unwrap(), 1);
        assert!(
            c.nearest_target(&g, origin, out + egui::vec2(12., 0.))
                .is_none()
        );
        c.zoom = 0.5;
        let out = origin
            + Vec2::from(g.node(d).unwrap().position) * c.zoom
            + physical_port_offset(g.node(d).unwrap(), 1) * c.zoom;
        assert!(
            c.nearest_target(&g, origin, out + egui::vec2(6., 0.))
                .is_some()
        );
    }
    #[test]
    fn keyboard_navigation_connects_named_ports_with_canvas_focus() {
        let ctx = egui::Context::default();
        let mut graph = Graph::default();
        let a = graph.add(Kind::Generator, [0., 0.]);
        let b = graph.add(Kind::Analyzer, [420., 0.]);
        let mut canvas = Canvas {
            selected: None,
            selected_ids: BTreeSet::new(),
            ..Default::default()
        };
        let mut history = History::default();
        let mut draw = |key: Option<egui::Key>| {
            let events = key
                .into_iter()
                .flat_map(|key| {
                    [true, false].map(|pressed| egui::Event::Key {
                        key,
                        physical_key: Some(key),
                        pressed,
                        repeat: false,
                        modifiers: egui::Modifiers::NONE,
                    })
                })
                .collect();
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(900., 600.))),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        let _ = ui.button("Toolbar button");
                        let error = canvas.show(ui, &mut graph, &mut history, 450.);
                        assert!(error.is_none(), "{error:?}");
                    });
                },
            );
        };
        draw(None);
        draw(Some(egui::Key::Tab));
        draw(Some(egui::Key::Enter));
        draw(Some(egui::Key::Tab));
        draw(Some(egui::Key::Enter));
        assert_eq!(graph.edges.len(), 1);
        assert_eq!((graph.edges[0].from, graph.edges[0].to), (a, b));
        assert!(canvas.wiring.is_none());
        history.undo(&mut graph);
        assert!(graph.edges.is_empty());
    }
    #[test]
    fn history_keeps_edits_beyond_the_old_hundred_step_limit() {
        let mut g = Graph::default();
        g.add(Kind::Generator, [0., 0.]);
        let original = g.clone();
        let mut h = History::default();
        for i in 1..=150 {
            h.record(g.clone());
            g.nodes[0].config.power_dbm = -(i as f64);
        }
        for _ in 0..150 {
            h.undo(&mut g);
        }
        assert_eq!(g, original);
        for _ in 0..150 {
            h.redo(&mut g);
        }
        assert_eq!(g.nodes[0].config.power_dbm, -150.);
    }
    #[test]
    fn shift_click_and_group_drag_use_real_hit_regions_and_one_undo() {
        let ctx = egui::Context::default();
        let mut g = Graph::default();
        let a = g.add(Kind::Generator, [0., 0.]);
        let b = g.add(Kind::Generator, [330., 0.]);
        let original = g.clone();
        let mut canvas = Canvas {
            fit: false,
            auto_fit: false,
            pan: Vec2::ZERO,
            snap: false,
            ..Default::default()
        };
        let mut h = History::default();
        let mut draw = |events: Vec<egui::Event>, modifiers: egui::Modifiers| {
            let mut origin = Pos2::ZERO;
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(900., 600.))),
                    events,
                    modifiers,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        origin = ui.available_rect_before_wrap().min;
                        canvas.show(ui, &mut g, &mut h, 450.);
                    });
                },
            );
            origin
        };
        let origin = draw(vec![], egui::Modifiers::NONE);
        draw(vec![], egui::Modifiers::NONE);
        for (pos, modifiers) in [
            (origin + egui::vec2(100., 80.), egui::Modifiers::NONE),
            (origin + egui::vec2(430., 80.), egui::Modifiers::SHIFT),
        ] {
            for pressed in [true, false] {
                draw(
                    vec![
                        egui::Event::PointerMoved(pos),
                        egui::Event::PointerButton {
                            pos,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers,
                        },
                    ],
                    modifiers,
                );
            }
        }
        let start = origin + egui::vec2(100., 80.);
        let end = start + egui::vec2(40., 20.);
        draw(
            vec![
                egui::Event::PointerMoved(start),
                egui::Event::PointerButton {
                    pos: start,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            egui::Modifiers::NONE,
        );
        draw(vec![egui::Event::PointerMoved(end)], egui::Modifiers::NONE);
        draw(
            vec![egui::Event::PointerButton {
                pos: end,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
            egui::Modifiers::NONE,
        );
        assert_eq!(canvas.selected_ids, [a, b].into_iter().collect());
        assert_eq!(g.nodes[0].position, [40., 20.]);
        assert_eq!(g.nodes[1].position, [370., 20.]);
        h.undo(&mut g);
        assert_eq!(g, original);
    }
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
