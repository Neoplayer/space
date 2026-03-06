use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::{GalaxyGenConfig, GateId, RoutingGraphView, StationId, SystemId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum StationProfile {
    Civilian,
    Industrial,
    Research,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SystemNode {
    pub id: SystemId,
    pub x: f64,
    pub y: f64,
    pub radius: f64,
    pub gate_nodes: Vec<GateNode>,
    pub dock_capacity: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GateNode {
    pub gate_id: GateId,
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GateEdge {
    pub id: GateId,
    pub a: SystemId,
    pub b: SystemId,
    pub base_capacity: f64,
    pub travel_ticks: u32,
    pub blocked_until_tick: u64,
    pub capacity_factor: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct StationAnchor {
    pub id: StationId,
    pub system_id: SystemId,
    pub profile: StationProfile,
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct World {
    pub systems: Vec<SystemNode>,
    pub edges: Vec<GateEdge>,
    pub adjacency: BTreeMap<SystemId, Vec<(SystemId, GateId)>>,
    pub stations: Vec<StationAnchor>,
    pub stations_by_system: BTreeMap<SystemId, Vec<StationId>>,
}

impl World {
    pub fn generate(cfg: &GalaxyGenConfig, seed: u64) -> Self {
        let mut rng = DeterministicRng::new(seed);
        let count_span = u64::from(
            cfg.cluster_system_max
                .saturating_sub(cfg.cluster_system_min),
        ) + 1_u64;
        let system_count = usize::from(cfg.cluster_system_min)
            + usize::try_from(rng.next_u64() % count_span).unwrap_or(0);

        let mut systems = Vec::with_capacity(system_count);
        for idx in 0..system_count {
            let ang = (idx as f64 / system_count as f64) * std::f64::consts::TAU;
            let wobble = rng.next_f64() * 8.0;
            systems.push(SystemNode {
                id: SystemId(idx),
                x: (cfg.system_radius * 2.5 + wobble) * ang.cos(),
                y: (cfg.system_radius * 2.5 + wobble) * ang.sin(),
                radius: cfg.system_radius,
                gate_nodes: Vec::new(),
                dock_capacity: 4.0,
            });
        }

        let mut edges = Vec::<GateEdge>::new();
        let mut edge_set = BTreeSet::<(usize, usize)>::new();

        for idx in 0..system_count.saturating_sub(1) {
            let a = idx;
            let b = idx + 1;
            edge_set.insert((a, b));
        }

        let min_degree = usize::from(cfg.min_degree.max(1));
        let max_degree = usize::from(cfg.max_degree.max(cfg.min_degree.max(1)));

        let mut degree = vec![0_usize; system_count];
        for (a, b) in &edge_set {
            degree[*a] += 1;
            degree[*b] += 1;
        }

        if system_count > 2 {
            let a = 0;
            let b = system_count - 1;
            if !edge_set.contains(&(a, b)) && degree[a] < max_degree && degree[b] < max_degree {
                edge_set.insert((a, b));
                degree[a] += 1;
                degree[b] += 1;
            }
        }

        for idx in 0..system_count {
            let mut attempts = 0;
            while degree[idx] < min_degree && attempts < system_count * 4 {
                attempts += 1;
                let j = usize::try_from(rng.next_u64() % u64::try_from(system_count).unwrap_or(1))
                    .unwrap_or(0);
                if idx == j {
                    continue;
                }
                let (a, b) = if idx < j { (idx, j) } else { (j, idx) };
                if edge_set.contains(&(a, b)) {
                    continue;
                }
                if degree[a] >= max_degree || degree[b] >= max_degree {
                    continue;
                }
                edge_set.insert((a, b));
                degree[a] += 1;
                degree[b] += 1;
            }
        }

        for (edge_idx, (a, b)) in edge_set.iter().copied().enumerate() {
            edges.push(GateEdge {
                id: GateId(edge_idx),
                a: SystemId(a),
                b: SystemId(b),
                base_capacity: cfg.base_gate_capacity,
                travel_ticks: cfg.base_gate_travel_ticks,
                blocked_until_tick: 0,
                capacity_factor: 1.0,
            });
        }

        let mut adjacency: BTreeMap<SystemId, Vec<(SystemId, GateId)>> = BTreeMap::new();
        for edge in &edges {
            adjacency.entry(edge.a).or_default().push((edge.b, edge.id));
            adjacency.entry(edge.b).or_default().push((edge.a, edge.id));

            let a_idx = edge.a.0;
            let b_idx = edge.b.0;
            let (sx, sy) = (systems[a_idx].x, systems[a_idx].y);
            let (tx, ty) = (systems[b_idx].x, systems[b_idx].y);
            let a_radius = systems[a_idx].radius;
            let b_radius = systems[b_idx].radius;
            let dx = tx - sx;
            let dy = ty - sy;
            let dist = (dx * dx + dy * dy).sqrt().max(1.0);
            let ux = dx / dist;
            let uy = dy / dist;

            systems[a_idx].gate_nodes.push(GateNode {
                gate_id: edge.id,
                x: sx + ux * a_radius,
                y: sy + uy * a_radius,
            });
            systems[b_idx].gate_nodes.push(GateNode {
                gate_id: edge.id,
                x: tx - ux * b_radius,
                y: ty - uy * b_radius,
            });
        }

        let mut stations = Vec::with_capacity(system_count.saturating_mul(2));
        let mut stations_by_system: BTreeMap<SystemId, Vec<StationId>> = BTreeMap::new();
        for system in &systems {
            let mut station_rng =
                DeterministicRng::new(seed ^ ((system.id.0 as u64).wrapping_mul(0x9E37_79B9)));
            for radius_mult in [0.32_f64, 0.56_f64] {
                let angle = station_rng.next_f64() * std::f64::consts::TAU;
                let radius = system.radius * radius_mult;
                let station_id = StationId(stations.len());
                let profile_roll = station_rng.next_f64();
                let profile = if profile_roll < 0.45 {
                    StationProfile::Industrial
                } else if profile_roll < 0.80 {
                    StationProfile::Civilian
                } else {
                    StationProfile::Research
                };
                stations.push(StationAnchor {
                    id: station_id,
                    system_id: system.id,
                    profile,
                    x: system.x + angle.cos() * radius,
                    y: system.y + angle.sin() * radius,
                });
                stations_by_system
                    .entry(system.id)
                    .or_default()
                    .push(station_id);
            }
        }

        let mut present = BTreeSet::new();
        for station in &stations {
            present.insert(station.profile);
        }
        let required = [
            StationProfile::Civilian,
            StationProfile::Industrial,
            StationProfile::Research,
        ];
        for (offset, profile) in required.into_iter().enumerate() {
            if present.contains(&profile) || stations.is_empty() {
                continue;
            }
            let idx = ((seed as usize).wrapping_add(offset * 7)) % stations.len();
            if let Some(station) = stations.get_mut(idx) {
                station.profile = profile;
                present.insert(profile);
            }
        }

        Self {
            systems,
            edges,
            adjacency,
            stations,
            stations_by_system,
        }
    }

    pub fn system_count(&self) -> usize {
        self.systems.len()
    }

    pub fn first_station(&self, system_id: SystemId) -> Option<StationId> {
        self.stations_by_system
            .get(&system_id)
            .and_then(|stations| stations.first().copied())
    }

    pub fn station_coords(&self, station_id: StationId) -> Option<(f64, f64)> {
        self.stations
            .iter()
            .find(|station| station.id == station_id)
            .map(|station| (station.x, station.y))
    }

    pub fn gate_coords(&self, system_id: SystemId, gate_id: GateId) -> Option<(f64, f64)> {
        self.systems
            .iter()
            .find(|system| system.id == system_id)
            .and_then(|system| {
                system
                    .gate_nodes
                    .iter()
                    .find(|node| node.gate_id == gate_id)
                    .map(|node| (node.x, node.y))
            })
    }

    pub fn degree_map(&self) -> BTreeMap<SystemId, usize> {
        self.adjacency
            .iter()
            .map(|(sid, entries)| (*sid, entries.len()))
            .collect()
    }

    pub fn is_connected(&self) -> bool {
        if self.systems.is_empty() {
            return true;
        }
        let start = self.systems[0].id;
        let mut visited = BTreeSet::new();
        let mut queue = VecDeque::new();
        visited.insert(start);
        queue.push_back(start);

        while let Some(node) = queue.pop_front() {
            if let Some(neighbors) = self.adjacency.get(&node) {
                for (next, _) in neighbors {
                    if visited.insert(*next) {
                        queue.push_back(*next);
                    }
                }
            }
        }

        visited.len() == self.systems.len()
    }

    pub fn to_graph_view(
        &self,
        tick: u64,
        gate_queue_load: &BTreeMap<GateId, f64>,
    ) -> RoutingGraphView {
        let mut gate_eta_ticks = BTreeMap::new();
        let mut gate_risk = BTreeMap::new();
        let mut blocked = BTreeSet::new();

        for edge in &self.edges {
            if edge.blocked_until_tick > tick {
                blocked.insert(edge.id);
            }
            let load = *gate_queue_load.get(&edge.id).unwrap_or(&0.0);
            let effective_capacity = (edge.base_capacity * edge.capacity_factor).max(1.0);
            let queue_penalty = (load / effective_capacity).ceil() as u32;
            gate_eta_ticks.insert(edge.id, edge.travel_ticks.saturating_add(queue_penalty));
            gate_risk.insert(edge.id, load / effective_capacity);
        }

        RoutingGraphView {
            adjacency: self.adjacency.clone(),
            gate_eta_ticks,
            gate_risk,
            blocked_edges: blocked,
        }
    }
}

struct DeterministicRng {
    state: u64,
}

impl DeterministicRng {
    fn new(seed: u64) -> Self {
        Self {
            state: seed ^ 0x9E37_79B9_7F4A_7C15,
        }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self
            .state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.state
    }

    fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / ((1_u64 << 53) as f64)
    }
}
