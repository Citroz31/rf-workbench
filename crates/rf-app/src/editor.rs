//! Reusable editing operations and bounded obstacle-aware Manhattan routing.
use crate::canvas::{NODE_SIZE, physical_port_exit, physical_port_offset, port_offset};
use eframe::egui::{Pos2, Rect};
use rf_core::{Graph, Kind};
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap};

pub fn fuzzy_score(query: &str, text: &str) -> Option<usize> {
    let normalize = |s: &str| {
        s.to_lowercase()
            .chars()
            .map(|c| match c {
                'é' | 'è' | 'ê' | 'ë' => 'e',
                'à' | 'â' => 'a',
                'î' | 'ï' => 'i',
                'ô' => 'o',
                'û' | 'ù' => 'u',
                'ç' => 'c',
                _ => c,
            })
            .collect::<String>()
    };
    let query = normalize(query);
    let text = normalize(text);
    if query.is_empty() {
        return Some(0);
    }
    if let Some(i) = text.find(&query) {
        return Some(i);
    }
    let mut score = 20;
    let mut cursor = 0;
    for c in query.chars() {
        let offset = text[cursor..].find(c)?;
        score += offset;
        cursor += offset + c.len_utf8();
    }
    Some(score)
}
pub fn search(query: &str) -> Vec<Kind> {
    let mut hits: Vec<_> = Kind::ALL
        .into_iter()
        .filter_map(|k| {
            let aliases = match k {
                Kind::Adc => "ADC analog digital converter",
                Kind::Dac => "digital analog converter",
                Kind::Analyzer => "spectrum analyzer",
                Kind::Generator => "RF signal generator",
                Kind::VariableResistor => "variable resistance resistor",
                Kind::Thermometer => "temperature thermometer",
                Kind::IqModulator => "IQ quadrature mixer modulator",
                Kind::NoiseFigureMeter => "NFM bruit noise figure",
                Kind::Dut => "device under test chip",
                _ => "",
            };
            fuzzy_score(query, &format!("{} {} {aliases}", k.label(), k.category()))
                .map(|score| (score, k))
        })
        .collect();
    hits.sort_by_key(|(score, _)| *score);
    hits.into_iter().map(|(_, k)| k).collect()
}
pub fn selection(g: &Graph, ids: &BTreeSet<u64>) -> Graph {
    Graph {
        nodes: g
            .nodes
            .iter()
            .filter(|n| ids.contains(&n.id))
            .cloned()
            .collect(),
        edges: g
            .edges
            .iter()
            .filter(|e| ids.contains(&e.from) && ids.contains(&e.to))
            .cloned()
            .collect(),
        physical_connections: g
            .physical_connections
            .iter()
            .filter(|e| ids.contains(&e.from) && ids.contains(&e.to))
            .cloned()
            .collect(),
        annotations: Vec::new(),
    }
}
pub fn paste(g: &mut Graph, clip: &Graph, offset: [f32; 2]) -> Result<BTreeSet<u64>, String> {
    if clip.nodes.is_empty() || g.nodes.len() + clip.nodes.len() > 1000 {
        return Err("Copie vide ou limite de 1000 blocs dépassée".into());
    }
    clip.order().map_err(|e| e.to_string())?;
    if clip.nodes.iter().any(|n| {
        n.comment.len() > 8192 || n.position.iter().any(|v| !v.is_finite() || v.abs() > 1e6)
    }) {
        return Err("Données de copie invalides".into());
    }
    let mut next = g.clone();
    let mut ids = BTreeMap::new();
    for old in &clip.nodes {
        let id = next.add(old.kind, old.position.map(|v| v));
        let n = next.nodes.last_mut().unwrap();
        *n = old.clone();
        n.id = id;
        n.position = [
            (old.position[0] + offset[0]).clamp(-1e6, 1e6),
            (old.position[1] + offset[1]).clamp(-1e6, 1e6),
        ];
        ids.insert(old.id, id);
    }
    for old in &clip.edges {
        let mut e = old.clone();
        e.from = ids[&old.from];
        e.to = ids[&old.to];
        for p in &mut e.waypoints {
            p[0] += offset[0];
            p[1] += offset[1];
        }
        next.edges.push(e);
    }
    for old in &clip.physical_connections {
        let mut e = old.clone();
        e.from = *ids.get(&old.from).ok_or("Source copiée absente")?;
        e.to = *ids.get(&old.to).ok_or("Destination copiée absente")?;
        for p in &mut e.waypoints {
            p[0] += offset[0];
            p[1] += offset[1];
        }
        next.physical_connections.push(e);
    }
    next.order().map_err(|e| e.to_string())?;
    next.validate_physical().map_err(|e| e.to_string())?;
    *g = next;
    Ok(ids.into_values().collect())
}
#[derive(Clone, Copy)]
pub enum Alignment {
    Left,
    Top,
    Horizontal,
    Vertical,
}
pub fn align(g: &mut Graph, ids: &BTreeSet<u64>, mode: Alignment) {
    let mut selected: Vec<_> = g
        .nodes
        .iter()
        .filter(|n| ids.contains(&n.id))
        .map(|n| (n.id, n.position))
        .collect();
    if selected.len() < 2 {
        return;
    }
    selected.sort_by(|a, b| {
        a.1[match mode {
            Alignment::Vertical | Alignment::Top => 1,
            _ => 0,
        }]
        .total_cmp(
            &b.1[match mode {
                Alignment::Vertical | Alignment::Top => 1,
                _ => 0,
            }],
        )
    });
    let left = selected
        .iter()
        .map(|(_, p)| p[0])
        .fold(f32::INFINITY, f32::min);
    let top = selected
        .iter()
        .map(|(_, p)| p[1])
        .fold(f32::INFINITY, f32::min);
    for (i, (id, _)) in selected.iter().enumerate() {
        let n = g.nodes.iter_mut().find(|n| n.id == *id).unwrap();
        match mode {
            Alignment::Left => n.position[0] = left,
            Alignment::Top => n.position[1] = top,
            Alignment::Horizontal => n.position = [left + i as f32 * 312., top],
            Alignment::Vertical => n.position = [left, top + i as f32 * 264.],
        }
    }
}
pub fn auto_layout(g: &mut Graph) -> Result<(), String> {
    let order = g.order().map_err(|e| e.to_string())?;
    let mut depths = BTreeMap::new();
    let mut rows = BTreeMap::<usize, usize>::new();
    for id in order {
        let depth = g
            .edges
            .iter()
            .filter(|e| e.to == id)
            .map(|e| depths[&e.from] + 1)
            .max()
            .unwrap_or(0);
        depths.insert(id, depth);
        let row = rows.entry(depth).or_default();
        g.nodes.iter_mut().find(|n| n.id == id).unwrap().position =
            [36. + depth as f32 * 312., 40. + *row as f32 * 264.];
        *row += 1;
    }
    Ok(())
}
fn intersects(a: Pos2, b: Pos2, r: Rect) -> bool {
    if (a.y - b.y).abs() < 0.01 {
        a.y > r.top() && a.y < r.bottom() && a.x.min(b.x) < r.right() && a.x.max(b.x) > r.left()
    } else {
        a.x > r.left() && a.x < r.right() && a.y.min(b.y) < r.bottom() && a.y.max(b.y) > r.top()
    }
}
pub fn route_edge(g: &Graph, index: usize) -> Result<Vec<[f32; 2]>, String> {
    let e = &g.edges[index];
    let a = g.node(e.from).ok_or("Source absente")?;
    let b = g.node(e.to).ok_or("Destination absente")?;
    let from = Pos2::from(a.position) + port_offset(a, true, e.from_port);
    let to = Pos2::from(b.position) + port_offset(b, false, e.to_port);
    route_terminals(
        g,
        a.id,
        b.id,
        from,
        to,
        eframe::egui::vec2(24., 0.),
        eframe::egui::vec2(-24., 0.),
    )
}
pub fn route_physical(g: &Graph, index: usize) -> Result<Vec<[f32; 2]>, String> {
    let e = &g.physical_connections[index];
    let a = g.node(e.from).ok_or("Source absente")?;
    let b = g.node(e.to).ok_or("Destination absente")?;
    let from = Pos2::from(a.position) + physical_port_offset(a, e.from_port);
    let to = Pos2::from(b.position) + physical_port_offset(b, e.to_port);
    route_terminals(
        g,
        a.id,
        b.id,
        from,
        to,
        physical_port_exit(a, e.from_port) * 24.,
        physical_port_exit(b, e.to_port) * 24.,
    )
}
fn route_terminals(
    g: &Graph,
    source_id: u64,
    target_id: u64,
    from: Pos2,
    to: Pos2,
    exit_a: eframe::egui::Vec2,
    exit_b: eframe::egui::Vec2,
) -> Result<Vec<[f32; 2]>, String> {
    let obstacles: Vec<_> = g
        .nodes
        .iter()
        .map(|n| Rect::from_min_size(Pos2::from(n.position), NODE_SIZE).expand(14.))
        .collect();
    let start = from + exit_a;
    let end = to + exit_b;
    if g.nodes.iter().zip(&obstacles).any(|(n, r)| {
        (n.id != source_id && intersects(from, start, *r))
            || (n.id != target_id && intersects(end, to, *r))
    }) {
        return Err("Connecteur obstrué : éloigner les blocs".into());
    }
    let region = Rect::from_two_pos(start, end).expand(320.);
    let local: Vec<_> = obstacles
        .iter()
        .copied()
        .filter(|r| r.intersects(region))
        .collect();
    if local.len() > 96 {
        return Err("Zone trop dense : placer des coudes manuellement".into());
    }
    let mut xs = vec![start.x, end.x, region.left(), region.right()];
    let mut ys = vec![start.y, end.y, region.top(), region.bottom()];
    for r in &local {
        xs.extend([
            r.left().clamp(region.left(), region.right()),
            r.right().clamp(region.left(), region.right()),
        ]);
        ys.extend([
            r.top().clamp(region.top(), region.bottom()),
            r.bottom().clamp(region.top(), region.bottom()),
        ]);
    }
    xs.sort_by(f32::total_cmp);
    xs.dedup();
    ys.sort_by(f32::total_cmp);
    ys.dedup();
    let nx = xs.len();
    let cell = |x: usize, y: usize| y * nx + x;
    let pos = |i: usize| Pos2::new(xs[i % nx], ys[i / nx]);
    let source = cell(
        xs.iter().position(|x| *x == start.x).unwrap(),
        ys.iter().position(|y| *y == start.y).unwrap(),
    );
    let target = cell(
        xs.iter().position(|x| *x == end.x).unwrap(),
        ys.iter().position(|y| *y == end.y).unwrap(),
    );
    let count = nx * ys.len();
    let blocked: Vec<_> = (0..count)
        .map(|i| {
            local.iter().any(|r| {
                let p = pos(i);
                p.x > r.left() && p.x < r.right() && p.y > r.top() && p.y < r.bottom()
            })
        })
        .collect();
    if blocked[source] || blocked[target] {
        return Err("Connecteurs trop proches d'un autre bloc".into());
    }
    let mut distances = vec![u64::MAX; count * 3];
    let mut parents = vec![usize::MAX; count * 3];
    let mut queue = BinaryHeap::new();
    distances[source * 3] = 0;
    queue.push((Reverse(0_u64), source * 3));
    let mut finish = None;
    while let Some((Reverse(cost), state)) = queue.pop() {
        if cost != distances[state] {
            continue;
        }
        let i = state / 3;
        if i == target {
            finish = Some(state);
            break;
        }
        let (x, y) = (i % nx, i / nx);
        let neighbours = [
            x.checked_sub(1).map(|x| (cell(x, y), 1)),
            (x + 1 < nx).then_some((cell(x + 1, y), 1)),
            y.checked_sub(1).map(|y| (cell(x, y), 2)),
            (y + 1 < ys.len()).then_some((cell(x, y + 1), 2)),
        ];
        for (j, direction) in neighbours.into_iter().flatten() {
            if blocked[j] || local.iter().any(|r| intersects(pos(i), pos(j), *r)) {
                continue;
            }
            let turn = if state % 3 != 0 && state % 3 != direction {
                1200
            } else {
                0
            };
            let next_cost = cost + (pos(i).distance(pos(j)) * 100.).round() as u64 + turn;
            let next = j * 3 + direction;
            if next_cost < distances[next] {
                distances[next] = next_cost;
                parents[next] = state;
                queue.push((Reverse(next_cost), next));
            }
        }
    }
    let mut state =
        finish.ok_or("Aucun parcours libre : déplacer les blocs ou utiliser des coudes")?;
    let mut path = vec![to];
    loop {
        path.push(pos(state / 3));
        if state == source * 3 {
            break;
        }
        state = parents[state];
    }
    path.push(from);
    path.reverse();
    let mut compact = vec![path[0]];
    for i in 1..path.len() - 1 {
        let a = *compact.last().unwrap();
        let b = path[i];
        let c = path[i + 1];
        if (a.x - b.x).abs() < 0.01 && (b.x - c.x).abs() < 0.01
            || (a.y - b.y).abs() < 0.01 && (b.y - c.y).abs() < 0.01
        {
            continue;
        }
        compact.push(b);
    }
    compact.push(to);
    if compact.len() > 66 {
        return Err("Parcours trop complexe".into());
    }
    Ok(compact[1..compact.len() - 1]
        .iter()
        .map(|p| [p.x, p.y])
        .collect())
}

