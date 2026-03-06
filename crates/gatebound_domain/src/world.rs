use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::{ClusterId, FactionId, GalaxyGenConfig, GateId, RoutingGraphView, StationId, SystemId};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Faction {
    pub id: FactionId,
    pub name: String,
    pub color_rgb: [u8; 3],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cluster {
    pub id: ClusterId,
    pub faction_id: FactionId,
    pub system_ids: Vec<SystemId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum StationProfile {
    Civilian,
    Industrial,
    Research,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SystemNode {
    pub id: SystemId,
    pub cluster_id: ClusterId,
    pub owner_faction_id: FactionId,
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
    pub factions: Vec<Faction>,
    pub clusters: Vec<Cluster>,
    pub systems: Vec<SystemNode>,
    pub edges: Vec<GateEdge>,
    pub adjacency: BTreeMap<SystemId, Vec<(SystemId, GateId)>>,
    pub stations: Vec<StationAnchor>,
    pub stations_by_system: BTreeMap<SystemId, Vec<StationId>>,
}

impl World {
    pub fn generate(cfg: &GalaxyGenConfig, seed: u64) -> Self {
        let mut rng = DeterministicRng::new(seed);
        let system_count = usize::from(cfg.system_count);
        let min_degree = usize::from(cfg.min_degree.max(1));
        let max_degree = usize::from(cfg.max_degree.max(cfg.min_degree.max(1)));

        let cluster_sizes = partition_cluster_sizes(cfg, &mut rng);
        let cluster_centers = layout_cluster_centers(cfg, cluster_sizes.len(), &mut rng);
        let mut assignment_rng = DeterministicRng::new(seed ^ 0xA5A5_D3C7_19E2_4F61);
        let factions = build_fixed_factions(cfg);
        let cluster_faction_ids =
            assign_cluster_factions(cfg, &cluster_centers, &mut assignment_rng);

        let mut clusters = Vec::with_capacity(cluster_sizes.len());
        let mut systems = Vec::with_capacity(system_count);
        let mut cluster_members = Vec::with_capacity(cluster_sizes.len());

        let mut next_system_id = 0_usize;
        for (cluster_idx, cluster_size) in cluster_sizes.iter().copied().enumerate() {
            let cluster_id = ClusterId(cluster_idx);
            let faction_id = cluster_faction_ids[cluster_idx];

            let (center_x, center_y) = cluster_centers[cluster_idx];
            let mut members = Vec::with_capacity(cluster_size);
            let mut orbit_order = (0..cluster_size).collect::<Vec<_>>();
            rng.shuffle(&mut orbit_order);

            for orbit_idx in orbit_order {
                let system_id = SystemId(next_system_id);
                next_system_id = next_system_id.saturating_add(1);
                let angle = (orbit_idx as f64 / cluster_size as f64) * std::f64::consts::TAU
                    + rng.next_f64() * 0.45;
                let orbit_radius = cfg.system_radius * (2.1 + rng.next_f64() * 1.1);
                systems.push(SystemNode {
                    id: system_id,
                    cluster_id,
                    owner_faction_id: faction_id,
                    x: center_x + angle.cos() * orbit_radius,
                    y: center_y + angle.sin() * orbit_radius,
                    radius: cfg.system_radius,
                    gate_nodes: Vec::new(),
                    dock_capacity: 4.0,
                });
                members.push(system_id);
            }

            members.sort_by_key(|system_id| system_id.0);
            cluster_members.push(members.clone());
            clusters.push(Cluster {
                id: cluster_id,
                faction_id,
                system_ids: members,
            });
        }

        let mut degree = vec![0_usize; system_count];
        let mut edge_set = BTreeSet::<(usize, usize)>::new();

        for members in &cluster_members {
            let mut cycle_order = members
                .iter()
                .map(|system_id| system_id.0)
                .collect::<Vec<_>>();
            rng.shuffle(&mut cycle_order);
            add_cycle_edges(&cycle_order, &mut edge_set, &mut degree);
        }

        let bridge_min = usize::from(cfg.inter_cluster_gate_min);
        let bridge_max = usize::from(cfg.inter_cluster_gate_max.max(cfg.inter_cluster_gate_min));
        for (cluster_a_idx, cluster_b_idx) in nearest_cluster_tree(&cluster_centers) {
            let desired = rng.next_usize_inclusive(bridge_min, bridge_max);
            add_bridge_edges(
                &cluster_members[cluster_a_idx],
                &cluster_members[cluster_b_idx],
                &systems,
                desired,
                max_degree,
                &mut edge_set,
                &mut degree,
            );
        }

        for members in &cluster_members {
            let desired_extra = if members.len() <= 3 {
                0
            } else {
                rng.next_usize_inclusive(0, members.len().saturating_sub(3))
            };
            add_cluster_chords(
                members,
                desired_extra,
                max_degree,
                &mut edge_set,
                &mut degree,
                &mut rng,
            );
        }

        for degree_value in &degree {
            debug_assert!(
                *degree_value >= min_degree && *degree_value <= max_degree,
                "generator must satisfy configured degree bounds"
            );
        }

        let mut edges = Vec::with_capacity(edge_set.len());
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

        let station_min = usize::from(cfg.station_count_min);
        let station_max = usize::from(cfg.station_count_max.max(cfg.station_count_min));
        let mut station_counts = vec![0_usize; system_count];
        for system in &systems {
            let mut station_rng = DeterministicRng::new(
                seed ^ ((system.id.0 as u64)
                    .wrapping_add(1)
                    .wrapping_mul(0xD1B5_4A32_D192_ED03)),
            );
            station_counts[system.id.0] =
                station_rng.next_usize_inclusive(station_min, station_max);
        }
        enforce_station_guards(&cluster_members, &mut station_counts);

        let mut stations = Vec::new();
        let mut stations_by_system: BTreeMap<SystemId, Vec<StationId>> = BTreeMap::new();
        for system in &systems {
            let station_count = station_counts[system.id.0];
            if station_count == 0 {
                continue;
            }

            let mut station_rng =
                DeterministicRng::new(seed ^ ((system.id.0 as u64).wrapping_mul(0x9E37_79B9)));
            for station_idx in 0..station_count {
                let angle = (station_idx as f64 / station_count as f64) * std::f64::consts::TAU
                    + station_rng.next_f64() * 0.35;
                let radius = system.radius * (0.28 + station_idx as f64 * 0.12)
                    + system.radius * station_rng.next_f64() * 0.08;
                let station_id = StationId(stations.len());
                let profile_roll = station_rng.next_f64();
                let profile = if profile_roll < 0.40 {
                    StationProfile::Industrial
                } else if profile_roll < 0.78 {
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

        ensure_station_profile_coverage(seed, &mut stations);

        Self {
            factions,
            clusters,
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

    pub fn station_count_in_system(&self, system_id: SystemId) -> usize {
        self.stations_by_system
            .get(&system_id)
            .map(|stations| stations.len())
            .unwrap_or(0)
    }

    pub fn has_stations(&self, system_id: SystemId) -> bool {
        self.station_count_in_system(system_id) > 0
    }

    pub fn first_station(&self, system_id: SystemId) -> Option<StationId> {
        self.stations_by_system
            .get(&system_id)
            .and_then(|stations| stations.first().copied())
    }

    pub fn faction_color(&self, faction_id: FactionId) -> Option<[u8; 3]> {
        self.factions
            .iter()
            .find(|faction| faction.id == faction_id)
            .map(|faction| faction.color_rgb)
    }

    pub fn systems_with_stations(&self) -> Vec<SystemId> {
        self.systems
            .iter()
            .filter_map(|system| self.has_stations(system.id).then_some(system.id))
            .collect()
    }

    pub fn systems_with_stations_in_cluster(&self, cluster_id: ClusterId) -> Vec<SystemId> {
        self.clusters
            .iter()
            .find(|cluster| cluster.id == cluster_id)
            .into_iter()
            .flat_map(|cluster| cluster.system_ids.iter().copied())
            .filter(|system_id| self.has_stations(*system_id))
            .collect()
    }

    pub fn station_system_id(&self, station_id: StationId) -> Option<SystemId> {
        self.stations
            .iter()
            .find(|station| station.id == station_id)
            .map(|station| station.system_id)
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

fn partition_cluster_sizes(cfg: &GalaxyGenConfig, rng: &mut DeterministicRng) -> Vec<usize> {
    let system_count = usize::from(cfg.system_count);
    let min_size = usize::from(cfg.cluster_size_min);
    let max_size = usize::from(cfg.cluster_size_max.max(cfg.cluster_size_min));
    let min_clusters = system_count.div_ceil(max_size);
    let max_clusters = system_count / min_size;
    let cluster_count = rng.next_usize_inclusive(min_clusters, max_clusters);

    let mut cluster_sizes = vec![min_size; cluster_count];
    let mut remaining = system_count.saturating_sub(min_size * cluster_count);
    while remaining > 0 {
        let idx = rng.next_usize(cluster_count);
        if cluster_sizes[idx] >= max_size {
            continue;
        }
        cluster_sizes[idx] += 1;
        remaining -= 1;
    }
    rng.shuffle(&mut cluster_sizes);
    cluster_sizes
}

fn build_fixed_factions(cfg: &GalaxyGenConfig) -> Vec<Faction> {
    cfg.factions
        .iter()
        .enumerate()
        .map(|(index, faction)| Faction {
            id: FactionId(index),
            name: faction.name.clone(),
            color_rgb: faction.color_rgb,
        })
        .collect()
}

fn assign_cluster_factions(
    cfg: &GalaxyGenConfig,
    cluster_centers: &[(f64, f64)],
    rng: &mut DeterministicRng,
) -> Vec<FactionId> {
    let cluster_count = cluster_centers.len();
    let faction_count = cfg.factions.len();
    if cluster_count == 0 || faction_count == 0 {
        return Vec::new();
    }

    let mut cluster_order = cluster_centers
        .iter()
        .enumerate()
        .map(|(index, (x, y))| (index, y.atan2(*x)))
        .collect::<Vec<_>>();
    cluster_order.sort_by(|left, right| left.1.total_cmp(&right.1));
    let mut cluster_ring = cluster_order
        .into_iter()
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    if !cluster_ring.is_empty() {
        let offset = rng.next_usize(cluster_ring.len());
        cluster_ring.rotate_left(offset);
    }

    let mut faction_order = (0..faction_count).map(FactionId).collect::<Vec<_>>();
    rng.shuffle(&mut faction_order);

    let faction_cluster_counts = faction_cluster_counts(cfg, cluster_count, &faction_order);
    let mut assignments = vec![FactionId(0); cluster_count];
    let mut cursor = 0_usize;
    for faction_id in faction_order {
        let count = faction_cluster_counts[faction_id.0];
        for _ in 0..count {
            if let Some(cluster_index) = cluster_ring.get(cursor).copied() {
                assignments[cluster_index] = faction_id;
            }
            cursor = cursor.saturating_add(1);
        }
    }
    assignments
}

fn faction_cluster_counts(
    cfg: &GalaxyGenConfig,
    cluster_count: usize,
    faction_order: &[FactionId],
) -> Vec<usize> {
    let faction_count = cfg.factions.len();
    let mut counts = vec![0_usize; faction_count];

    if cluster_count <= faction_count {
        for faction_id in faction_order.iter().take(cluster_count) {
            counts[faction_id.0] = 1;
        }
        return counts;
    }

    counts.iter_mut().for_each(|count| *count = 1);
    let remaining = cluster_count.saturating_sub(faction_count);
    let total_weight = cfg
        .factions
        .iter()
        .map(|faction| faction.cluster_weight)
        .sum::<f64>()
        .max(f64::EPSILON);

    let sector_priority = faction_order
        .iter()
        .enumerate()
        .map(|(index, faction_id)| (faction_id.0, index))
        .collect::<BTreeMap<_, _>>();
    let mut allocations = cfg
        .factions
        .iter()
        .enumerate()
        .map(|(index, faction)| {
            let raw = remaining as f64 * faction.cluster_weight / total_weight;
            let base = raw.floor() as usize;
            (index, base, raw - base as f64)
        })
        .collect::<Vec<_>>();
    let allocated = allocations.iter().map(|(_, base, _)| *base).sum::<usize>();
    let leftover = remaining.saturating_sub(allocated);

    for (index, base, _) in &allocations {
        counts[*index] += *base;
    }
    allocations.sort_by(|left, right| {
        right
            .2
            .total_cmp(&left.2)
            .then_with(|| sector_priority[&left.0].cmp(&sector_priority[&right.0]))
            .then_with(|| left.0.cmp(&right.0))
    });
    for (index, _, _) in allocations.into_iter().take(leftover) {
        counts[index] += 1;
    }

    counts
}

fn layout_cluster_centers(
    cfg: &GalaxyGenConfig,
    cluster_count: usize,
    rng: &mut DeterministicRng,
) -> Vec<(f64, f64)> {
    let mut centers = Vec::with_capacity(cluster_count);
    let orbit_radius = cfg.system_radius * (8.0 + cluster_count as f64 * 0.45);
    for idx in 0..cluster_count {
        let angle =
            (idx as f64 / cluster_count as f64) * std::f64::consts::TAU + rng.next_f64() * 0.25;
        let jitter = cfg.system_radius * (0.8 + rng.next_f64() * 0.8);
        centers.push((
            (orbit_radius + jitter) * angle.cos(),
            (orbit_radius + jitter) * angle.sin(),
        ));
    }
    centers
}

fn nearest_cluster_tree(cluster_centers: &[(f64, f64)]) -> Vec<(usize, usize)> {
    let mut candidates = Vec::new();
    for left in 0..cluster_centers.len() {
        for right in (left + 1)..cluster_centers.len() {
            let dx = cluster_centers[left].0 - cluster_centers[right].0;
            let dy = cluster_centers[left].1 - cluster_centers[right].1;
            candidates.push(((left, right), dx * dx + dy * dy));
        }
    }
    candidates.sort_by(|left, right| {
        left.1
            .total_cmp(&right.1)
            .then_with(|| left.0 .0.cmp(&right.0 .0))
            .then_with(|| left.0 .1.cmp(&right.0 .1))
    });

    let mut union_find = UnionFind::new(cluster_centers.len());
    let mut tree_edges = Vec::with_capacity(cluster_centers.len().saturating_sub(1));
    for ((left, right), _) in candidates {
        if union_find.union(left, right) {
            tree_edges.push((left, right));
            if tree_edges.len() + 1 >= cluster_centers.len() {
                break;
            }
        }
    }
    tree_edges
}

fn add_cycle_edges(order: &[usize], edge_set: &mut BTreeSet<(usize, usize)>, degree: &mut [usize]) {
    for idx in 0..order.len() {
        let a = order[idx];
        let b = order[(idx + 1) % order.len()];
        add_edge(a, b, edge_set, degree);
    }
}

fn add_bridge_edges(
    cluster_a: &[SystemId],
    cluster_b: &[SystemId],
    systems: &[SystemNode],
    desired_count: usize,
    max_degree: usize,
    edge_set: &mut BTreeSet<(usize, usize)>,
    degree: &mut [usize],
) {
    let mut candidate_pairs = cluster_a
        .iter()
        .copied()
        .flat_map(|left| {
            cluster_b.iter().copied().map(move |right| {
                let left_system = &systems[left.0];
                let right_system = &systems[right.0];
                let dx = left_system.x - right_system.x;
                let dy = left_system.y - right_system.y;
                (ordered_pair(left.0, right.0), dx * dx + dy * dy)
            })
        })
        .collect::<Vec<_>>();
    candidate_pairs.sort_by(|left, right| {
        left.1
            .total_cmp(&right.1)
            .then_with(|| left.0 .0.cmp(&right.0 .0))
            .then_with(|| left.0 .1.cmp(&right.0 .1))
    });

    let mut added = 0_usize;
    for ((left, right), _) in candidate_pairs {
        if added >= desired_count {
            break;
        }
        if degree[left] >= max_degree || degree[right] >= max_degree {
            continue;
        }
        if edge_set.contains(&(left, right)) {
            continue;
        }
        add_edge(left, right, edge_set, degree);
        added += 1;
    }
}

fn add_cluster_chords(
    members: &[SystemId],
    desired_count: usize,
    max_degree: usize,
    edge_set: &mut BTreeSet<(usize, usize)>,
    degree: &mut [usize],
    rng: &mut DeterministicRng,
) {
    let member_ids = members
        .iter()
        .map(|system_id| system_id.0)
        .collect::<Vec<_>>();
    let mut candidate_pairs = Vec::new();
    for i in 0..member_ids.len() {
        for j in (i + 1)..member_ids.len() {
            let edge = ordered_pair(member_ids[i], member_ids[j]);
            if edge_set.contains(&edge) {
                continue;
            }
            candidate_pairs.push(edge);
        }
    }

    rng.shuffle(&mut candidate_pairs);
    let mut added = 0_usize;
    for (a, b) in candidate_pairs {
        if added >= desired_count {
            break;
        }
        if degree[a] >= max_degree || degree[b] >= max_degree {
            continue;
        }
        add_edge(a, b, edge_set, degree);
        added += 1;
    }
}

fn add_edge(a: usize, b: usize, edge_set: &mut BTreeSet<(usize, usize)>, degree: &mut [usize]) {
    let edge = ordered_pair(a, b);
    if edge_set.insert(edge) {
        degree[edge.0] += 1;
        degree[edge.1] += 1;
    }
}

fn ordered_pair(a: usize, b: usize) -> (usize, usize) {
    if a < b {
        (a, b)
    } else {
        (b, a)
    }
}

fn enforce_station_guards(cluster_members: &[Vec<SystemId>], station_counts: &mut [usize]) {
    for (cluster_idx, members) in cluster_members.iter().enumerate() {
        if cluster_idx == 0 {
            for system_id in members.iter().take(2) {
                station_counts[system_id.0] = station_counts[system_id.0].max(2);
            }
        }

        let minimum_station_systems = if cluster_idx == 0 { 2 } else { 1 };
        let mut station_systems = members
            .iter()
            .filter(|system_id| station_counts[system_id.0] > 0)
            .count();
        for system_id in members {
            if station_systems >= minimum_station_systems {
                break;
            }
            if station_counts[system_id.0] == 0 {
                station_counts[system_id.0] = 1;
                station_systems += 1;
            }
        }
    }
}

fn ensure_station_profile_coverage(seed: u64, stations: &mut [StationAnchor]) {
    let mut present = BTreeSet::new();
    for station in stations.iter() {
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
}

struct UnionFind {
    parent: Vec<usize>,
    rank: Vec<u8>,
}

impl UnionFind {
    fn new(size: usize) -> Self {
        Self {
            parent: (0..size).collect(),
            rank: vec![0; size],
        }
    }

    fn find(&mut self, node: usize) -> usize {
        if self.parent[node] != node {
            let root = self.find(self.parent[node]);
            self.parent[node] = root;
        }
        self.parent[node]
    }

    fn union(&mut self, left: usize, right: usize) -> bool {
        let left_root = self.find(left);
        let right_root = self.find(right);
        if left_root == right_root {
            return false;
        }

        if self.rank[left_root] < self.rank[right_root] {
            self.parent[left_root] = right_root;
        } else if self.rank[left_root] > self.rank[right_root] {
            self.parent[right_root] = left_root;
        } else {
            self.parent[right_root] = left_root;
            self.rank[left_root] = self.rank[left_root].saturating_add(1);
        }

        true
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

    fn next_usize(&mut self, bound: usize) -> usize {
        if bound == 0 {
            0
        } else {
            usize::try_from(self.next_u64() % u64::try_from(bound).unwrap_or(1)).unwrap_or(0)
        }
    }

    fn next_usize_inclusive(&mut self, min: usize, max: usize) -> usize {
        if min >= max {
            min
        } else {
            min + self.next_usize(max - min + 1)
        }
    }

    fn shuffle<T>(&mut self, slice: &mut [T]) {
        for idx in (1..slice.len()).rev() {
            let swap_idx = self.next_usize(idx + 1);
            slice.swap(idx, swap_idx);
        }
    }
}
