#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt::{Display, Formatter};
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SystemId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GateId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StationId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CompanyId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ShipId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContractId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimeUnitsConfig {
    pub tick_seconds: u32,
    pub cycle_ticks: u32,
    pub rolling_window_cycles: u32,
}

impl Default for TimeUnitsConfig {
    fn default() -> Self {
        Self {
            tick_seconds: 1,
            cycle_ticks: 60,
            rolling_window_cycles: 20,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GalaxyGenConfig {
    pub seed: u64,
    pub cluster_system_min: u8,
    pub cluster_system_max: u8,
    pub min_degree: u8,
    pub max_degree: u8,
    pub system_radius: f64,
    pub base_gate_capacity: f64,
    pub base_gate_travel_ticks: u32,
}

impl Default for GalaxyGenConfig {
    fn default() -> Self {
        Self {
            seed: 42,
            cluster_system_min: 3,
            cluster_system_max: 7,
            min_degree: 1,
            max_degree: 3,
            system_radius: 100.0,
            base_gate_capacity: 8.0,
            base_gate_travel_ticks: 15,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MarketConfig {
    pub k_stock: f64,
    pub k_flow: f64,
    pub delta_cap: f64,
    pub floor_mult: f64,
    pub ceiling_mult: f64,
}

impl Default for MarketConfig {
    fn default() -> Self {
        Self {
            k_stock: 0.08,
            k_flow: 0.04,
            delta_cap: 0.10,
            floor_mult: 0.25,
            ceiling_mult: 4.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct EconomyPressureConfig {
    pub loan_interest_rate: f64,
    pub ship_upkeep_per_tick: f64,
    pub slot_lease_cost: f64,
    pub sla_penalty_curve: Vec<f64>,
}

impl Default for EconomyPressureConfig {
    fn default() -> Self {
        Self {
            loan_interest_rate: 0.02,
            ship_upkeep_per_tick: 0.5,
            slot_lease_cost: 2.0,
            sla_penalty_curve: vec![1.0, 1.3, 1.7, 2.2, 2.8],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct RuntimeConfig {
    pub time: TimeUnitsConfig,
    pub galaxy: GalaxyGenConfig,
    pub market: MarketConfig,
    pub pressure: EconomyPressureConfig,
}

impl RuntimeConfig {
    pub fn load_from_dir(dir: &Path) -> Result<Self, ConfigError> {
        let mut cfg = RuntimeConfig::default();

        let time = parse_simple_toml_map(
            &fs::read_to_string(dir.join("time_units.toml"))
                .map_err(|e| ConfigError::Io(format!("failed to read time_units.toml: {e}")))?,
        );
        cfg.time.tick_seconds = parse_required_u32(&time, "tick_seconds")?;
        cfg.time.cycle_ticks = parse_required_u32(&time, "cycle_ticks")?;
        cfg.time.rolling_window_cycles = parse_required_u32(&time, "rolling_window_cycles")?;

        let galaxy = parse_simple_toml_map(
            &fs::read_to_string(dir.join("galaxy.toml"))
                .map_err(|e| ConfigError::Io(format!("failed to read galaxy.toml: {e}")))?,
        );
        cfg.galaxy.seed = parse_required_u64(&galaxy, "seed")?;
        cfg.galaxy.cluster_system_min = parse_required_u8(&galaxy, "cluster_system_min")?;
        cfg.galaxy.cluster_system_max = parse_required_u8(&galaxy, "cluster_system_max")?;
        cfg.galaxy.min_degree = parse_required_u8(&galaxy, "min_degree")?;
        cfg.galaxy.max_degree = parse_required_u8(&galaxy, "max_degree")?;
        cfg.galaxy.system_radius = parse_required_f64(&galaxy, "system_radius")?;
        cfg.galaxy.base_gate_capacity = parse_required_f64(&galaxy, "base_gate_capacity")?;
        cfg.galaxy.base_gate_travel_ticks = parse_required_u32(&galaxy, "base_gate_travel_ticks")?;

        let market = parse_simple_toml_map(
            &fs::read_to_string(dir.join("market.toml"))
                .map_err(|e| ConfigError::Io(format!("failed to read market.toml: {e}")))?,
        );
        cfg.market.k_stock = parse_required_f64(&market, "k_stock")?;
        cfg.market.k_flow = parse_required_f64(&market, "k_flow")?;
        cfg.market.delta_cap = parse_required_f64(&market, "delta_cap")?;
        cfg.market.floor_mult = parse_required_f64(&market, "floor_mult")?;
        cfg.market.ceiling_mult = parse_required_f64(&market, "ceiling_mult")?;

        let pressure = parse_simple_toml_map(
            &fs::read_to_string(dir.join("economy_pressure.toml")).map_err(|e| {
                ConfigError::Io(format!("failed to read economy_pressure.toml: {e}"))
            })?,
        );
        cfg.pressure.loan_interest_rate = parse_required_f64(&pressure, "loan_interest_rate")?;
        cfg.pressure.ship_upkeep_per_tick = parse_required_f64(&pressure, "ship_upkeep_per_tick")?;
        cfg.pressure.slot_lease_cost = parse_required_f64(&pressure, "slot_lease_cost")?;
        cfg.pressure.sla_penalty_curve = parse_required_f64_array(&pressure, "sla_penalty_curve")?;

        cfg.validate()?;
        Ok(cfg)
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.time.tick_seconds == 0 {
            return Err(ConfigError::Validation(
                "tick_seconds must be > 0".to_string(),
            ));
        }
        if self.time.cycle_ticks == 0 {
            return Err(ConfigError::Validation(
                "cycle_ticks must be > 0".to_string(),
            ));
        }
        if self.time.rolling_window_cycles == 0 {
            return Err(ConfigError::Validation(
                "rolling_window_cycles must be > 0".to_string(),
            ));
        }
        if self.galaxy.cluster_system_min == 0 || self.galaxy.cluster_system_max == 0 {
            return Err(ConfigError::Validation(
                "cluster_system_(min|max) must be > 0".to_string(),
            ));
        }
        if self.galaxy.cluster_system_min > self.galaxy.cluster_system_max {
            return Err(ConfigError::Validation(
                "cluster_system_min must be <= cluster_system_max".to_string(),
            ));
        }
        if self.galaxy.min_degree > self.galaxy.max_degree {
            return Err(ConfigError::Validation(
                "min_degree must be <= max_degree".to_string(),
            ));
        }
        if self.market.delta_cap <= 0.0 {
            return Err(ConfigError::Validation("delta_cap must be > 0".to_string()));
        }
        if self.market.floor_mult <= 0.0 || self.market.ceiling_mult <= self.market.floor_mult {
            return Err(ConfigError::Validation(
                "market floor/ceiling multipliers invalid".to_string(),
            ));
        }
        if self.pressure.sla_penalty_curve.is_empty() {
            return Err(ConfigError::Validation(
                "sla_penalty_curve must not be empty".to_string(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug)]
pub enum ConfigError {
    Io(String),
    Parse(String),
    Validation(String),
}

impl Display for ConfigError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(v) | Self::Parse(v) | Self::Validation(v) => write!(f, "{v}"),
        }
    }
}

impl std::error::Error for ConfigError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Commodity {
    Ore,
    Ice,
    Gas,
    Metal,
    Fuel,
    Parts,
    Electronics,
}

impl Commodity {
    pub const ALL: [Commodity; 7] = [
        Commodity::Ore,
        Commodity::Ice,
        Commodity::Gas,
        Commodity::Metal,
        Commodity::Fuel,
        Commodity::Parts,
        Commodity::Electronics,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContractTypeStageA {
    Delivery,
    Supply,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiskStageA {
    GateCongestion,
    DockCongestion,
    FuelShock,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PriorityMode {
    Profit,
    Stability,
    Hybrid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepeatMode {
    Loop,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AutopilotPolicy {
    pub min_margin: f64,
    pub max_risk_score: f64,
    pub max_hops: usize,
    pub priority_mode: PriorityMode,
    pub waypoints: Vec<SystemId>,
    pub repeat_mode: RepeatMode,
}

impl Default for AutopilotPolicy {
    fn default() -> Self {
        Self {
            min_margin: 0.0,
            max_risk_score: 100.0,
            max_hops: 6,
            priority_mode: PriorityMode::Hybrid,
            waypoints: vec![SystemId(0)],
            repeat_mode: RepeatMode::Loop,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegmentKind {
    InSystem,
    GateQueue,
    Warp,
    Dock,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RouteSegment {
    pub from: SystemId,
    pub to: SystemId,
    pub edge: Option<GateId>,
    pub kind: SegmentKind,
    pub eta_ticks: u32,
    pub risk: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RoutePlan {
    pub segments: Vec<RouteSegment>,
    pub eta_ticks: u32,
    pub risk_score: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MarketIntel {
    pub system_id: SystemId,
    pub observed_tick: u64,
    pub staleness_ticks: u64,
    pub confidence: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SnapshotV1 {
    pub version: u32,
    pub state: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TickReport {
    pub tick: u64,
    pub cycle: u64,
    pub active_ships: usize,
    pub active_contracts: usize,
    pub total_queue_delay: u64,
    pub avg_price_index: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CycleReport {
    pub cycle: u64,
    pub sla_success_rate: f64,
    pub reroute_count: u64,
    pub economy_stress_index: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RiskEvent {
    GateCongestion {
        edge: GateId,
        capacity_factor: f64,
        duration_ticks: u32,
    },
    DockCongestion {
        delay_factor: f64,
        duration_ticks: u32,
    },
    FuelShock {
        production_factor: f64,
        duration_ticks: u32,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct RoutingRequest {
    pub origin: SystemId,
    pub destination: SystemId,
    pub policy: AutopilotPolicy,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RoutingGraphView {
    pub adjacency: BTreeMap<SystemId, Vec<(SystemId, GateId)>>,
    pub gate_eta_ticks: BTreeMap<GateId, u32>,
    pub gate_risk: BTreeMap<GateId, f64>,
    pub blocked_edges: BTreeSet<GateId>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RoutingError {
    Unreachable,
    MaxHopsExceeded,
}

pub struct RoutingService;

impl RoutingService {
    pub fn plan_route(
        graph: &RoutingGraphView,
        request: &RoutingRequest,
    ) -> Result<RoutePlan, RoutingError> {
        if request.origin == request.destination {
            return Ok(RoutePlan {
                segments: Vec::new(),
                eta_ticks: 0,
                risk_score: 0.0,
            });
        }

        let mut queue = VecDeque::new();
        let mut prev: BTreeMap<SystemId, (SystemId, GateId)> = BTreeMap::new();
        let mut visited = BTreeSet::new();
        visited.insert(request.origin);
        queue.push_back(request.origin);

        while let Some(node) = queue.pop_front() {
            if node == request.destination {
                break;
            }
            if let Some(neighbors) = graph.adjacency.get(&node) {
                for (next, gate) in neighbors {
                    if graph.blocked_edges.contains(gate) {
                        continue;
                    }
                    if visited.insert(*next) {
                        prev.insert(*next, (node, *gate));
                        queue.push_back(*next);
                    }
                }
            }
        }

        if !visited.contains(&request.destination) {
            return Err(RoutingError::Unreachable);
        }

        let mut rev = Vec::<(SystemId, SystemId, GateId)>::new();
        let mut cursor = request.destination;
        while cursor != request.origin {
            let (p, gate) = prev
                .get(&cursor)
                .copied()
                .ok_or(RoutingError::Unreachable)?;
            rev.push((p, cursor, gate));
            cursor = p;
        }
        rev.reverse();

        if rev.len() > request.policy.max_hops {
            return Err(RoutingError::MaxHopsExceeded);
        }

        let mut segments = Vec::new();
        let mut eta = 0_u32;
        let mut risk = 0.0_f64;

        for (from, to, gate) in rev {
            let gate_eta = *graph.gate_eta_ticks.get(&gate).unwrap_or(&1);
            let gate_risk = *graph.gate_risk.get(&gate).unwrap_or(&0.0);

            segments.push(RouteSegment {
                from,
                to,
                edge: Some(gate),
                kind: SegmentKind::Warp,
                eta_ticks: gate_eta,
                risk: gate_risk,
            });
            eta = eta.saturating_add(gate_eta);
            risk += gate_risk;
        }

        Ok(RoutePlan {
            segments,
            eta_ticks: eta,
            risk_score: risk,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SystemNode {
    pub id: SystemId,
    pub x: f64,
    pub y: f64,
    pub radius: f64,
    pub gate_nodes: Vec<GateNode>,
    pub dock_capacity: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GateNode {
    pub gate_id: GateId,
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GateEdge {
    pub id: GateId,
    pub a: SystemId,
    pub b: SystemId,
    pub base_capacity: f64,
    pub travel_ticks: u32,
    pub blocked_until_tick: u64,
    pub capacity_factor: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct World {
    pub systems: Vec<SystemNode>,
    pub edges: Vec<GateEdge>,
    pub adjacency: BTreeMap<SystemId, Vec<(SystemId, GateId)>>,
}

impl World {
    fn generate(cfg: &GalaxyGenConfig, seed: u64) -> Self {
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

        Self {
            systems,
            edges,
            adjacency,
        }
    }

    pub fn system_count(&self) -> usize {
        self.systems.len()
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

    fn to_graph_view(
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

#[derive(Debug, Clone, PartialEq)]
pub struct MarketState {
    pub base_price: f64,
    pub price: f64,
    pub stock: f64,
    pub target_stock: f64,
    pub cycle_inflow: f64,
    pub cycle_outflow: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MarketBook {
    pub goods: BTreeMap<Commodity, MarketState>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Contract {
    pub id: ContractId,
    pub kind: ContractTypeStageA,
    pub origin: SystemId,
    pub destination: SystemId,
    pub quantity: f64,
    pub deadline_tick: u64,
    pub per_cycle: f64,
    pub total_cycles: u32,
    pub payout: f64,
    pub penalty: f64,
    pub assigned_ship: Option<ShipId>,
    pub delivered_amount: f64,
    pub missed_cycles: u32,
    pub completed: bool,
    pub failed: bool,
    pub last_eval_cycle: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Ship {
    pub id: ShipId,
    pub company_id: CompanyId,
    pub location: SystemId,
    pub eta_ticks_remaining: u32,
    pub active_contract: Option<ContractId>,
    pub route_cursor: usize,
    pub policy: AutopilotPolicy,
    pub planned_path: Vec<SystemId>,
    pub current_target: Option<SystemId>,
    pub last_risk_score: f64,
    pub reroutes: u64,
}

#[derive(Debug, Clone)]
struct ActiveModifier {
    until_tick: u64,
    gate: Option<GateId>,
    risk: RiskStageA,
    magnitude: f64,
}

#[derive(Debug, Clone)]
pub struct Simulation {
    pub config: RuntimeConfig,
    pub world: World,
    pub tick: u64,
    pub cycle: u64,
    pub markets: BTreeMap<SystemId, MarketBook>,
    pub contracts: BTreeMap<ContractId, Contract>,
    pub ships: BTreeMap<ShipId, Ship>,
    pub capital: f64,
    pub queue_delay_accumulator: u64,
    pub reroute_count: u64,
    pub sla_successes: u64,
    pub sla_failures: u64,
    pub gate_queue_load: BTreeMap<GateId, f64>,
    modifiers: Vec<ActiveModifier>,
}

impl Simulation {
    pub fn new(config: RuntimeConfig, seed: u64) -> Self {
        let world = World::generate(&config.galaxy, seed);
        let mut markets = BTreeMap::new();
        for system in &world.systems {
            let mut goods = BTreeMap::new();
            for commodity in Commodity::ALL {
                let base_price = base_price_for(commodity);
                goods.insert(
                    commodity,
                    MarketState {
                        base_price,
                        price: base_price,
                        stock: 100.0,
                        target_stock: 100.0,
                        cycle_inflow: 0.0,
                        cycle_outflow: 0.0,
                    },
                );
            }
            markets.insert(system.id, MarketBook { goods });
        }

        let mut contracts = BTreeMap::new();
        let mut ships = BTreeMap::new();
        if world.system_count() >= 2 {
            contracts.insert(
                ContractId(0),
                Contract {
                    id: ContractId(0),
                    kind: ContractTypeStageA::Delivery,
                    origin: SystemId(0),
                    destination: SystemId(1),
                    quantity: 10.0,
                    deadline_tick: u64::from(config.time.cycle_ticks) * 3,
                    per_cycle: 0.0,
                    total_cycles: 0,
                    payout: 50.0,
                    penalty: 25.0,
                    assigned_ship: Some(ShipId(0)),
                    delivered_amount: 0.0,
                    missed_cycles: 0,
                    completed: false,
                    failed: false,
                    last_eval_cycle: 0,
                },
            );

            let policy = AutopilotPolicy {
                waypoints: vec![SystemId(0), SystemId(1)],
                ..AutopilotPolicy::default()
            };
            ships.insert(
                ShipId(0),
                Ship {
                    id: ShipId(0),
                    company_id: CompanyId(0),
                    location: SystemId(0),
                    eta_ticks_remaining: 0,
                    active_contract: Some(ContractId(0)),
                    route_cursor: 0,
                    policy,
                    planned_path: Vec::new(),
                    current_target: None,
                    last_risk_score: 0.0,
                    reroutes: 0,
                },
            );
        }

        Self {
            config,
            world,
            tick: 0,
            cycle: 0,
            markets,
            contracts,
            ships,
            capital: 500.0,
            queue_delay_accumulator: 0,
            reroute_count: 0,
            sla_successes: 0,
            sla_failures: 0,
            gate_queue_load: BTreeMap::new(),
            modifiers: Vec::new(),
        }
    }

    pub fn step_tick(&mut self) -> TickReport {
        self.tick = self.tick.saturating_add(1);
        self.apply_upkeep();
        self.update_ship_movements();
        self.run_economy_flow();
        self.update_contracts_tick();
        self.expire_modifiers();

        if self
            .tick
            .is_multiple_of(u64::from(self.config.time.cycle_ticks))
        {
            self.step_cycle();
        }

        TickReport {
            tick: self.tick,
            cycle: self.cycle,
            active_ships: self.ships.len(),
            active_contracts: self
                .contracts
                .values()
                .filter(|c| !c.completed && !c.failed)
                .count(),
            total_queue_delay: self.queue_delay_accumulator,
            avg_price_index: self.average_price_index(),
        }
    }

    pub fn step_cycle(&mut self) -> CycleReport {
        self.cycle = self.cycle.saturating_add(1);
        self.update_market_prices();
        self.evaluate_supply_contracts();

        let total_sla = self.sla_successes + self.sla_failures;
        let sla_success_rate = if total_sla == 0 {
            1.0
        } else {
            self.sla_successes as f64 / total_sla as f64
        };

        let economy_stress_index = (1.0 - sla_success_rate).clamp(0.0, 1.0)
            + self.average_gate_load().clamp(0.0, 1.0)
            + self.average_price_index().max(1.0)
            - 1.0;

        CycleReport {
            cycle: self.cycle,
            sla_success_rate,
            reroute_count: self.reroute_count,
            economy_stress_index,
        }
    }

    pub fn apply_event(&mut self, event: RiskEvent) {
        match event {
            RiskEvent::GateCongestion {
                edge,
                capacity_factor,
                duration_ticks,
            } => {
                if let Some(found) = self.world.edges.iter_mut().find(|e| e.id == edge) {
                    found.capacity_factor = capacity_factor;
                }
                self.modifiers.push(ActiveModifier {
                    until_tick: self.tick + u64::from(duration_ticks),
                    gate: Some(edge),
                    risk: RiskStageA::GateCongestion,
                    magnitude: capacity_factor,
                });
            }
            RiskEvent::DockCongestion {
                delay_factor,
                duration_ticks,
            } => {
                self.modifiers.push(ActiveModifier {
                    until_tick: self.tick + u64::from(duration_ticks),
                    gate: None,
                    risk: RiskStageA::DockCongestion,
                    magnitude: delay_factor,
                });
            }
            RiskEvent::FuelShock {
                production_factor,
                duration_ticks,
            } => {
                self.modifiers.push(ActiveModifier {
                    until_tick: self.tick + u64::from(duration_ticks),
                    gate: None,
                    risk: RiskStageA::FuelShock,
                    magnitude: production_factor,
                });
            }
        }
    }

    pub fn save_snapshot(&self, path: &Path) -> Result<(), SnapshotError> {
        let state = self.serialize_state();
        let json = format!("{{\"version\":1,\"state\":\"{state}\"}}\n");
        fs::write(path, json).map_err(|e| SnapshotError::Io(format!("save failed: {e}")))
    }

    pub fn load_snapshot(path: &Path, config: RuntimeConfig) -> Result<Self, SnapshotError> {
        let payload =
            fs::read_to_string(path).map_err(|e| SnapshotError::Io(format!("load failed: {e}")))?;
        let state = extract_json_string_field(&payload, "state")
            .ok_or_else(|| SnapshotError::Parse("missing state field".to_string()))?;
        Self::deserialize_state(&state, config)
    }

    pub fn snapshot_hash(&self) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.serialize_state().hash(&mut hasher);
        hasher.finish()
    }

    pub fn market_intel(&self, system_id: SystemId, local_cluster: bool) -> Option<MarketIntel> {
        self.markets.get(&system_id).map(|_| {
            if local_cluster {
                MarketIntel {
                    system_id,
                    observed_tick: self.tick,
                    staleness_ticks: 0,
                    confidence: 1.0,
                }
            } else {
                let staleness = 5 + (self.tick % 13);
                let confidence = (1.0 - staleness as f64 / 30.0).clamp(0.3, 0.95);
                MarketIntel {
                    system_id,
                    observed_tick: self.tick.saturating_sub(staleness),
                    staleness_ticks: staleness,
                    confidence,
                }
            }
        })
    }

    pub fn set_edge_blocked_until(&mut self, edge: GateId, until_tick: u64) {
        if let Some(item) = self.world.edges.iter_mut().find(|e| e.id == edge) {
            item.blocked_until_tick = until_tick;
        }
    }

    pub fn route_for_ship(&self, ship_id: ShipId, destination: SystemId) -> Option<RoutePlan> {
        let ship = self.ships.get(&ship_id)?;
        let req = RoutingRequest {
            origin: ship.location,
            destination,
            policy: ship.policy.clone(),
        };
        let graph = self.world.to_graph_view(self.tick, &self.gate_queue_load);
        RoutingService::plan_route(&graph, &req).ok()
    }

    pub fn create_supply_contract(
        &mut self,
        origin: SystemId,
        destination: SystemId,
        per_cycle: f64,
        total_cycles: u32,
    ) -> ContractId {
        let next_id = ContractId(self.contracts.len());
        self.contracts.insert(
            next_id,
            Contract {
                id: next_id,
                kind: ContractTypeStageA::Supply,
                origin,
                destination,
                quantity: per_cycle,
                deadline_tick: 0,
                per_cycle,
                total_cycles,
                payout: 20.0,
                penalty: 10.0,
                assigned_ship: None,
                delivered_amount: 0.0,
                missed_cycles: 0,
                completed: false,
                failed: false,
                last_eval_cycle: self.cycle,
            },
        );
        next_id
    }

    fn apply_upkeep(&mut self) {
        let ship_upkeep = self.config.pressure.ship_upkeep_per_tick * self.ships.len() as f64;
        self.capital -= ship_upkeep + self.config.pressure.slot_lease_cost;
    }

    fn update_ship_movements(&mut self) {
        self.gate_queue_load.clear();
        let dock_delay_factor = self
            .modifiers
            .iter()
            .filter(|m| m.risk == RiskStageA::DockCongestion)
            .map(|m| m.magnitude)
            .fold(1.0_f64, f64::max);

        let ship_ids: Vec<ShipId> = self.ships.keys().copied().collect();
        for ship_id in ship_ids {
            let Some(ship_snapshot) = self.ships.get(&ship_id).cloned() else {
                continue;
            };

            if ship_snapshot.eta_ticks_remaining > 0 {
                if let Some(ship) = self.ships.get_mut(&ship_id) {
                    ship.eta_ticks_remaining = ship.eta_ticks_remaining.saturating_sub(1);
                    if ship.eta_ticks_remaining == 0 {
                        if let Some(target) = ship.current_target {
                            ship.location = target;
                        }
                    }
                }
                continue;
            }

            let next_target = self.next_waypoint(ship_id);
            let Some(target) = next_target else {
                continue;
            };

            if ship_snapshot.location == target {
                if let Some(ship) = self.ships.get_mut(&ship_id) {
                    if !ship.policy.waypoints.is_empty() {
                        ship.route_cursor = (ship.route_cursor + 1) % ship.policy.waypoints.len();
                    }
                }
                continue;
            }

            let graph = self.world.to_graph_view(self.tick, &self.gate_queue_load);
            let request = RoutingRequest {
                origin: ship_snapshot.location,
                destination: target,
                policy: ship_snapshot.policy.clone(),
            };

            let route = match RoutingService::plan_route(&graph, &request) {
                Ok(route) => route,
                Err(_) => {
                    if let Some(ship) = self.ships.get_mut(&ship_id) {
                        ship.reroutes = ship.reroutes.saturating_add(1);
                    }
                    self.reroute_count = self.reroute_count.saturating_add(1);
                    continue;
                }
            };

            let acceptable_risk = ship_snapshot.policy.max_risk_score;
            if route.risk_score > acceptable_risk {
                continue;
            }

            let Some(first_segment) = route.segments.first() else {
                continue;
            };
            let edge = first_segment.edge.unwrap_or(GateId(usize::MAX));
            *self.gate_queue_load.entry(edge).or_insert(0.0) += 1.0;

            let queue_load = self.gate_queue_load.get(&edge).copied().unwrap_or(0.0);
            let edge_ref = self.world.edges.iter().find(|e| e.id == edge);
            let effective_capacity = edge_ref
                .map(|e| (e.base_capacity * e.capacity_factor).max(1.0))
                .unwrap_or(1.0);
            let queue_delay = (queue_load / effective_capacity).ceil() as u32;
            self.queue_delay_accumulator = self
                .queue_delay_accumulator
                .saturating_add(u64::from(queue_delay));

            let eta = first_segment
                .eta_ticks
                .saturating_add(queue_delay)
                .saturating_add(dock_delay_factor.ceil() as u32);

            if let Some(ship) = self.ships.get_mut(&ship_id) {
                ship.eta_ticks_remaining = eta.max(1);
                ship.current_target = Some(first_segment.to);
                ship.last_risk_score = route.risk_score;
                ship.planned_path = route
                    .segments
                    .iter()
                    .map(|s| s.to)
                    .collect::<Vec<SystemId>>();
            }
        }
    }

    fn update_contracts_tick(&mut self) {
        let contract_ids: Vec<ContractId> = self.contracts.keys().copied().collect();
        for cid in contract_ids {
            let Some(snapshot) = self.contracts.get(&cid).cloned() else {
                continue;
            };
            if snapshot.completed || snapshot.failed {
                continue;
            }

            match snapshot.kind {
                ContractTypeStageA::Delivery => {
                    let Some(ship_id) = snapshot.assigned_ship else {
                        continue;
                    };
                    let arrived = self
                        .ships
                        .get(&ship_id)
                        .map(|s| s.location == snapshot.destination && s.eta_ticks_remaining == 0)
                        .unwrap_or(false);

                    if arrived {
                        if let Some(c) = self.contracts.get_mut(&cid) {
                            c.completed = true;
                            c.delivered_amount = c.quantity;
                        }
                        self.capital += snapshot.payout;
                        self.sla_successes = self.sla_successes.saturating_add(1);
                    } else if self.tick > snapshot.deadline_tick {
                        let penalty_mult = self.penalty_multiplier(snapshot.missed_cycles as usize);
                        self.capital -= snapshot.penalty * penalty_mult;
                        if let Some(c) = self.contracts.get_mut(&cid) {
                            c.failed = true;
                            c.missed_cycles = c.missed_cycles.saturating_add(1);
                        }
                        self.sla_failures = self.sla_failures.saturating_add(1);
                    }
                }
                ContractTypeStageA::Supply => {
                    let delivered = snapshot.assigned_ship.and_then(|ship_id| {
                        self.ships.get(&ship_id).map(|s| {
                            s.location == snapshot.destination && s.eta_ticks_remaining == 0
                        })
                    });
                    if delivered == Some(true) {
                        if let Some(contract) = self.contracts.get_mut(&cid) {
                            contract.delivered_amount += snapshot.per_cycle;
                        }
                    }
                }
            }
        }
    }

    fn evaluate_supply_contracts(&mut self) {
        let ids: Vec<ContractId> = self.contracts.keys().copied().collect();
        for cid in ids {
            let Some(current) = self.contracts.get(&cid).cloned() else {
                continue;
            };
            if current.kind != ContractTypeStageA::Supply || current.completed || current.failed {
                continue;
            }
            if current.last_eval_cycle == self.cycle {
                continue;
            }

            let delta = current.delivered_amount;
            if delta >= current.per_cycle {
                self.capital += current.payout;
                self.sla_successes = self.sla_successes.saturating_add(1);
            } else {
                let miss_index = current.missed_cycles as usize;
                let penalty_mult = self.penalty_multiplier(miss_index);
                self.capital -= current.penalty * penalty_mult;
                self.sla_failures = self.sla_failures.saturating_add(1);
                if let Some(contract) = self.contracts.get_mut(&cid) {
                    contract.missed_cycles = contract.missed_cycles.saturating_add(1);
                }
            }

            if let Some(contract) = self.contracts.get_mut(&cid) {
                contract.delivered_amount = 0.0;
                contract.last_eval_cycle = self.cycle;
                if self.cycle >= u64::from(contract.total_cycles.max(1)) {
                    contract.completed = true;
                }
            }
        }
    }

    fn penalty_multiplier(&self, misses: usize) -> f64 {
        let curve = &self.config.pressure.sla_penalty_curve;
        let idx = misses.min(curve.len().saturating_sub(1));
        curve[idx]
    }

    fn run_economy_flow(&mut self) {
        let fuel_shock_factor = self
            .modifiers
            .iter()
            .filter(|m| m.risk == RiskStageA::FuelShock)
            .map(|m| m.magnitude)
            .fold(1.0_f64, f64::min);

        for market in self.markets.values_mut() {
            for (commodity, state) in &mut market.goods {
                let base_prod = match commodity {
                    Commodity::Ore | Commodity::Ice | Commodity::Gas => 1.8,
                    Commodity::Metal | Commodity::Fuel => 1.2,
                    Commodity::Parts | Commodity::Electronics => 0.9,
                };
                let base_cons = match commodity {
                    Commodity::Fuel => 1.5,
                    Commodity::Electronics => 1.2,
                    _ => 1.0,
                };

                let prod = if *commodity == Commodity::Fuel {
                    base_prod * fuel_shock_factor
                } else {
                    base_prod
                };
                let cons = base_cons;

                state.stock = (state.stock + prod - cons).max(0.0);
                state.cycle_inflow += prod;
                state.cycle_outflow += cons;
            }
        }
    }

    fn update_market_prices(&mut self) {
        for market in self.markets.values_mut() {
            for state in market.goods.values_mut() {
                let imbalance = (state.target_stock - state.stock) / state.target_stock.max(1.0);
                let flow_pressure =
                    (state.cycle_outflow - state.cycle_inflow) / state.target_stock.max(1.0);
                let raw_delta = self.config.market.k_stock * imbalance
                    + self.config.market.k_flow * flow_pressure;
                let delta = raw_delta
                    .max(-self.config.market.delta_cap)
                    .min(self.config.market.delta_cap);
                let floor = state.base_price * self.config.market.floor_mult;
                let ceil = state.base_price * self.config.market.ceiling_mult;
                state.price = (state.price * (1.0 + delta)).clamp(floor, ceil);
                state.cycle_inflow = 0.0;
                state.cycle_outflow = 0.0;
            }
        }
    }

    fn next_waypoint(&self, ship_id: ShipId) -> Option<SystemId> {
        let ship = self.ships.get(&ship_id)?;
        if ship.policy.waypoints.is_empty() {
            return None;
        }
        ship.policy
            .waypoints
            .get(ship.route_cursor % ship.policy.waypoints.len())
            .copied()
    }

    fn expire_modifiers(&mut self) {
        let tick = self.tick;
        let mut remaining = Vec::new();
        for modifier in self.modifiers.drain(..) {
            if tick < modifier.until_tick {
                remaining.push(modifier);
                continue;
            }

            if modifier.risk == RiskStageA::GateCongestion {
                if let Some(gate_id) = modifier.gate {
                    if let Some(edge) = self.world.edges.iter_mut().find(|e| e.id == gate_id) {
                        edge.capacity_factor = 1.0;
                    }
                }
            }
        }
        self.modifiers = remaining;
    }

    fn average_gate_load(&self) -> f64 {
        if self.gate_queue_load.is_empty() {
            return 0.0;
        }
        self.gate_queue_load.values().sum::<f64>() / self.gate_queue_load.len() as f64
    }

    fn average_price_index(&self) -> f64 {
        let mut sum = 0.0;
        let mut count = 0_usize;
        for market in self.markets.values() {
            for state in market.goods.values() {
                sum += state.price / state.base_price;
                count += 1;
            }
        }
        if count == 0 {
            1.0
        } else {
            sum / count as f64
        }
    }

    fn serialize_state(&self) -> String {
        // Snapshot format intentionally simple and deterministic.
        let mut edges = String::new();
        for edge in &self.world.edges {
            edges.push_str(&format!(
                "{}:{}:{}:{}:{},",
                edge.id.0, edge.a.0, edge.b.0, edge.capacity_factor, edge.blocked_until_tick
            ));
        }

        let mut ships = String::new();
        for ship in self.ships.values() {
            ships.push_str(&format!(
                "{}:{}:{}:{}:{}:{},",
                ship.id.0,
                ship.location.0,
                ship.eta_ticks_remaining,
                ship.route_cursor,
                ship.current_target.map_or(usize::MAX, |x| x.0),
                ship.last_risk_score
            ));
        }

        let mut contracts = String::new();
        for contract in self.contracts.values() {
            contracts.push_str(&format!(
                "{}:{:?}:{}:{}:{}:{}:{}:{}:{}:{}:{},",
                contract.id.0,
                contract.kind,
                contract.origin.0,
                contract.destination.0,
                contract.delivered_amount,
                contract.penalty,
                contract.missed_cycles,
                contract.completed as u8,
                contract.failed as u8,
                contract.deadline_tick,
                contract.last_eval_cycle
            ));
        }

        let mut markets = String::new();
        for (system_id, book) in &self.markets {
            for commodity in Commodity::ALL {
                if let Some(state) = book.goods.get(&commodity) {
                    markets.push_str(&format!(
                        "{}:{}:{}:{}:{}:{}:{},",
                        system_id.0,
                        commodity_code(commodity),
                        state.price,
                        state.stock,
                        state.cycle_inflow,
                        state.cycle_outflow,
                        state.target_stock
                    ));
                }
            }
        }

        let mut modifiers = String::new();
        for item in &self.modifiers {
            modifiers.push_str(&format!(
                "{}:{}:{}:{},",
                risk_code(item.risk),
                item.until_tick,
                item.gate.map_or(usize::MAX, |g| g.0),
                item.magnitude
            ));
        }

        format!(
            "tick={};cycle={};capital={};qdelay={};reroutes={};sla_s={};sla_f={};edges={};ships={};contracts={};markets={};modifiers={}",
            self.tick,
            self.cycle,
            self.capital,
            self.queue_delay_accumulator,
            self.reroute_count,
            self.sla_successes,
            self.sla_failures,
            edges,
            ships,
            contracts,
            markets,
            modifiers
        )
    }

    fn deserialize_state(state: &str, config: RuntimeConfig) -> Result<Self, SnapshotError> {
        let mut simulation = Simulation::new(config.clone(), config.galaxy.seed);

        let map = parse_semicolon_map(state);
        simulation.tick = parse_required_u64_from_map(&map, "tick")?;
        simulation.cycle = parse_required_u64_from_map(&map, "cycle")?;
        simulation.capital = parse_required_f64_from_map(&map, "capital")?;
        simulation.queue_delay_accumulator = parse_required_u64_from_map(&map, "qdelay")?;
        simulation.reroute_count = parse_required_u64_from_map(&map, "reroutes")?;
        simulation.sla_successes = parse_required_u64_from_map(&map, "sla_s")?;
        simulation.sla_failures = parse_required_u64_from_map(&map, "sla_f")?;

        if let Some(edges_blob) = map.get("edges") {
            for row in edges_blob.split(',').filter(|v| !v.is_empty()) {
                let parts: Vec<&str> = row.split(':').collect();
                if parts.len() != 5 {
                    return Err(SnapshotError::Parse(format!("bad edge row: {row}")));
                }
                let id: usize = parts[0]
                    .parse()
                    .map_err(|_| SnapshotError::Parse("edge id parse failed".to_string()))?;
                let cap: f64 = parts[3]
                    .parse()
                    .map_err(|_| SnapshotError::Parse("edge capacity parse failed".to_string()))?;
                let blocked_until_tick: u64 = parts[4]
                    .parse()
                    .map_err(|_| SnapshotError::Parse("edge blocked parse failed".to_string()))?;

                if let Some(edge) = simulation
                    .world
                    .edges
                    .iter_mut()
                    .find(|e| e.id == GateId(id))
                {
                    edge.capacity_factor = cap;
                    edge.blocked_until_tick = blocked_until_tick;
                }
            }
        }

        if let Some(ships_blob) = map.get("ships") {
            for row in ships_blob.split(',').filter(|v| !v.is_empty()) {
                let parts: Vec<&str> = row.split(':').collect();
                if parts.len() != 6 {
                    return Err(SnapshotError::Parse(format!("bad ship row: {row}")));
                }
                let id: usize = parts[0]
                    .parse()
                    .map_err(|_| SnapshotError::Parse("ship id parse failed".to_string()))?;
                if let Some(ship) = simulation.ships.get_mut(&ShipId(id)) {
                    ship.location =
                        SystemId(parts[1].parse().map_err(|_| {
                            SnapshotError::Parse("ship loc parse failed".to_string())
                        })?);
                    ship.eta_ticks_remaining = parts[2]
                        .parse()
                        .map_err(|_| SnapshotError::Parse("ship eta parse failed".to_string()))?;
                    ship.route_cursor = parts[3].parse().map_err(|_| {
                        SnapshotError::Parse("ship cursor parse failed".to_string())
                    })?;
                    let target_raw: usize = parts[4].parse().map_err(|_| {
                        SnapshotError::Parse("ship target parse failed".to_string())
                    })?;
                    ship.current_target = if target_raw == usize::MAX {
                        None
                    } else {
                        Some(SystemId(target_raw))
                    };
                    ship.last_risk_score = parts[5]
                        .parse()
                        .map_err(|_| SnapshotError::Parse("ship risk parse failed".to_string()))?;
                }
            }
        }

        if let Some(contracts_blob) = map.get("contracts") {
            for row in contracts_blob.split(',').filter(|v| !v.is_empty()) {
                let parts: Vec<&str> = row.split(':').collect();
                if parts.len() != 11 {
                    return Err(SnapshotError::Parse(format!("bad contract row: {row}")));
                }
                let id: usize = parts[0]
                    .parse()
                    .map_err(|_| SnapshotError::Parse("contract id parse failed".to_string()))?;
                if let Some(contract) = simulation.contracts.get_mut(&ContractId(id)) {
                    contract.delivered_amount = parts[4].parse().map_err(|_| {
                        SnapshotError::Parse("contract delivered parse failed".to_string())
                    })?;
                    contract.penalty = parts[5].parse().map_err(|_| {
                        SnapshotError::Parse("contract penalty parse failed".to_string())
                    })?;
                    contract.missed_cycles = parts[6].parse().map_err(|_| {
                        SnapshotError::Parse("contract misses parse failed".to_string())
                    })?;
                    contract.completed = parts[7] == "1";
                    contract.failed = parts[8] == "1";
                    contract.deadline_tick = parts[9].parse().map_err(|_| {
                        SnapshotError::Parse("contract deadline parse failed".to_string())
                    })?;
                    contract.last_eval_cycle = parts[10].parse().map_err(|_| {
                        SnapshotError::Parse("contract last_eval parse failed".to_string())
                    })?;
                }
            }
        }

        if let Some(markets_blob) = map.get("markets") {
            for row in markets_blob.split(',').filter(|v| !v.is_empty()) {
                let parts: Vec<&str> = row.split(':').collect();
                if parts.len() != 7 {
                    return Err(SnapshotError::Parse(format!("bad market row: {row}")));
                }
                let system_id: usize = parts[0].parse().map_err(|_| {
                    SnapshotError::Parse("market system id parse failed".to_string())
                })?;
                let commodity = commodity_from_code(parts[1]).ok_or_else(|| {
                    SnapshotError::Parse("market commodity parse failed".to_string())
                })?;
                let price: f64 = parts[2]
                    .parse()
                    .map_err(|_| SnapshotError::Parse("market price parse failed".to_string()))?;
                let stock: f64 = parts[3]
                    .parse()
                    .map_err(|_| SnapshotError::Parse("market stock parse failed".to_string()))?;
                let cycle_inflow: f64 = parts[4].parse().map_err(|_| {
                    SnapshotError::Parse("market cycle_inflow parse failed".to_string())
                })?;
                let cycle_outflow: f64 = parts[5].parse().map_err(|_| {
                    SnapshotError::Parse("market cycle_outflow parse failed".to_string())
                })?;
                let target_stock: f64 = parts[6].parse().map_err(|_| {
                    SnapshotError::Parse("market target_stock parse failed".to_string())
                })?;

                if let Some(book) = simulation.markets.get_mut(&SystemId(system_id)) {
                    if let Some(state) = book.goods.get_mut(&commodity) {
                        state.price = price;
                        state.stock = stock;
                        state.cycle_inflow = cycle_inflow;
                        state.cycle_outflow = cycle_outflow;
                        state.target_stock = target_stock;
                    }
                }
            }
        }

        simulation.modifiers.clear();
        if let Some(modifiers_blob) = map.get("modifiers") {
            for row in modifiers_blob.split(',').filter(|v| !v.is_empty()) {
                let parts: Vec<&str> = row.split(':').collect();
                if parts.len() != 4 {
                    return Err(SnapshotError::Parse(format!("bad modifier row: {row}")));
                }
                let risk = risk_from_code(parts[0]).ok_or_else(|| {
                    SnapshotError::Parse("modifier risk parse failed".to_string())
                })?;
                let until_tick: u64 = parts[1].parse().map_err(|_| {
                    SnapshotError::Parse("modifier until_tick parse failed".to_string())
                })?;
                let gate_raw: usize = parts[2]
                    .parse()
                    .map_err(|_| SnapshotError::Parse("modifier gate parse failed".to_string()))?;
                let magnitude: f64 = parts[3].parse().map_err(|_| {
                    SnapshotError::Parse("modifier magnitude parse failed".to_string())
                })?;

                simulation.modifiers.push(ActiveModifier {
                    until_tick,
                    gate: if gate_raw == usize::MAX {
                        None
                    } else {
                        Some(GateId(gate_raw))
                    },
                    risk,
                    magnitude,
                });
            }
        }

        Ok(simulation)
    }
}

#[derive(Debug)]
pub enum SnapshotError {
    Io(String),
    Parse(String),
}

impl Display for SnapshotError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(v) | Self::Parse(v) => write!(f, "{v}"),
        }
    }
}

impl std::error::Error for SnapshotError {}

#[derive(Debug, Clone)]
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

fn base_price_for(commodity: Commodity) -> f64 {
    match commodity {
        Commodity::Ore => 8.0,
        Commodity::Ice => 6.0,
        Commodity::Gas => 7.5,
        Commodity::Metal => 14.0,
        Commodity::Fuel => 16.0,
        Commodity::Parts => 25.0,
        Commodity::Electronics => 34.0,
    }
}

fn commodity_code(commodity: Commodity) -> &'static str {
    match commodity {
        Commodity::Ore => "ore",
        Commodity::Ice => "ice",
        Commodity::Gas => "gas",
        Commodity::Metal => "metal",
        Commodity::Fuel => "fuel",
        Commodity::Parts => "parts",
        Commodity::Electronics => "electronics",
    }
}

fn commodity_from_code(raw: &str) -> Option<Commodity> {
    match raw {
        "ore" => Some(Commodity::Ore),
        "ice" => Some(Commodity::Ice),
        "gas" => Some(Commodity::Gas),
        "metal" => Some(Commodity::Metal),
        "fuel" => Some(Commodity::Fuel),
        "parts" => Some(Commodity::Parts),
        "electronics" => Some(Commodity::Electronics),
        _ => None,
    }
}

fn risk_code(risk: RiskStageA) -> &'static str {
    match risk {
        RiskStageA::GateCongestion => "gate",
        RiskStageA::DockCongestion => "dock",
        RiskStageA::FuelShock => "fuel",
    }
}

fn risk_from_code(raw: &str) -> Option<RiskStageA> {
    match raw {
        "gate" => Some(RiskStageA::GateCongestion),
        "dock" => Some(RiskStageA::DockCongestion),
        "fuel" => Some(RiskStageA::FuelShock),
        _ => None,
    }
}

fn parse_simple_toml_map(input: &str) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for line in input.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        map.insert(key.trim().to_string(), value.trim().to_string());
    }
    map
}

fn parse_required_u32(map: &BTreeMap<String, String>, key: &str) -> Result<u32, ConfigError> {
    map.get(key)
        .ok_or_else(|| ConfigError::Parse(format!("missing key: {key}")))?
        .parse::<u32>()
        .map_err(|_| ConfigError::Parse(format!("bad u32 for key {key}")))
}

fn parse_required_u64(map: &BTreeMap<String, String>, key: &str) -> Result<u64, ConfigError> {
    map.get(key)
        .ok_or_else(|| ConfigError::Parse(format!("missing key: {key}")))?
        .parse::<u64>()
        .map_err(|_| ConfigError::Parse(format!("bad u64 for key {key}")))
}

fn parse_required_u8(map: &BTreeMap<String, String>, key: &str) -> Result<u8, ConfigError> {
    map.get(key)
        .ok_or_else(|| ConfigError::Parse(format!("missing key: {key}")))?
        .parse::<u8>()
        .map_err(|_| ConfigError::Parse(format!("bad u8 for key {key}")))
}

fn parse_required_f64(map: &BTreeMap<String, String>, key: &str) -> Result<f64, ConfigError> {
    map.get(key)
        .ok_or_else(|| ConfigError::Parse(format!("missing key: {key}")))?
        .trim_matches('"')
        .parse::<f64>()
        .map_err(|_| ConfigError::Parse(format!("bad f64 for key {key}")))
}

fn parse_required_f64_array(
    map: &BTreeMap<String, String>,
    key: &str,
) -> Result<Vec<f64>, ConfigError> {
    let raw = map
        .get(key)
        .ok_or_else(|| ConfigError::Parse(format!("missing key: {key}")))?;
    let trimmed = raw.trim();
    if !(trimmed.starts_with('[') && trimmed.ends_with(']')) {
        return Err(ConfigError::Parse(format!("expected array for key {key}")));
    }
    let body = &trimmed[1..trimmed.len().saturating_sub(1)];
    let mut values = Vec::new();
    for part in body.split(',').map(str::trim).filter(|v| !v.is_empty()) {
        values.push(
            part.parse::<f64>()
                .map_err(|_| ConfigError::Parse(format!("bad array item for key {key}")))?,
        );
    }
    if values.is_empty() {
        return Err(ConfigError::Parse(format!("empty array for key {key}")));
    }
    Ok(values)
}

fn extract_json_string_field(input: &str, field: &str) -> Option<String> {
    let needle = format!("\"{field}\":\"");
    let start = input.find(&needle)? + needle.len();
    let tail = &input[start..];
    let end = tail.find('"')?;
    Some(tail[..end].to_string())
}

fn parse_semicolon_map(input: &str) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for token in input.split(';') {
        if let Some((k, v)) = token.split_once('=') {
            map.insert(k.trim().to_string(), v.trim().to_string());
        }
    }
    map
}

fn parse_required_u64_from_map(
    map: &BTreeMap<String, String>,
    key: &str,
) -> Result<u64, SnapshotError> {
    map.get(key)
        .ok_or_else(|| SnapshotError::Parse(format!("missing key: {key}")))?
        .parse::<u64>()
        .map_err(|_| SnapshotError::Parse(format!("bad u64 key: {key}")))
}

fn parse_required_f64_from_map(
    map: &BTreeMap<String, String>,
    key: &str,
) -> Result<f64, SnapshotError> {
    map.get(key)
        .ok_or_else(|| SnapshotError::Parse(format!("missing key: {key}")))?
        .parse::<f64>()
        .map_err(|_| SnapshotError::Parse(format!("bad f64 key: {key}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    fn stage_a_config() -> RuntimeConfig {
        RuntimeConfig::load_from_dir(Path::new("../../assets/config/stage_a"))
            .expect("stage_a config should load")
    }

    #[test]
    fn generation_respects_cluster_and_connectivity_and_degree() {
        let cfg = stage_a_config();
        let sim = Simulation::new(cfg.clone(), cfg.galaxy.seed);
        let systems = sim.world.system_count();
        assert!(
            systems >= usize::from(cfg.galaxy.cluster_system_min)
                && systems <= usize::from(cfg.galaxy.cluster_system_max),
            "system count out of stage A bounds"
        );
        assert!(sim.world.is_connected(), "world graph must be connected");

        let degrees = sim.world.degree_map();
        for degree in degrees.values() {
            assert!(
                *degree >= usize::from(cfg.galaxy.min_degree)
                    && *degree <= usize::from(cfg.galaxy.max_degree),
                "node degree outside configured bounds"
            );
        }
    }

    #[test]
    fn gate_nodes_are_placed_on_system_boundary() {
        let sim = Simulation::new(stage_a_config(), 7);
        for system in &sim.world.systems {
            for gate in &system.gate_nodes {
                let dx = gate.x - system.x;
                let dy = gate.y - system.y;
                let distance = (dx * dx + dy * dy).sqrt();
                let eps = 1e-6;
                assert!(
                    (distance - system.radius).abs() < eps,
                    "gate must lie on system boundary"
                );
            }
        }
    }

    #[test]
    fn routing_supports_multihop_and_respects_max_hops() {
        let cfg = stage_a_config();
        let mut sim = Simulation::new(cfg, 3);
        let from = SystemId(0);
        let to = SystemId(sim.world.system_count() - 1);
        let ship_id = ShipId(0);

        if let Some(ship) = sim.ships.get_mut(&ship_id) {
            ship.location = from;
            ship.policy.max_hops = 16;
        }

        let route = sim
            .route_for_ship(ship_id, to)
            .expect("route should exist across connected graph");
        assert!(!route.segments.is_empty(), "route should contain hops");

        if let Some(ship) = sim.ships.get_mut(&ship_id) {
            ship.policy.max_hops = 1;
        }
        let maybe_too_short = sim.route_for_ship(ship_id, to);
        if from != to {
            assert!(
                maybe_too_short.is_none()
                    || maybe_too_short.expect("value checked").segments.len() <= 1,
                "max_hops must constrain routing"
            );
        }
    }

    #[test]
    fn reroute_happens_when_edge_blocked() {
        let cfg = stage_a_config();
        let mut sim = Simulation::new(cfg, 9);
        if sim.world.edges.len() < 2 {
            // Skip tiny graph edge-case while keeping the test deterministic.
            return;
        }

        let ship_id = ShipId(0);
        let destination = SystemId(sim.world.system_count() - 1);
        if let Some(ship) = sim.ships.get_mut(&ship_id) {
            ship.location = SystemId(0);
            ship.policy.waypoints = vec![SystemId(0), destination];
            ship.policy.max_hops = 16;
        }

        let baseline = sim
            .route_for_ship(ship_id, destination)
            .expect("baseline route should exist");
        let blocked_edge = baseline.segments[0]
            .edge
            .expect("warp segment should have edge id");
        sim.set_edge_blocked_until(blocked_edge, sim.tick + 1_000);

        let rerouted = sim.route_for_ship(ship_id, destination);
        assert!(rerouted.is_some(), "reroute path should exist");
        assert_ne!(
            rerouted
                .expect("checked")
                .segments
                .first()
                .and_then(|s| s.edge),
            Some(blocked_edge),
            "first hop should avoid blocked edge"
        );
    }

    #[test]
    fn delivery_penalty_curve_applies_without_hard_fail() {
        let mut cfg = stage_a_config();
        cfg.pressure.sla_penalty_curve = vec![1.0, 2.0, 3.0, 4.0];
        let mut sim = Simulation::new(cfg, 11);
        let start_capital = sim.capital;

        if let Some(contract) = sim.contracts.get_mut(&ContractId(0)) {
            contract.deadline_tick = 1;
            contract.assigned_ship = Some(ShipId(0));
            contract.destination = SystemId(sim.world.system_count() - 1);
        }
        if let Some(ship) = sim.ships.get_mut(&ShipId(0)) {
            ship.location = SystemId(0);
            ship.policy.waypoints = vec![SystemId(0)];
        }

        for _ in 0..5 {
            sim.step_tick();
        }

        let after_first_fail = sim.capital;
        assert!(
            after_first_fail < start_capital,
            "penalty should reduce capital"
        );

        // No hard run fail: simulation continues ticking.
        let tick_before = sim.tick;
        sim.step_tick();
        assert!(
            sim.tick > tick_before,
            "simulation should continue after SLA fail"
        );
    }

    #[test]
    fn supply_contract_tracks_cycle_shortfall_and_progressive_penalty() {
        let mut cfg = stage_a_config();
        cfg.pressure.sla_penalty_curve = vec![1.0, 1.5, 2.0];
        let mut sim = Simulation::new(cfg, 13);
        let cid = sim.create_supply_contract(SystemId(0), SystemId(1), 20.0, 3);
        let initial_capital = sim.capital;

        for _ in 0..(sim.config.time.cycle_ticks * 2) {
            sim.step_tick();
        }

        let contract = sim
            .contracts
            .get(&cid)
            .expect("supply contract should exist");
        assert!(contract.missed_cycles >= 1, "supply misses must accumulate");
        assert!(
            sim.capital < initial_capital,
            "misses should apply penalties"
        );
    }

    #[test]
    fn price_update_respects_delta_cap_and_floor_ceiling() {
        let cfg = stage_a_config();
        let mut sim = Simulation::new(cfg, 17);
        let sid = SystemId(0);
        let book = sim.markets.get_mut(&sid).expect("market should exist");
        let fuel = book
            .goods
            .get_mut(&Commodity::Fuel)
            .expect("fuel should exist");
        fuel.stock = 0.0;
        fuel.target_stock = 100.0;
        fuel.cycle_inflow = 0.0;
        fuel.cycle_outflow = 1000.0;
        let before = fuel.price;

        sim.update_market_prices();

        let after = sim
            .markets
            .get(&sid)
            .expect("market should exist")
            .goods
            .get(&Commodity::Fuel)
            .expect("fuel should exist")
            .price;

        let expected_max = before * (1.0 + sim.config.market.delta_cap);
        assert!(after <= expected_max + 1e-8, "delta cap must clamp rise");

        let floor = base_price_for(Commodity::Fuel) * sim.config.market.floor_mult;
        let ceil = base_price_for(Commodity::Fuel) * sim.config.market.ceiling_mult;
        assert!(
            after >= floor && after <= ceil,
            "price must stay in floor/ceiling"
        );
    }

    #[test]
    fn fuel_shock_increases_fuel_price_index() {
        let cfg = stage_a_config();
        let mut sim = Simulation::new(cfg, 19);
        let sid = SystemId(0);
        let before = sim
            .markets
            .get(&sid)
            .expect("market should exist")
            .goods
            .get(&Commodity::Fuel)
            .expect("fuel should exist")
            .price;

        sim.apply_event(RiskEvent::FuelShock {
            production_factor: 0.3,
            duration_ticks: sim.config.time.cycle_ticks,
        });

        for _ in 0..sim.config.time.cycle_ticks {
            sim.step_tick();
        }

        let after = sim
            .markets
            .get(&sid)
            .expect("market should exist")
            .goods
            .get(&Commodity::Fuel)
            .expect("fuel should exist")
            .price;
        assert!(after > before, "fuel shock should push fuel price upward");
    }

    #[test]
    fn congestion_changes_eta_and_risk() {
        let cfg = stage_a_config();
        let mut sim = Simulation::new(cfg, 23);
        if sim.world.system_count() < 2 {
            return;
        }
        let ship = ShipId(0);
        let destination = SystemId(1);
        let baseline = sim
            .route_for_ship(ship, destination)
            .expect("baseline route should exist");
        let edge = baseline.segments[0].edge.expect("must have edge");

        sim.apply_event(RiskEvent::GateCongestion {
            edge,
            capacity_factor: 0.2,
            duration_ticks: 200,
        });
        sim.step_tick();

        let after = sim
            .route_for_ship(ship, destination)
            .expect("route should still exist");

        assert!(
            after.eta_ticks >= baseline.eta_ticks,
            "congestion should not decrease eta"
        );
        assert!(
            after.risk_score >= baseline.risk_score,
            "congestion should not decrease risk"
        );
    }

    #[test]
    fn autopilot_loop_and_policy_change_affect_route() {
        let cfg = stage_a_config();
        let mut sim = Simulation::new(cfg, 29);
        if sim.world.system_count() < 3 {
            return;
        }

        let ship_id = ShipId(0);
        let last = SystemId(sim.world.system_count() - 1);

        if let Some(ship) = sim.ships.get_mut(&ship_id) {
            ship.policy.waypoints = vec![SystemId(0), SystemId(1), last];
            ship.policy.max_hops = 16;
            ship.location = SystemId(0);
            ship.route_cursor = 0;
        }

        for _ in 0..200 {
            sim.step_tick();
        }

        let cursor_after_loop = sim
            .ships
            .get(&ship_id)
            .expect("ship should exist")
            .route_cursor;
        assert!(
            cursor_after_loop < 3,
            "loop cursor must remain in waypoint bounds"
        );

        let route_before = sim
            .route_for_ship(ship_id, last)
            .expect("route before policy change");

        if let Some(ship) = sim.ships.get_mut(&ship_id) {
            ship.policy.max_hops = 1;
        }

        let route_after = sim.route_for_ship(ship_id, last);
        assert!(
            route_after.as_ref().is_none_or(|r| r.segments.len() <= 1),
            "policy max_hops must constrain route selection"
        );

        assert!(
            route_before.segments.len() >= route_after.as_ref().map_or(0, |r| r.segments.len()),
            "stricter policy should not increase route complexity"
        );
    }

    #[test]
    fn deterministic_seed_tick_run_produces_same_hash_and_reports() {
        let cfg = stage_a_config();
        let mut a = Simulation::new(cfg.clone(), 31);
        let mut b = Simulation::new(cfg, 31);

        let mut reports_a = Vec::new();
        let mut reports_b = Vec::new();

        for _ in 0..120 {
            reports_a.push(a.step_tick());
            reports_b.push(b.step_tick());
        }

        assert_eq!(reports_a, reports_b, "tick reports should be deterministic");
        assert_eq!(
            a.snapshot_hash(),
            b.snapshot_hash(),
            "snapshot hash should match"
        );
    }

    #[test]
    fn snapshot_round_trip_restores_future_ticks() {
        let cfg = stage_a_config();
        let mut base = Simulation::new(cfg.clone(), 37);
        for _ in 0..45 {
            base.step_tick();
        }

        let tmp = std::env::temp_dir().join("gatebound_stage_a_snapshot.json");
        base.save_snapshot(&tmp).expect("snapshot save should pass");

        let mut loaded = Simulation::load_snapshot(&tmp, cfg).expect("snapshot load should pass");

        let mut base_reports = Vec::new();
        let mut loaded_reports = Vec::new();
        for _ in 0..60 {
            base_reports.push(base.step_tick());
            loaded_reports.push(loaded.step_tick());
        }

        assert_eq!(
            base_reports, loaded_reports,
            "future simulation should match"
        );
    }

    #[test]
    fn stage_a_scope_guards_are_locked() {
        let cfg = stage_a_config();
        assert_eq!(cfg.time.cycle_ticks, 60, "cycle must be 60 ticks");

        let sim = Simulation::new(cfg, 41);
        for contract in sim.contracts.values() {
            assert!(
                matches!(
                    contract.kind,
                    ContractTypeStageA::Delivery | ContractTypeStageA::Supply
                ),
                "stage A must contain delivery/supply only"
            );
        }
    }

    #[test]
    fn market_intel_local_is_fresh_remote_is_stale() {
        let sim = Simulation::new(stage_a_config(), 43);
        let local = sim
            .market_intel(SystemId(0), true)
            .expect("local intel should be available");
        assert_eq!(local.staleness_ticks, 0);
        assert!((local.confidence - 1.0).abs() < 1e-9);

        let remote = sim
            .market_intel(SystemId(0), false)
            .expect("remote intel should be available");
        assert!(remote.staleness_ticks > 0);
        assert!(remote.confidence < 1.0);
    }

    #[test]
    fn benchmark_cluster_tick_latency_reports_percentiles() {
        let cfg = stage_a_config();
        let mut sim = Simulation::new(cfg, 47);

        let mut samples = Vec::new();
        for _ in 0..200 {
            let start = Instant::now();
            sim.step_tick();
            samples.push(start.elapsed().as_micros() as u64);
        }

        samples.sort_unstable();
        let p95_idx = ((samples.len() as f64) * 0.95).floor() as usize;
        let p99_idx = ((samples.len() as f64) * 0.99).floor() as usize;
        let p95 = samples[p95_idx.min(samples.len() - 1)];
        let p99 = samples[p99_idx.min(samples.len() - 1)];

        // We keep this generous to avoid flaky CI; this is a reporting gate.
        assert!(p95 < 200_000, "p95 tick latency should stay sane");
        assert!(p99 < 300_000, "p99 tick latency should stay sane");
    }
}