pub struct RoutingReport {
    pub graph: Graph,
    pub failed: usize,
}
pub struct RoutingJob {
    pub before: Graph,
    pub record: bool,
    pub receiver: std::sync::mpsc::Receiver<Result<RoutingReport, String>>,
}
impl RoutingJob {
    pub fn spawn(
        graph: &Graph,
        organize: bool,
        only_auto: bool,
        record: bool,
        wake: impl FnOnce() + Send + 'static,
    ) -> Self {
        let before = graph.clone();
        let mut working = graph.clone();
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        std::thread::spawn(move || {
            let result = (|| {
                working.order().map_err(|e| e.to_string())?;
                if organize {
                    auto_layout(&mut working)?;
                }
                let mut failed = 0;
                for i in 0..working.edges.len() {
                    if only_auto && !working.edges[i].auto_routed {
                        continue;
                    }
                    match route_edge(&working, i) {
                        Ok(points) => {
                            working.edges[i].waypoints = points;
                            working.edges[i].auto_routed = true;
                        }
                        Err(_) => failed += 1,
                    }
                }
                for i in 0..working.physical_connections.len() {
                    if only_auto && !working.physical_connections[i].auto_routed {
                        continue;
                    }
                    match route_physical(&working, i) {
                        Ok(points) => {
                            working.physical_connections[i].waypoints = points;
                            working.physical_connections[i].auto_routed = true;
                        }
                        Err(_) => failed += 1,
                    }
                }
                Ok(RoutingReport {
                    graph: working,
                    failed,
                })
            })();
            let _ = sender.send(result);
            wake();
        });
        Self {
            before,
            record,
            receiver,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn physical_loop_copy_paste_and_routing_preserve_endpoints() {
        let mut g = Graph::default();
        let v = g.add(Kind::PnaX, [0., 0.]);
        let d = g.add(Kind::Dut, [420., 0.]);
        g.connect_physical(v, 0, d, 0).unwrap();
        g.connect_physical(d, 1, v, 1).unwrap();
        let clip = selection(&g, &[v, d].into_iter().collect());
        let ids = paste(&mut g, &clip, [0., 300.]).unwrap();
        assert_eq!(ids.len(), 2);
        assert_eq!(g.physical_connections.len(), 4);
        g.validate().unwrap();
        for e in &g.physical_connections[2..] {
            assert!(ids.contains(&e.from) && ids.contains(&e.to));
        }
        let path = route_physical(&g, 0).unwrap();
        assert!(!path.is_empty());
        let a = g.node(v).unwrap();
        let b = g.node(d).unwrap();
        let points: Vec<_> = std::iter::once(Pos2::from(a.position) + physical_port_offset(a, 0))
            .chain(path.iter().map(|p| Pos2::from(*p)))
            .chain(std::iter::once(
                Pos2::from(b.position) + physical_port_offset(b, 0),
            ))
            .collect();
        for pair in points.windows(2) {
            assert!(pair[0].x == pair[1].x || pair[0].y == pair[1].y);
        }
    }
    #[test]
    fn background_routing_keeps_original_and_manual_routes() {
        let mut graph = Graph::default();
        let a = graph.add(Kind::Generator, [0., 0.]);
        let b = graph.add(Kind::Analyzer, [624., 0.]);
        graph.connect(a, b).unwrap();
        graph.edges[0].waypoints = vec![[350., 300.]];
        let original = graph.clone();
        let job = RoutingJob::spawn(&graph, false, true, false, || {});
        let result = job
            .receiver
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap()
            .unwrap();
        assert_eq!(result.graph, original);
        assert_eq!(job.before, original);
        assert_eq!(graph, original);
        let job = RoutingJob::spawn(&graph, true, false, true, || {});
        let result = job
            .receiver
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap()
            .unwrap();
        assert_eq!(result.failed, 0);
        assert!(result.graph.edges[0].auto_routed);
        assert_ne!(result.graph.nodes[1].position, original.nodes[1].position);
        result.graph.order().unwrap();
    }
    #[test]
    fn fuzzy_matches_aliases_and_accents() {
        assert!(search("spectr").contains(&Kind::Analyzer));
        assert!(search("adc").contains(&Kind::Adc));
        assert!(fuzzy_score("resvar", "Résistance variable").is_some());
        assert!(search("zxqj").is_empty());
    }
    #[test]
    fn paste_preserves_inner_wires_and_assigns_new_ids() {
        let mut g = Graph::iq_demo();
        let ids = g.nodes.iter().map(|n| n.id).collect();
        let clip = selection(&g, &ids);
        let new = paste(&mut g, &clip, [36., 36.]).unwrap();
        assert_eq!(new.len(), clip.nodes.len());
        assert_eq!(g.edges.len(), 2 * clip.edges.len());
        assert!(g.order().is_ok());
        assert!(
            g.edges
                .iter()
                .filter(|e| new.contains(&e.from))
                .all(|e| new.contains(&e.to))
        );
    }
    #[test]
    fn routing_avoids_intermediate_block() {
        let mut g = Graph::default();
        let a = g.add(Kind::Generator, [0., 0.]);
        g.add(Kind::VariableResistor, [312., 0.]);
        let b = g.add(Kind::Analyzer, [624., 0.]);
        g.connect(a, b).unwrap();
        let route = route_edge(&g, 0).unwrap();
        let obstacle = Rect::from_min_size(Pos2::new(312., 0.), NODE_SIZE).expand(14.);
        let mut points = vec![Pos2::new(240., 117.5)];
        points.extend(route.into_iter().map(Pos2::from));
        points.push(Pos2::new(624., 117.5));
        assert!(points.windows(2).all(|w| !intersects(w[0], w[1], obstacle)));
    }
}
