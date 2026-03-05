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
    pub gate_fee_per_jump: f64,
    pub market_fee_rate: f64,
    pub market_depth_per_cycle: f64,
    pub offer_refresh_cycles: u32,
    pub offer_ttl_cycles: u32,
    pub milestone_capital_target: f64,
    pub milestone_market_share_target: f64,
    pub milestone_throughput_target_share: f64,
    pub milestone_reputation_target: f64,
    pub premium_offer_reputation_min: f64,
    pub lease_price_throughput_k: f64,
    pub lease_price_gate_k: f64,
    pub lease_price_congestion_k: f64,
    pub lease_price_min_mult: f64,
    pub lease_price_max_mult: f64,
    pub recovery_loan_base: f64,
    pub recovery_loan_buffer: f64,
    pub recovery_reputation_penalty: f64,
    pub recovery_rate_hike: f64,
    pub recovery_rate_max: f64,
    pub sla_penalty_curve: Vec<f64>,
}

impl Default for EconomyPressureConfig {
    fn default() -> Self {
        Self {
            loan_interest_rate: 0.02,
            ship_upkeep_per_tick: 0.5,
            slot_lease_cost: 2.0,
            gate_fee_per_jump: 0.4,
            market_fee_rate: 0.05,
            market_depth_per_cycle: 16.0,
            offer_refresh_cycles: 2,
            offer_ttl_cycles: 6,
            milestone_capital_target: 900.0,
            milestone_market_share_target: 0.25,
            milestone_throughput_target_share: 0.35,
            milestone_reputation_target: 0.85,
            premium_offer_reputation_min: 0.80,
            lease_price_throughput_k: 0.60,
            lease_price_gate_k: 0.35,
            lease_price_congestion_k: 0.80,
            lease_price_min_mult: 0.70,
            lease_price_max_mult: 2.50,
            recovery_loan_base: 120.0,
            recovery_loan_buffer: 20.0,
            recovery_reputation_penalty: 0.12,
            recovery_rate_hike: 0.01,
            recovery_rate_max: 0.12,
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
        cfg.pressure.gate_fee_per_jump = parse_required_f64(&pressure, "gate_fee_per_jump")?;
        cfg.pressure.market_fee_rate = parse_required_f64(&pressure, "market_fee_rate")?;
        cfg.pressure.market_depth_per_cycle =
            parse_required_f64(&pressure, "market_depth_per_cycle")?;
        cfg.pressure.offer_refresh_cycles = parse_required_u32(&pressure, "offer_refresh_cycles")?;
        cfg.pressure.offer_ttl_cycles = parse_required_u32(&pressure, "offer_ttl_cycles")?;
        cfg.pressure.milestone_capital_target =
            parse_required_f64(&pressure, "milestone_capital_target")?;
        cfg.pressure.milestone_market_share_target =
            parse_required_f64(&pressure, "milestone_market_share_target")?;
        cfg.pressure.milestone_throughput_target_share =
            parse_required_f64(&pressure, "milestone_throughput_target_share")?;
        cfg.pressure.milestone_reputation_target =
            parse_required_f64(&pressure, "milestone_reputation_target")?;
        cfg.pressure.premium_offer_reputation_min =
            parse_required_f64(&pressure, "premium_offer_reputation_min")?;
        cfg.pressure.lease_price_throughput_k =
            parse_required_f64(&pressure, "lease_price_throughput_k")?;
        cfg.pressure.lease_price_gate_k = parse_required_f64(&pressure, "lease_price_gate_k")?;
        cfg.pressure.lease_price_congestion_k =
            parse_required_f64(&pressure, "lease_price_congestion_k")?;
        cfg.pressure.lease_price_min_mult = parse_required_f64(&pressure, "lease_price_min_mult")?;
        cfg.pressure.lease_price_max_mult = parse_required_f64(&pressure, "lease_price_max_mult")?;
        cfg.pressure.recovery_loan_base = parse_required_f64(&pressure, "recovery_loan_base")?;
        cfg.pressure.recovery_loan_buffer = parse_required_f64(&pressure, "recovery_loan_buffer")?;
        cfg.pressure.recovery_reputation_penalty =
            parse_required_f64(&pressure, "recovery_reputation_penalty")?;
        cfg.pressure.recovery_rate_hike = parse_required_f64(&pressure, "recovery_rate_hike")?;
        cfg.pressure.recovery_rate_max = parse_required_f64(&pressure, "recovery_rate_max")?;
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
        if self.pressure.market_fee_rate < 0.0 || self.pressure.market_fee_rate >= 1.0 {
            return Err(ConfigError::Validation(
                "market_fee_rate must be in [0,1)".to_string(),
            ));
        }
        if self.pressure.market_depth_per_cycle <= 0.0 {
            return Err(ConfigError::Validation(
                "market_depth_per_cycle must be > 0".to_string(),
            ));
        }
        if self.pressure.offer_refresh_cycles == 0 || self.pressure.offer_ttl_cycles == 0 {
            return Err(ConfigError::Validation(
                "offer cycles must be > 0".to_string(),
            ));
        }
        if self.pressure.milestone_throughput_target_share < 0.0
            || self.pressure.milestone_throughput_target_share > 1.0
        {
            return Err(ConfigError::Validation(
                "milestone_throughput_target_share must be in [0,1]".to_string(),
            ));
        }
        if self.pressure.milestone_market_share_target < 0.0
            || self.pressure.milestone_market_share_target > 1.0
        {
            return Err(ConfigError::Validation(
                "milestone_market_share_target must be in [0,1]".to_string(),
            ));
        }
        if self.pressure.premium_offer_reputation_min < 0.0
            || self.pressure.premium_offer_reputation_min > 1.0
        {
            return Err(ConfigError::Validation(
                "premium_offer_reputation_min must be in [0,1]".to_string(),
            ));
        }
        if self.pressure.lease_price_min_mult <= 0.0
            || self.pressure.lease_price_max_mult < self.pressure.lease_price_min_mult
        {
            return Err(ConfigError::Validation(
                "lease price multipliers invalid".to_string(),
            ));
        }
        if self.pressure.recovery_rate_hike < 0.0
            || self.pressure.recovery_rate_max < self.pressure.loan_interest_rate
        {
            return Err(ConfigError::Validation(
                "recovery rate bounds invalid".to_string(),
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
    pub from_anchor: Option<StationId>,
    pub to_anchor: Option<StationId>,
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
pub struct SnapshotV2 {
    pub version: u32,
    pub state: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SlotType {
    Dock,
    Storage,
    Factory,
    Market,
}

impl SlotType {
    pub const ALL: [SlotType; 4] = [
        SlotType::Dock,
        SlotType::Storage,
        SlotType::Factory,
        SlotType::Market,
    ];
}

#[derive(Debug, Clone, PartialEq)]
pub struct LeasePosition {
    pub system_id: SystemId,
    pub slot_type: SlotType,
    pub cycles_remaining: u32,
    pub price_per_cycle: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LeaseMarketView {
    pub system_id: SystemId,
    pub slot_type: SlotType,
    pub available: u32,
    pub total: u32,
    pub price_per_cycle: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeaseError {
    NoCapacity,
    InvalidCycles,
    UnknownSystem,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompanyArchetype {
    Player,
    Hauler,
    Miner,
    Industrial,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Company {
    pub id: CompanyId,
    pub name: String,
    pub archetype: CompanyArchetype,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OfferProblemTag {
    HighRisk,
    CongestedRoute,
    LowMargin,
    FuelVolatility,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ContractOffer {
    pub id: u64,
    pub kind: ContractTypeStageA,
    pub origin: SystemId,
    pub destination: SystemId,
    pub origin_station: StationId,
    pub destination_station: StationId,
    pub quantity: f64,
    pub payout: f64,
    pub penalty: f64,
    pub eta_ticks: u32,
    pub risk_score: f64,
    pub margin_estimate: f64,
    pub route_gate_ids: Vec<GateId>,
    pub problem_tag: OfferProblemTag,
    pub premium: bool,
    pub profit_per_ton: f64,
    pub expires_cycle: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OfferError {
    UnknownOffer,
    ExpiredOffer,
    ShipBusy,
    InvalidAssignment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FleetWarning {
    HighRisk,
    HighQueueDelay,
    NoRoute,
    ShipIdle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FleetJobKind {
    Pickup,
    Transit,
    GateQueue,
    Warp,
    Unload,
    LoopReturn,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FleetJobStep {
    pub kind: FleetJobKind,
    pub system: SystemId,
    pub eta_ticks: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FleetShipStatus {
    pub ship_id: ShipId,
    pub company_id: CompanyId,
    pub location: SystemId,
    pub target: Option<SystemId>,
    pub eta: u32,
    pub active_contract: Option<ContractId>,
    pub route_len: usize,
    pub reroutes: u64,
    pub warning: Option<FleetWarning>,
    pub job_queue: Vec<FleetJobStep>,
    pub idle_ticks_cycle: u32,
    pub avg_delay_ticks_cycle: f64,
    pub profit_per_run: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MilestoneId {
    Capital,
    MarketShare,
    ThroughputControl,
    Reputation,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MilestoneStatus {
    pub id: MilestoneId,
    pub current: f64,
    pub target: f64,
    pub completed: bool,
    pub completed_cycle: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GateThroughputSnapshot {
    pub gate_id: GateId,
    pub player_share: f64,
    pub total_flow: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MarketInsightRow {
    pub commodity: Commodity,
    pub trend_delta: f64,
    pub forecast_next: f64,
    pub imbalance_factor: f64,
    pub congestion_factor: f64,
    pub fuel_factor: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RecoveryAction {
    pub cycle: u64,
    pub released_leases: u32,
    pub capital_after: f64,
    pub debt_after: f64,
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
                from_anchor: None,
                to_anchor: None,
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StationAnchor {
    pub id: StationId,
    pub system_id: SystemId,
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct World {
    pub systems: Vec<SystemNode>,
    pub edges: Vec<GateEdge>,
    pub adjacency: BTreeMap<SystemId, Vec<(SystemId, GateId)>>,
    pub stations: Vec<StationAnchor>,
    pub stations_by_system: BTreeMap<SystemId, Vec<StationId>>,
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

        let mut stations = Vec::with_capacity(system_count.saturating_mul(2));
        let mut stations_by_system: BTreeMap<SystemId, Vec<StationId>> = BTreeMap::new();
        for system in &systems {
            let mut station_rng =
                DeterministicRng::new(seed ^ ((system.id.0 as u64).wrapping_mul(0x9E37_79B9)));
            for radius_mult in [0.32_f64, 0.56_f64] {
                let angle = station_rng.next_f64() * std::f64::consts::TAU;
                let radius = system.radius * radius_mult;
                let station_id = StationId(stations.len());
                stations.push(StationAnchor {
                    id: station_id,
                    system_id: system.id,
                    x: system.x + angle.cos() * radius,
                    y: system.y + angle.sin() * radius,
                });
                stations_by_system
                    .entry(system.id)
                    .or_default()
                    .push(station_id);
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
    pub origin_station: StationId,
    pub destination_station: StationId,
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
    pub sub_light_speed: f64,
    pub movement_queue: VecDeque<RouteSegment>,
    pub segment_eta_remaining: u32,
    pub segment_progress_total: u32,
    pub current_segment_kind: Option<SegmentKind>,
    pub active_contract: Option<ContractId>,
    pub route_cursor: usize,
    pub policy: AutopilotPolicy,
    pub planned_path: Vec<SystemId>,
    pub current_target: Option<SystemId>,
    pub last_gate_arrival: Option<GateId>,
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
    pub companies: BTreeMap<CompanyId, Company>,
    pub markets: BTreeMap<SystemId, MarketBook>,
    pub contracts: BTreeMap<ContractId, Contract>,
    pub contract_offers: BTreeMap<u64, ContractOffer>,
    pub next_offer_id: u64,
    pub ships: BTreeMap<ShipId, Ship>,
    pub milestones: Vec<MilestoneStatus>,
    pub capital: f64,
    pub active_leases: Vec<LeasePosition>,
    pub outstanding_debt: f64,
    pub reputation: f64,
    pub current_loan_interest_rate: f64,
    pub recovery_events: u32,
    pub gate_traversals_cycle: BTreeMap<GateId, BTreeMap<CompanyId, u32>>,
    pub gate_traversals_window: VecDeque<BTreeMap<GateId, BTreeMap<CompanyId, u32>>>,
    pub queue_delay_accumulator: u64,
    pub reroute_count: u64,
    pub sla_successes: u64,
    pub sla_failures: u64,
    pub gate_queue_load: BTreeMap<GateId, f64>,
    pub ship_idle_ticks_cycle: BTreeMap<ShipId, u32>,
    pub ship_delay_ticks_cycle: BTreeMap<ShipId, u32>,
    pub ship_runs_completed: BTreeMap<ShipId, u32>,
    pub ship_profit_earned: BTreeMap<ShipId, f64>,
    pub previous_cycle_prices: BTreeMap<(SystemId, Commodity), f64>,
    pub recovery_log: Vec<RecoveryAction>,
    modifiers: Vec<ActiveModifier>,
}

impl Simulation {
    pub fn new(config: RuntimeConfig, seed: u64) -> Self {
        let initial_loan_interest_rate = config.pressure.loan_interest_rate;
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
        let mut previous_cycle_prices = BTreeMap::new();
        for (system_id, book) in &markets {
            for commodity in Commodity::ALL {
                if let Some(state) = book.goods.get(&commodity) {
                    previous_cycle_prices.insert((*system_id, commodity), state.price);
                }
            }
        }

        let companies = seed_stage_a_companies();
        let mut ships = seed_stage_a_ships(&world);
        let mut contracts = BTreeMap::new();
        if world.system_count() >= 2 && ships.contains_key(&ShipId(0)) {
            let origin_station = world.first_station(SystemId(0)).unwrap_or(StationId(0));
            let destination_station = world.first_station(SystemId(1)).unwrap_or(origin_station);
            contracts.insert(
                ContractId(0),
                Contract {
                    id: ContractId(0),
                    kind: ContractTypeStageA::Delivery,
                    origin: SystemId(0),
                    destination: SystemId(1),
                    origin_station,
                    destination_station,
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
            if let Some(player_ship) = ships.get_mut(&ShipId(0)) {
                player_ship.active_contract = Some(ContractId(0));
            }
        }

        let mut simulation = Self {
            config,
            world,
            tick: 0,
            cycle: 0,
            companies,
            markets,
            contracts,
            contract_offers: BTreeMap::new(),
            next_offer_id: 0,
            ships,
            milestones: Vec::new(),
            capital: 500.0,
            active_leases: Vec::new(),
            outstanding_debt: 0.0,
            reputation: 1.0,
            current_loan_interest_rate: initial_loan_interest_rate,
            recovery_events: 0,
            gate_traversals_cycle: BTreeMap::new(),
            gate_traversals_window: VecDeque::new(),
            queue_delay_accumulator: 0,
            reroute_count: 0,
            sla_successes: 0,
            sla_failures: 0,
            gate_queue_load: BTreeMap::new(),
            ship_idle_ticks_cycle: BTreeMap::new(),
            ship_delay_ticks_cycle: BTreeMap::new(),
            ship_runs_completed: BTreeMap::new(),
            ship_profit_earned: BTreeMap::new(),
            previous_cycle_prices,
            recovery_log: Vec::new(),
            modifiers: Vec::new(),
        };
        simulation.milestones = vec![
            MilestoneStatus {
                id: MilestoneId::Capital,
                current: simulation.capital,
                target: simulation.config.pressure.milestone_capital_target,
                completed: false,
                completed_cycle: None,
            },
            MilestoneStatus {
                id: MilestoneId::MarketShare,
                current: 0.0,
                target: simulation.config.pressure.milestone_market_share_target,
                completed: false,
                completed_cycle: None,
            },
            MilestoneStatus {
                id: MilestoneId::ThroughputControl,
                current: 0.0,
                target: simulation.config.pressure.milestone_throughput_target_share,
                completed: false,
                completed_cycle: None,
            },
            MilestoneStatus {
                id: MilestoneId::Reputation,
                current: simulation.reputation,
                target: simulation.config.pressure.milestone_reputation_target,
                completed: false,
                completed_cycle: None,
            },
        ];
        simulation.refresh_contract_offers();
        simulation
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
        self.capture_previous_cycle_prices();
        self.update_market_prices();
        self.evaluate_supply_contracts();
        self.advance_lease_cycle();
        self.apply_cycle_financial_pressure();
        self.roll_gate_traversal_window();
        self.expire_contract_offers();
        if self
            .cycle
            .is_multiple_of(u64::from(self.config.pressure.offer_refresh_cycles.max(1)))
        {
            self.refresh_contract_offers();
        }
        self.update_milestones();

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
        let json = format!("{{\"version\":2,\"state\":\"{state}\"}}\n");
        fs::write(path, json).map_err(|e| SnapshotError::Io(format!("save failed: {e}")))
    }

    pub fn load_snapshot(path: &Path, config: RuntimeConfig) -> Result<Self, SnapshotError> {
        let payload =
            fs::read_to_string(path).map_err(|e| SnapshotError::Io(format!("load failed: {e}")))?;
        let version = extract_json_u32_field(&payload, "version")
            .ok_or_else(|| SnapshotError::Parse("missing version field".to_string()))?;
        if version != 1 && version != 2 {
            return Err(SnapshotError::Parse(format!(
                "unsupported snapshot version: {version}"
            )));
        }
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
        let origin_station = self.world.first_station(ship.location)?;
        let destination_station = self.world.first_station(destination)?;
        self.build_station_route_with_speed(
            origin_station,
            destination_station,
            ship.policy.clone(),
            ship.sub_light_speed,
        )
    }

    pub fn station_of_contract(&self, contract_id: ContractId) -> Option<(StationId, StationId)> {
        self.contracts
            .get(&contract_id)
            .map(|contract| (contract.origin_station, contract.destination_station))
    }

    pub fn station_position(&self, station_id: StationId) -> Option<(f64, f64)> {
        self.world.station_coords(station_id)
    }

    pub fn build_station_route(
        &self,
        origin_station: StationId,
        destination_station: StationId,
        policy: AutopilotPolicy,
    ) -> Option<RoutePlan> {
        self.build_station_route_with_speed(origin_station, destination_station, policy, 18.0)
    }

    pub fn create_supply_contract(
        &mut self,
        origin: SystemId,
        destination: SystemId,
        per_cycle: f64,
        total_cycles: u32,
    ) -> ContractId {
        let next_id = ContractId(self.contracts.len());
        let origin_station = self.world.first_station(origin).unwrap_or(StationId(0));
        let destination_station = self
            .world
            .first_station(destination)
            .unwrap_or(origin_station);
        self.contracts.insert(
            next_id,
            Contract {
                id: next_id,
                kind: ContractTypeStageA::Supply,
                origin,
                destination,
                origin_station,
                destination_station,
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

    pub fn refresh_contract_offers(&mut self) {
        let mut offers = BTreeMap::new();
        let system_ids: Vec<SystemId> = self.world.systems.iter().map(|system| system.id).collect();

        for window in system_ids.windows(2) {
            let origin = window[0];
            let destination = window[1];
            self.maybe_push_offer(
                origin,
                destination,
                ContractTypeStageA::Delivery,
                &mut offers,
            );
            self.maybe_push_offer(destination, origin, ContractTypeStageA::Supply, &mut offers);
        }

        self.contract_offers = offers;
    }

    pub fn accept_contract_offer(
        &mut self,
        offer_id: u64,
        ship_id: ShipId,
    ) -> Result<ContractId, OfferError> {
        let Some(offer) = self.contract_offers.get(&offer_id).cloned() else {
            return Err(OfferError::UnknownOffer);
        };
        if offer.expires_cycle < self.cycle {
            self.contract_offers.remove(&offer_id);
            return Err(OfferError::ExpiredOffer);
        }
        let Some(ship_snapshot) = self.ships.get(&ship_id).cloned() else {
            return Err(OfferError::InvalidAssignment);
        };
        if ship_snapshot.company_id != CompanyId(0) {
            return Err(OfferError::InvalidAssignment);
        }
        if ship_snapshot.active_contract.is_some() || ship_snapshot.eta_ticks_remaining > 0 {
            return Err(OfferError::ShipBusy);
        }

        let contract_id = ContractId(
            self.contracts
                .keys()
                .map(|id| id.0)
                .max()
                .unwrap_or(0)
                .saturating_add(1),
        );
        let is_supply = offer.kind == ContractTypeStageA::Supply;
        let cycle_ticks = u64::from(self.config.time.cycle_ticks.max(1));
        self.contracts.insert(
            contract_id,
            Contract {
                id: contract_id,
                kind: offer.kind,
                origin: offer.origin,
                destination: offer.destination,
                origin_station: offer.origin_station,
                destination_station: offer.destination_station,
                quantity: offer.quantity,
                deadline_tick: self
                    .tick
                    .saturating_add(u64::from(offer.eta_ticks).saturating_add(cycle_ticks * 3)),
                per_cycle: if is_supply { offer.quantity } else { 0.0 },
                total_cycles: if is_supply { 6 } else { 0 },
                payout: offer.payout,
                penalty: offer.penalty,
                assigned_ship: Some(ship_id),
                delivered_amount: 0.0,
                missed_cycles: 0,
                completed: false,
                failed: false,
                last_eval_cycle: self.cycle,
            },
        );
        if let Some(ship) = self.ships.get_mut(&ship_id) {
            ship.active_contract = Some(contract_id);
            ship.route_cursor = 0;
            ship.policy.waypoints = vec![ship.location, offer.destination];
        }

        self.contract_offers.remove(&offer_id);
        Ok(contract_id)
    }

    pub fn fleet_status(&self) -> Vec<FleetShipStatus> {
        let high_queue = self
            .gate_queue_load
            .values()
            .copied()
            .fold(0.0_f64, f64::max)
            > 1.0;
        let mut status = self
            .ships
            .values()
            .map(|ship| {
                let warning = if ship.eta_ticks_remaining == 0 && ship.active_contract.is_none() {
                    Some(FleetWarning::ShipIdle)
                } else if ship.active_contract.is_some()
                    && ship.current_target.is_none()
                    && ship.planned_path.is_empty()
                {
                    Some(FleetWarning::NoRoute)
                } else if ship.last_risk_score >= 1.0 {
                    Some(FleetWarning::HighRisk)
                } else if high_queue {
                    Some(FleetWarning::HighQueueDelay)
                } else {
                    None
                };

                FleetShipStatus {
                    ship_id: ship.id,
                    company_id: ship.company_id,
                    location: ship.location,
                    target: ship.current_target,
                    eta: ship.eta_ticks_remaining,
                    active_contract: ship.active_contract,
                    route_len: ship.planned_path.len(),
                    reroutes: ship.reroutes,
                    warning,
                    job_queue: self.project_ship_job_queue(ship),
                    idle_ticks_cycle: self
                        .ship_idle_ticks_cycle
                        .get(&ship.id)
                        .copied()
                        .unwrap_or(0),
                    avg_delay_ticks_cycle: {
                        let delay = self
                            .ship_delay_ticks_cycle
                            .get(&ship.id)
                            .copied()
                            .unwrap_or(0) as f64;
                        let runs =
                            self.ship_runs_completed.get(&ship.id).copied().unwrap_or(0) as f64;
                        if runs > 0.0 {
                            delay / runs
                        } else {
                            delay
                        }
                    },
                    profit_per_run: {
                        let profit = self
                            .ship_profit_earned
                            .get(&ship.id)
                            .copied()
                            .unwrap_or(0.0);
                        let runs =
                            self.ship_runs_completed.get(&ship.id).copied().unwrap_or(0) as f64;
                        if runs > 0.0 {
                            profit / runs
                        } else {
                            0.0
                        }
                    },
                }
            })
            .collect::<Vec<_>>();
        status.sort_by_key(|entry| entry.ship_id.0);
        status
    }

    pub fn gate_throughput_view(&self) -> Vec<GateThroughputSnapshot> {
        self.world
            .edges
            .iter()
            .map(|edge| {
                let mut total = 0_u32;
                let mut player = 0_u32;
                for cycle_map in &self.gate_traversals_window {
                    if let Some(by_company) = cycle_map.get(&edge.id) {
                        for (company_id, count) in by_company {
                            total = total.saturating_add(*count);
                            if *company_id == CompanyId(0) {
                                player = player.saturating_add(*count);
                            }
                        }
                    }
                }
                let player_share = if total == 0 {
                    0.0
                } else {
                    player as f64 / total as f64
                };
                GateThroughputSnapshot {
                    gate_id: edge.id,
                    player_share,
                    total_flow: total,
                }
            })
            .collect()
    }

    pub fn milestone_status(&self) -> &[MilestoneStatus] {
        &self.milestones
    }

    pub fn market_share_view(&self) -> f64 {
        let mut player_total = 0_u64;
        let mut world_total = 0_u64;
        for cycle_map in &self.gate_traversals_window {
            for by_company in cycle_map.values() {
                for (company_id, count) in by_company {
                    world_total = world_total.saturating_add(u64::from(*count));
                    if *company_id == CompanyId(0) {
                        player_total = player_total.saturating_add(u64::from(*count));
                    }
                }
            }
        }
        if world_total == 0 {
            0.0
        } else {
            player_total as f64 / world_total as f64
        }
    }

    pub fn market_insights(&self, system_id: SystemId) -> Vec<MarketInsightRow> {
        let Some(book) = self.markets.get(&system_id) else {
            return Vec::new();
        };
        let congestion_factor = self.system_congestion_signal(system_id);
        let fuel_factor = self
            .modifiers
            .iter()
            .filter(|m| m.risk == RiskStageA::FuelShock)
            .map(|m| 1.0 - m.magnitude)
            .fold(0.0_f64, f64::max);
        let mut rows = Vec::new();
        for commodity in Commodity::ALL {
            let Some(state) = book.goods.get(&commodity) else {
                continue;
            };
            let prev_price = self
                .previous_cycle_prices
                .get(&(system_id, commodity))
                .copied()
                .unwrap_or(state.price);
            let trend_delta = state.price - prev_price;
            let imbalance = (state.target_stock - state.stock) / state.target_stock.max(1.0);
            let flow_pressure =
                (state.cycle_outflow - state.cycle_inflow) / state.target_stock.max(1.0);
            let raw_delta =
                self.config.market.k_stock * imbalance + self.config.market.k_flow * flow_pressure;
            let delta =
                raw_delta.clamp(-self.config.market.delta_cap, self.config.market.delta_cap);
            let floor = state.base_price * self.config.market.floor_mult;
            let ceil = state.base_price * self.config.market.ceiling_mult;
            let forecast_next = (state.price * (1.0 + delta)).clamp(floor, ceil);
            rows.push(MarketInsightRow {
                commodity,
                trend_delta,
                forecast_next,
                imbalance_factor: imbalance,
                congestion_factor,
                fuel_factor,
            });
        }
        rows
    }

    pub fn recent_recovery_actions(&self) -> &[RecoveryAction] {
        &self.recovery_log
    }

    pub fn lease_slot(
        &mut self,
        system_id: SystemId,
        slot_type: SlotType,
        cycles: u32,
    ) -> Result<(), LeaseError> {
        if cycles == 0 {
            return Err(LeaseError::InvalidCycles);
        }
        if !self
            .world
            .systems
            .iter()
            .any(|system| system.id == system_id)
        {
            return Err(LeaseError::UnknownSystem);
        }

        let total = self.total_slots_for(slot_type);
        let used = self
            .active_leases
            .iter()
            .filter(|lease| lease.system_id == system_id && lease.slot_type == slot_type)
            .count() as u32;
        if used >= total {
            return Err(LeaseError::NoCapacity);
        }

        let price_per_cycle = self.lease_price_for(system_id, slot_type);
        self.active_leases.push(LeasePosition {
            system_id,
            slot_type,
            cycles_remaining: cycles,
            price_per_cycle,
        });
        Ok(())
    }

    pub fn release_one_slot(&mut self, system_id: SystemId, slot_type: SlotType) -> bool {
        if let Some(idx) = self
            .active_leases
            .iter()
            .position(|lease| lease.system_id == system_id && lease.slot_type == slot_type)
        {
            self.active_leases.remove(idx);
            return true;
        }
        false
    }

    pub fn lease_market_for_system(&self, system_id: SystemId) -> Vec<LeaseMarketView> {
        if !self
            .world
            .systems
            .iter()
            .any(|system| system.id == system_id)
        {
            return Vec::new();
        }

        SlotType::ALL
            .into_iter()
            .map(|slot_type| {
                let total = self.total_slots_for(slot_type);
                let used = self
                    .active_leases
                    .iter()
                    .filter(|lease| lease.system_id == system_id && lease.slot_type == slot_type)
                    .count() as u32;
                LeaseMarketView {
                    system_id,
                    slot_type,
                    available: total.saturating_sub(used),
                    total,
                    price_per_cycle: self.lease_price_for(system_id, slot_type),
                }
            })
            .collect()
    }

    fn apply_upkeep(&mut self) {
        let ship_upkeep = self.config.pressure.ship_upkeep_per_tick * self.ships.len() as f64;
        let cycle_ticks = f64::from(self.config.time.cycle_ticks.max(1));
        let lease_upkeep = self
            .active_leases
            .iter()
            .map(|lease| lease.price_per_cycle / cycle_ticks)
            .sum::<f64>();
        self.capital -= ship_upkeep + lease_upkeep;
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
            let mut completed_segment = None;
            if let Some(ship) = self.ships.get_mut(&ship_id) {
                if ship.segment_eta_remaining > 0 {
                    ship.segment_eta_remaining = ship.segment_eta_remaining.saturating_sub(1);
                    ship.eta_ticks_remaining = ship.segment_eta_remaining;
                    if ship.segment_eta_remaining == 0 {
                        completed_segment = ship.movement_queue.pop_front();
                        ship.current_segment_kind = None;
                        ship.current_target = None;
                        ship.segment_progress_total = 0;
                    } else {
                        continue;
                    }
                }
            }
            if let Some(segment) = completed_segment {
                if let Some(ship) = self.ships.get_mut(&ship_id) {
                    ship.location = segment.to;
                    if segment.kind == SegmentKind::Warp {
                        ship.last_gate_arrival = segment.edge;
                    }
                }
            }

            self.start_next_movement_segment(ship_id, dock_delay_factor);

            let Some(ship_snapshot) = self.ships.get(&ship_id).cloned() else {
                continue;
            };
            if ship_snapshot.segment_eta_remaining > 0 || !ship_snapshot.movement_queue.is_empty() {
                continue;
            }

            if ship_snapshot.active_contract.is_none() {
                let idle_ticks = self.ship_idle_ticks_cycle.entry(ship_id).or_insert(0);
                *idle_ticks = idle_ticks
                    .saturating_add(1)
                    .min(self.config.time.cycle_ticks.max(1));
            }

            let (target_system, target_station) =
                if let Some(contract_id) = ship_snapshot.active_contract {
                    let Some(contract) = self.contracts.get(&contract_id) else {
                        continue;
                    };
                    (contract.destination, contract.destination_station)
                } else {
                    let Some(target) = self.next_waypoint(ship_id) else {
                        continue;
                    };
                    let Some(target_station) = self.world.first_station(target) else {
                        continue;
                    };
                    (target, target_station)
                };

            if ship_snapshot.active_contract.is_some() && ship_snapshot.location == target_system {
                continue;
            }

            if ship_snapshot.active_contract.is_none() && ship_snapshot.location == target_system {
                if let Some(ship) = self.ships.get_mut(&ship_id) {
                    if !ship.policy.waypoints.is_empty() {
                        ship.route_cursor = (ship.route_cursor + 1) % ship.policy.waypoints.len();
                    }
                }
                continue;
            }

            let origin_station = self
                .world
                .first_station(ship_snapshot.location)
                .unwrap_or(target_station);
            let route = match self.build_station_route_with_speed(
                origin_station,
                target_station,
                ship_snapshot.policy.clone(),
                ship_snapshot.sub_light_speed,
            ) {
                Some(route) => route,
                None => {
                    if let Some(ship) = self.ships.get_mut(&ship_id) {
                        ship.reroutes = ship.reroutes.saturating_add(1);
                    }
                    self.reroute_count = self.reroute_count.saturating_add(1);
                    continue;
                }
            };

            if route.risk_score > ship_snapshot.policy.max_risk_score {
                continue;
            }

            if let Some(ship) = self.ships.get_mut(&ship_id) {
                ship.last_risk_score = route.risk_score;
                ship.movement_queue = VecDeque::from(route.segments.clone());
                ship.planned_path = route.segments.iter().map(|segment| segment.to).collect();
                ship.segment_eta_remaining = 0;
                ship.segment_progress_total = 0;
                ship.current_segment_kind = None;
                ship.current_target = None;
                ship.eta_ticks_remaining = 0;
                ship.last_gate_arrival = None;
            }
            self.start_next_movement_segment(ship_id, dock_delay_factor);
        }
    }

    fn start_next_movement_segment(&mut self, ship_id: ShipId, dock_delay_factor: f64) {
        loop {
            let Some(segment) = self
                .ships
                .get(&ship_id)
                .and_then(|ship| ship.movement_queue.front().cloned())
            else {
                if let Some(ship) = self.ships.get_mut(&ship_id) {
                    ship.segment_eta_remaining = 0;
                    ship.segment_progress_total = 0;
                    ship.current_segment_kind = None;
                    ship.current_target = None;
                    ship.eta_ticks_remaining = 0;
                }
                return;
            };

            let mut eta = segment.eta_ticks;
            if segment.kind == SegmentKind::GateQueue {
                if let Some(edge) = segment.edge {
                    *self.gate_queue_load.entry(edge).or_insert(0.0) += 1.0;
                    let queue_delay = self.gate_queue_eta(edge);
                    self.queue_delay_accumulator = self
                        .queue_delay_accumulator
                        .saturating_add(u64::from(queue_delay));
                    let delay_ticks = self.ship_delay_ticks_cycle.entry(ship_id).or_insert(0);
                    *delay_ticks = delay_ticks
                        .saturating_add(queue_delay)
                        .min(self.config.time.cycle_ticks.max(1) * 4);
                    eta = eta.saturating_add(queue_delay);
                }
                eta = eta.saturating_add(dock_delay_factor.ceil() as u32);
            }

            if segment.kind == SegmentKind::Warp {
                if let Some(edge) = segment.edge {
                    let company_id = self
                        .ships
                        .get(&ship_id)
                        .map(|ship| ship.company_id)
                        .unwrap_or(CompanyId(0));
                    self.capital -= self.config.pressure.gate_fee_per_jump;
                    self.record_gate_traversal(edge, company_id);
                }
            }

            if let Some(ship) = self.ships.get_mut(&ship_id) {
                if segment.from_anchor.is_some() {
                    ship.last_gate_arrival = None;
                }
                ship.current_segment_kind = Some(segment.kind);
                ship.current_target = Some(segment.to);
                ship.segment_progress_total = eta;
                ship.segment_eta_remaining = eta;
                ship.eta_ticks_remaining = eta;
            }

            if eta > 0 {
                return;
            }

            if let Some(ship) = self.ships.get_mut(&ship_id) {
                ship.location = segment.to;
                if segment.kind == SegmentKind::Warp {
                    ship.last_gate_arrival = segment.edge;
                }
                ship.movement_queue.pop_front();
                ship.current_target = None;
                ship.current_segment_kind = None;
                ship.segment_progress_total = 0;
                ship.segment_eta_remaining = 0;
                ship.eta_ticks_remaining = 0;
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
                        .map(|s| {
                            s.location == snapshot.destination
                                && s.eta_ticks_remaining == 0
                                && s.movement_queue.is_empty()
                                && s.current_segment_kind.is_none()
                        })
                        .unwrap_or(false);

                    if arrived {
                        if let Some(c) = self.contracts.get_mut(&cid) {
                            c.completed = true;
                            c.delivered_amount = c.quantity;
                        }
                        if let Some(ship) = self.ships.get_mut(&ship_id) {
                            ship.active_contract = None;
                        }
                        let net_payout = self.apply_market_fee(snapshot.payout);
                        self.capital += net_payout;
                        self.record_ship_profit(ship_id, net_payout);
                        self.sla_successes = self.sla_successes.saturating_add(1);
                    } else if self.tick > snapshot.deadline_tick {
                        let penalty_mult = self.penalty_multiplier(snapshot.missed_cycles as usize);
                        self.capital -= snapshot.penalty * penalty_mult;
                        if let Some(c) = self.contracts.get_mut(&cid) {
                            c.failed = true;
                            c.missed_cycles = c.missed_cycles.saturating_add(1);
                        }
                        if let Some(ship) = self.ships.get_mut(&ship_id) {
                            ship.active_contract = None;
                        }
                        self.sla_failures = self.sla_failures.saturating_add(1);
                    }
                }
                ContractTypeStageA::Supply => {
                    let delivered = snapshot.assigned_ship.and_then(|ship_id| {
                        self.ships.get(&ship_id).map(|s| {
                            s.location == snapshot.destination
                                && s.eta_ticks_remaining == 0
                                && s.movement_queue.is_empty()
                                && s.current_segment_kind.is_none()
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

            let delta = current
                .delivered_amount
                .min(self.config.pressure.market_depth_per_cycle);
            if delta >= current.per_cycle {
                let net_payout = self.apply_market_fee(current.payout);
                self.capital += net_payout;
                if let Some(ship_id) = current.assigned_ship {
                    self.record_ship_profit(ship_id, net_payout);
                }
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
                    if let Some(ship_id) = contract.assigned_ship {
                        if let Some(ship) = self.ships.get_mut(&ship_id) {
                            ship.active_contract = None;
                        }
                    }
                }
            }
        }
    }

    fn apply_market_fee(&self, gross: f64) -> f64 {
        gross * (1.0 - self.config.pressure.market_fee_rate)
    }

    fn record_gate_traversal(&mut self, gate_id: GateId, company_id: CompanyId) {
        let by_company = self.gate_traversals_cycle.entry(gate_id).or_default();
        let count = by_company.entry(company_id).or_insert(0);
        *count = count.saturating_add(1);
    }

    fn roll_gate_traversal_window(&mut self) {
        self.gate_traversals_window
            .push_back(self.gate_traversals_cycle.clone());
        self.gate_traversals_cycle.clear();
        let max_len = usize::try_from(self.config.time.rolling_window_cycles).unwrap_or(1);
        while self.gate_traversals_window.len() > max_len {
            self.gate_traversals_window.pop_front();
        }
    }

    fn expire_contract_offers(&mut self) {
        self.contract_offers
            .retain(|_, offer| offer.expires_cycle >= self.cycle);
    }

    fn maybe_push_offer(
        &mut self,
        origin: SystemId,
        destination: SystemId,
        kind: ContractTypeStageA,
        offers: &mut BTreeMap<u64, ContractOffer>,
    ) {
        if origin == destination {
            return;
        }
        let origin_station = self.world.first_station(origin).unwrap_or(StationId(0));
        let destination_station = self
            .world
            .first_station(destination)
            .unwrap_or(origin_station);
        let Some(route) = self.build_station_route_with_speed(
            origin_station,
            destination_station,
            AutopilotPolicy {
                max_hops: 16,
                ..AutopilotPolicy::default()
            },
            18.0,
        ) else {
            return;
        };
        let eta_ticks = route.eta_ticks;
        let risk_score = route.risk_score;
        let route_gate_ids = route
            .segments
            .iter()
            .filter(|segment| segment.kind == SegmentKind::Warp)
            .filter_map(|segment| segment.edge)
            .collect::<Vec<_>>();
        let Some(destination_market) = self.markets.get(&destination) else {
            return;
        };

        let imbalance = destination_market
            .goods
            .values()
            .map(|state| {
                ((state.target_stock - state.stock) / state.target_stock.max(1.0)).max(0.0)
            })
            .sum::<f64>()
            / destination_market.goods.len().max(1) as f64;
        let flow_pressure = destination_market
            .goods
            .values()
            .map(|state| (state.cycle_outflow - state.cycle_inflow).max(0.0))
            .sum::<f64>()
            / destination_market.goods.len().max(1) as f64;

        let quantity = (8.0 + imbalance * 12.0 + flow_pressure * 0.8).clamp(5.0, 30.0);
        let payout = 18.0 + quantity * 2.2 + eta_ticks as f64 * 0.3;
        let penalty = (payout * 0.45).max(8.0);
        let margin_estimate = payout
            - f64::from(eta_ticks) * 0.15
            - risk_score * 10.0
            - self.config.pressure.gate_fee_per_jump;
        let profit_per_ton = margin_estimate / quantity.max(1.0);
        let route_is_congested = route_gate_ids.iter().any(|gate_id| {
            let load = self.gate_queue_load.get(gate_id).copied().unwrap_or(0.0);
            let effective_capacity = self
                .world
                .edges
                .iter()
                .find(|edge| edge.id == *gate_id)
                .map(|edge| (edge.base_capacity * edge.capacity_factor).max(1.0))
                .unwrap_or(1.0);
            load / effective_capacity > 0.95
        });
        let fuel_ratio = destination_market
            .goods
            .get(&Commodity::Fuel)
            .map(|state| state.stock / state.target_stock.max(1.0))
            .unwrap_or(1.0);
        let problem_tag = if risk_score >= 1.0 {
            OfferProblemTag::HighRisk
        } else if route_is_congested {
            OfferProblemTag::CongestedRoute
        } else if fuel_ratio < 0.75 {
            OfferProblemTag::FuelVolatility
        } else {
            OfferProblemTag::LowMargin
        };
        let premium = self.reputation >= self.config.pressure.premium_offer_reputation_min;
        let offer = ContractOffer {
            id: self.next_offer_id,
            kind,
            origin,
            destination,
            origin_station,
            destination_station,
            quantity,
            payout,
            penalty,
            eta_ticks,
            risk_score,
            margin_estimate,
            route_gate_ids,
            problem_tag,
            premium,
            profit_per_ton,
            expires_cycle: self
                .cycle
                .saturating_add(u64::from(self.config.pressure.offer_ttl_cycles.max(1))),
        };
        offers.insert(self.next_offer_id, offer);
        self.next_offer_id = self.next_offer_id.saturating_add(1);
    }

    fn build_station_route_with_speed(
        &self,
        origin_station: StationId,
        destination_station: StationId,
        policy: AutopilotPolicy,
        sub_light_speed: f64,
    ) -> Option<RoutePlan> {
        let origin_anchor = self
            .world
            .stations
            .iter()
            .find(|station| station.id == origin_station)?;
        let destination_anchor = self
            .world
            .stations
            .iter()
            .find(|station| station.id == destination_station)?;

        let request = RoutingRequest {
            origin: origin_anchor.system_id,
            destination: destination_anchor.system_id,
            policy,
        };
        let graph = self.world.to_graph_view(self.tick, &self.gate_queue_load);
        let system_route = RoutingService::plan_route(&graph, &request).ok()?;

        let mut segments = Vec::new();
        let mut eta_total = 0_u32;
        let mut risk_total = 0.0_f64;

        let mut cursor_system = origin_anchor.system_id;
        let mut cursor_x = origin_anchor.x;
        let mut cursor_y = origin_anchor.y;
        let mut cursor_anchor = Some(origin_anchor.id);

        for hop in &system_route.segments {
            let gate_id = hop.edge?;
            let (exit_x, exit_y) = self.world.gate_coords(hop.from, gate_id)?;
            let in_eta =
                self.in_system_eta_ticks(cursor_x, cursor_y, exit_x, exit_y, sub_light_speed);
            segments.push(RouteSegment {
                from: cursor_system,
                to: hop.from,
                from_anchor: cursor_anchor,
                to_anchor: None,
                edge: Some(gate_id),
                kind: SegmentKind::InSystem,
                eta_ticks: in_eta,
                risk: 0.0,
            });
            eta_total = eta_total.saturating_add(in_eta);

            let queue_eta = self.gate_queue_eta(gate_id);
            let queue_risk = self.gate_risk(gate_id);
            segments.push(RouteSegment {
                from: hop.from,
                to: hop.from,
                from_anchor: None,
                to_anchor: None,
                edge: Some(gate_id),
                kind: SegmentKind::GateQueue,
                eta_ticks: queue_eta,
                risk: queue_risk,
            });
            eta_total = eta_total.saturating_add(queue_eta);
            risk_total += queue_risk;

            segments.push(RouteSegment {
                from: hop.from,
                to: hop.to,
                from_anchor: None,
                to_anchor: None,
                edge: Some(gate_id),
                kind: SegmentKind::Warp,
                eta_ticks: 0,
                risk: 0.0,
            });

            let (entry_x, entry_y) = self.world.gate_coords(hop.to, gate_id)?;
            cursor_system = hop.to;
            cursor_x = entry_x;
            cursor_y = entry_y;
            cursor_anchor = None;
        }

        let final_eta = self.in_system_eta_ticks(
            cursor_x,
            cursor_y,
            destination_anchor.x,
            destination_anchor.y,
            sub_light_speed,
        );
        segments.push(RouteSegment {
            from: cursor_system,
            to: destination_anchor.system_id,
            from_anchor: cursor_anchor,
            to_anchor: Some(destination_anchor.id),
            edge: None,
            kind: SegmentKind::InSystem,
            eta_ticks: final_eta,
            risk: 0.0,
        });
        eta_total = eta_total.saturating_add(final_eta);

        Some(RoutePlan {
            segments,
            eta_ticks: eta_total,
            risk_score: risk_total,
        })
    }

    fn update_milestones(&mut self) {
        let throughput_current = self
            .gate_throughput_view()
            .into_iter()
            .map(|snapshot| snapshot.player_share)
            .fold(0.0_f64, f64::max);
        let market_share_current = self.market_share_view();

        for milestone in &mut self.milestones {
            milestone.current = match milestone.id {
                MilestoneId::Capital => self.capital,
                MilestoneId::MarketShare => market_share_current,
                MilestoneId::ThroughputControl => throughput_current,
                MilestoneId::Reputation => self.reputation,
            };
            if !milestone.completed && milestone.current >= milestone.target {
                milestone.completed = true;
                milestone.completed_cycle = Some(self.cycle);
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

    fn capture_previous_cycle_prices(&mut self) {
        self.previous_cycle_prices.clear();
        for (system_id, book) in &self.markets {
            for commodity in Commodity::ALL {
                if let Some(state) = book.goods.get(&commodity) {
                    self.previous_cycle_prices
                        .insert((*system_id, commodity), state.price);
                }
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

    fn advance_lease_cycle(&mut self) {
        for lease in &mut self.active_leases {
            lease.cycles_remaining = lease.cycles_remaining.saturating_sub(1);
        }
        self.active_leases
            .retain(|lease| lease.cycles_remaining > 0);
    }

    fn apply_cycle_financial_pressure(&mut self) {
        if self.outstanding_debt > 0.0 {
            self.capital -= self.outstanding_debt * self.current_loan_interest_rate;
        }

        if self.capital > 0.0 && self.outstanding_debt > 0.0 {
            let repayment = (self.capital * 0.2).min(self.outstanding_debt);
            self.capital -= repayment;
            self.outstanding_debt -= repayment;
        }

        if self.capital < 0.0 {
            let emergency_loan = self
                .config
                .pressure
                .recovery_loan_base
                .max(-self.capital + self.config.pressure.recovery_loan_buffer);
            self.capital += emergency_loan;
            self.outstanding_debt += emergency_loan;
            let mut released_leases = 0_u32;
            if !self.active_leases.is_empty() {
                let mut indices = self.active_leases.iter().enumerate().collect::<Vec<_>>();
                indices.sort_by(|(_, a), (_, b)| b.price_per_cycle.total_cmp(&a.price_per_cycle));
                let mut to_remove = indices
                    .into_iter()
                    .take(2)
                    .map(|(idx, _)| idx)
                    .collect::<Vec<_>>();
                to_remove.sort_unstable_by(|a, b| b.cmp(a));
                for idx in to_remove {
                    if idx < self.active_leases.len() {
                        self.active_leases.remove(idx);
                        released_leases = released_leases.saturating_add(1);
                    }
                }
            }
            self.reputation =
                (self.reputation - self.config.pressure.recovery_reputation_penalty).max(0.0);
            self.current_loan_interest_rate = (self.current_loan_interest_rate
                + self.config.pressure.recovery_rate_hike)
                .min(self.config.pressure.recovery_rate_max);
            self.recovery_events = self.recovery_events.saturating_add(1);
            self.recovery_log.push(RecoveryAction {
                cycle: self.cycle,
                released_leases,
                capital_after: self.capital,
                debt_after: self.outstanding_debt,
            });
            if self.recovery_log.len() > 16 {
                let extra = self.recovery_log.len() - 16;
                self.recovery_log.drain(0..extra);
            }
        }
    }

    fn lease_price_for(&self, system_id: SystemId, slot_type: SlotType) -> f64 {
        let base = self.config.pressure.slot_lease_cost * slot_multiplier(slot_type);

        let throughput_signal = self
            .markets
            .get(&system_id)
            .map(|book| {
                if book.goods.is_empty() {
                    0.0
                } else {
                    book.goods
                        .values()
                        .map(|state| state.cycle_inflow + state.cycle_outflow)
                        .sum::<f64>()
                        / book.goods.len() as f64
                        / 100.0
                }
            })
            .unwrap_or(0.0);

        let max_degree = self
            .world
            .adjacency
            .values()
            .map(Vec::len)
            .max()
            .unwrap_or(1) as f64;
        let degree = self
            .world
            .adjacency
            .get(&system_id)
            .map(Vec::len)
            .unwrap_or(0) as f64;
        let gate_proximity_signal = if max_degree <= 0.0 {
            0.0
        } else {
            degree / max_degree
        };

        let congestion_signal = self.system_congestion_signal(system_id);

        let price_mult_raw = 1.0
            + self.config.pressure.lease_price_throughput_k * throughput_signal
            + self.config.pressure.lease_price_gate_k * gate_proximity_signal
            + self.config.pressure.lease_price_congestion_k * congestion_signal;
        let price_mult = price_mult_raw.clamp(
            self.config.pressure.lease_price_min_mult,
            self.config.pressure.lease_price_max_mult,
        );

        base * price_mult
    }

    fn total_slots_for(&self, slot_type: SlotType) -> u32 {
        match slot_type {
            SlotType::Dock => 4,
            SlotType::Storage => 6,
            SlotType::Factory => 3,
            SlotType::Market => 2,
        }
    }

    fn system_congestion_signal(&self, system_id: SystemId) -> f64 {
        let Some(edges) = self.world.adjacency.get(&system_id) else {
            return 0.0;
        };
        if edges.is_empty() {
            return 0.0;
        }

        edges
            .iter()
            .map(|(_, gate_id)| {
                let load = self.gate_queue_load.get(gate_id).copied().unwrap_or(0.0);
                let effective_capacity = self
                    .world
                    .edges
                    .iter()
                    .find(|edge| edge.id == *gate_id)
                    .map(|edge| (edge.base_capacity * edge.capacity_factor).max(1.0))
                    .unwrap_or(1.0);
                load / effective_capacity
            })
            .sum::<f64>()
            / edges.len() as f64
    }

    fn gate_queue_eta(&self, gate_id: GateId) -> u32 {
        let load = self.gate_queue_load.get(&gate_id).copied().unwrap_or(0.0);
        let effective_capacity = self
            .world
            .edges
            .iter()
            .find(|edge| edge.id == gate_id)
            .map(|edge| (edge.base_capacity * edge.capacity_factor).max(1.0))
            .unwrap_or(1.0);
        (load / effective_capacity).ceil() as u32
    }

    fn gate_risk(&self, gate_id: GateId) -> f64 {
        let load = self.gate_queue_load.get(&gate_id).copied().unwrap_or(0.0);
        let effective_capacity = self
            .world
            .edges
            .iter()
            .find(|edge| edge.id == gate_id)
            .map(|edge| (edge.base_capacity * edge.capacity_factor).max(1.0))
            .unwrap_or(1.0);
        load / effective_capacity
    }

    fn in_system_eta_ticks(
        &self,
        from_x: f64,
        from_y: f64,
        to_x: f64,
        to_y: f64,
        sub_light_speed: f64,
    ) -> u32 {
        let speed = sub_light_speed.max(0.1);
        let dx = to_x - from_x;
        let dy = to_y - from_y;
        let distance = (dx * dx + dy * dy).sqrt();
        (distance / speed).ceil().max(1.0) as u32
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

    fn record_ship_profit(&mut self, ship_id: ShipId, net_payout: f64) {
        let runs = self.ship_runs_completed.entry(ship_id).or_insert(0);
        *runs = runs.saturating_add(1);
        let profit = self.ship_profit_earned.entry(ship_id).or_insert(0.0);
        *profit += net_payout;
    }

    fn project_ship_job_queue(&self, ship: &Ship) -> Vec<FleetJobStep> {
        let mut queue = Vec::new();
        if let Some(contract_id) = ship.active_contract {
            if let Some(contract) = self.contracts.get(&contract_id) {
                queue.push(FleetJobStep {
                    kind: FleetJobKind::Pickup,
                    system: contract.origin,
                    eta_ticks: 0,
                });
            }
        }
        let mut eta_cursor = ship.segment_eta_remaining;
        for (idx, segment) in ship.movement_queue.iter().enumerate() {
            let step_kind = match segment.kind {
                SegmentKind::InSystem => FleetJobKind::Transit,
                SegmentKind::GateQueue => FleetJobKind::GateQueue,
                SegmentKind::Warp => FleetJobKind::Warp,
                SegmentKind::Dock => FleetJobKind::Unload,
            };
            if idx > 0 {
                eta_cursor = eta_cursor.saturating_add(segment.eta_ticks);
            }
            queue.push(FleetJobStep {
                kind: step_kind,
                system: segment.to,
                eta_ticks: eta_cursor,
            });
        }
        if let Some(contract_id) = ship.active_contract {
            if let Some(contract) = self.contracts.get(&contract_id) {
                queue.push(FleetJobStep {
                    kind: FleetJobKind::Unload,
                    system: contract.destination,
                    eta_ticks: eta_cursor,
                });
            }
        }
        if let Some(loop_target) = ship.policy.waypoints.first().copied() {
            queue.push(FleetJobStep {
                kind: FleetJobKind::LoopReturn,
                system: loop_target,
                eta_ticks: eta_cursor.saturating_add(1),
            });
        }
        queue
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

        let mut ship_runtime = String::new();
        for ship in self.ships.values() {
            let queue_encoded = if ship.movement_queue.is_empty() {
                "none".to_string()
            } else {
                ship.movement_queue
                    .iter()
                    .map(encode_route_segment)
                    .collect::<Vec<_>>()
                    .join("|")
            };
            ship_runtime.push_str(&format!(
                "{}:{}:{}:{}:{}:{}:{},",
                ship.id.0,
                ship.sub_light_speed,
                ship.segment_eta_remaining,
                ship.segment_progress_total,
                ship.current_segment_kind
                    .map(segment_kind_code)
                    .unwrap_or("none"),
                queue_encoded,
                ship.last_gate_arrival.map_or(usize::MAX, |gate| gate.0)
            ));
        }

        let mut stations = String::new();
        for station in &self.world.stations {
            stations.push_str(&format!(
                "{}:{}:{}:{},",
                station.id.0, station.system_id.0, station.x, station.y
            ));
        }

        let mut contracts = String::new();
        for contract in self.contracts.values() {
            contracts.push_str(&format!(
                "{}:{:?}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{},",
                contract.id.0,
                contract.kind,
                contract.origin.0,
                contract.destination.0,
                contract.origin_station.0,
                contract.destination_station.0,
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

        let mut leases = String::new();
        for lease in &self.active_leases {
            leases.push_str(&format!(
                "{}:{}:{}:{},",
                lease.system_id.0,
                slot_type_code(lease.slot_type),
                lease.cycles_remaining,
                lease.price_per_cycle
            ));
        }

        let mut companies = String::new();
        for company in self.companies.values() {
            companies.push_str(&format!(
                "{}:{}:{},",
                company.id.0,
                company_archetype_code(company.archetype),
                sanitize_snapshot_text(&company.name)
            ));
        }

        let mut offers = String::new();
        for offer in self.contract_offers.values() {
            let route_gate_ids = if offer.route_gate_ids.is_empty() {
                "none".to_string()
            } else {
                offer
                    .route_gate_ids
                    .iter()
                    .map(|gate_id| gate_id.0.to_string())
                    .collect::<Vec<_>>()
                    .join("-")
            };
            offers.push_str(&format!(
                "{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{},",
                offer.id,
                contract_type_code(offer.kind),
                offer.origin.0,
                offer.destination.0,
                offer.origin_station.0,
                offer.destination_station.0,
                offer.quantity,
                offer.payout,
                offer.penalty,
                offer.eta_ticks,
                offer.risk_score,
                offer.margin_estimate,
                offer_problem_code(offer.problem_tag),
                offer.premium as u8,
                offer.profit_per_ton,
                route_gate_ids,
                offer.expires_cycle
            ));
        }

        let mut milestones = String::new();
        for milestone in &self.milestones {
            milestones.push_str(&format!(
                "{}:{}:{}:{}:{},",
                milestone_id_code(milestone.id),
                milestone.current,
                milestone.target,
                milestone.completed as u8,
                milestone.completed_cycle.unwrap_or(u64::MAX)
            ));
        }

        let gate_cycle = encode_gate_counts_map(&self.gate_traversals_cycle);
        let gate_window = self
            .gate_traversals_window
            .iter()
            .map(encode_gate_counts_map)
            .collect::<Vec<_>>()
            .join("|");

        let mut ship_kpis = String::new();
        for ship_id in self.ships.keys() {
            ship_kpis.push_str(&format!(
                "{}:{}:{}:{}:{},",
                ship_id.0,
                self.ship_idle_ticks_cycle
                    .get(ship_id)
                    .copied()
                    .unwrap_or(0),
                self.ship_delay_ticks_cycle
                    .get(ship_id)
                    .copied()
                    .unwrap_or(0),
                self.ship_runs_completed.get(ship_id).copied().unwrap_or(0),
                self.ship_profit_earned.get(ship_id).copied().unwrap_or(0.0)
            ));
        }

        let mut recovery_log = String::new();
        for action in &self.recovery_log {
            recovery_log.push_str(&format!(
                "{}:{}:{}:{},",
                action.cycle, action.released_leases, action.capital_after, action.debt_after
            ));
        }

        let mut prev_prices = String::new();
        for ((system_id, commodity), price) in &self.previous_cycle_prices {
            prev_prices.push_str(&format!(
                "{}:{}:{},",
                system_id.0,
                commodity_code(*commodity),
                price
            ));
        }

        let mut gate_loads = String::new();
        for (gate_id, load) in &self.gate_queue_load {
            gate_loads.push_str(&format!("{}:{},", gate_id.0, load));
        }

        format!(
            "tick={};cycle={};capital={};debt={};reputation={};loan_rate={};recovery_events={};qdelay={};reroutes={};sla_s={};sla_f={};next_offer_id={};edges={};stations={};ships={};ship_runtime={};contracts={};markets={};modifiers={};leases={};companies={};offers={};milestones={};gate_cycle={};gate_window={};ship_kpis={};recovery_log={};prev_prices={};gate_loads={}",
            self.tick,
            self.cycle,
            self.capital,
            self.outstanding_debt,
            self.reputation,
            self.current_loan_interest_rate,
            self.recovery_events,
            self.queue_delay_accumulator,
            self.reroute_count,
            self.sla_successes,
            self.sla_failures,
            self.next_offer_id,
            edges,
            stations,
            ships,
            ship_runtime,
            contracts,
            markets,
            modifiers,
            leases,
            companies,
            offers,
            milestones,
            gate_cycle,
            gate_window,
            ship_kpis,
            recovery_log,
            prev_prices,
            gate_loads
        )
    }

    fn deserialize_state(state: &str, config: RuntimeConfig) -> Result<Self, SnapshotError> {
        let mut simulation = Simulation::new(config.clone(), config.galaxy.seed);

        let map = parse_semicolon_map(state);
        simulation.tick = parse_required_u64_from_map(&map, "tick")?;
        simulation.cycle = parse_required_u64_from_map(&map, "cycle")?;
        simulation.capital = parse_required_f64_from_map(&map, "capital")?;
        simulation.outstanding_debt = parse_optional_f64_from_map(&map, "debt").unwrap_or(0.0);
        simulation.reputation = parse_optional_f64_from_map(&map, "reputation").unwrap_or(1.0);
        simulation.current_loan_interest_rate = parse_optional_f64_from_map(&map, "loan_rate")
            .unwrap_or(simulation.config.pressure.loan_interest_rate);
        simulation.recovery_events =
            parse_optional_u64_from_map(&map, "recovery_events").unwrap_or(0) as u32;
        simulation.queue_delay_accumulator = parse_required_u64_from_map(&map, "qdelay")?;
        simulation.reroute_count = parse_required_u64_from_map(&map, "reroutes")?;
        simulation.sla_successes = parse_required_u64_from_map(&map, "sla_s")?;
        simulation.sla_failures = parse_required_u64_from_map(&map, "sla_f")?;
        simulation.next_offer_id =
            parse_optional_u64_from_map(&map, "next_offer_id").unwrap_or(simulation.next_offer_id);

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

        if let Some(stations_blob) = map.get("stations") {
            simulation.world.stations.clear();
            simulation.world.stations_by_system.clear();
            for row in stations_blob.split(',').filter(|value| !value.is_empty()) {
                let parts: Vec<&str> = row.split(':').collect();
                if parts.len() != 4 {
                    return Err(SnapshotError::Parse(format!("bad station row: {row}")));
                }
                let station_id: usize = parts[0]
                    .parse()
                    .map_err(|_| SnapshotError::Parse("station id parse failed".to_string()))?;
                let system_id: usize = parts[1]
                    .parse()
                    .map_err(|_| SnapshotError::Parse("station system parse failed".to_string()))?;
                let x: f64 = parts[2]
                    .parse()
                    .map_err(|_| SnapshotError::Parse("station x parse failed".to_string()))?;
                let y: f64 = parts[3]
                    .parse()
                    .map_err(|_| SnapshotError::Parse("station y parse failed".to_string()))?;
                let station = StationAnchor {
                    id: StationId(station_id),
                    system_id: SystemId(system_id),
                    x,
                    y,
                };
                simulation
                    .world
                    .stations_by_system
                    .entry(station.system_id)
                    .or_default()
                    .push(station.id);
                simulation.world.stations.push(station);
            }
        }

        if let Some(ships_blob) = map.get("ships") {
            for row in ships_blob.split(',').filter(|v| !v.is_empty()) {
                let parts: Vec<&str> = row.split(':').collect();
                if parts.len() < 6 {
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
                    ship.segment_eta_remaining = ship.eta_ticks_remaining;
                    ship.segment_progress_total = ship.eta_ticks_remaining;
                    ship.current_segment_kind = if ship.segment_eta_remaining > 0 {
                        Some(SegmentKind::InSystem)
                    } else {
                        None
                    };
                    ship.movement_queue.clear();
                    ship.sub_light_speed = 18.0;
                    ship.last_gate_arrival = None;
                }
            }
        }

        if let Some(runtime_blob) = map.get("ship_runtime") {
            for row in runtime_blob.split(',').filter(|v| !v.is_empty()) {
                let parts: Vec<&str> = row.split(':').collect();
                if parts.len() != 6 && parts.len() != 7 {
                    return Err(SnapshotError::Parse(format!("bad ship_runtime row: {row}")));
                }
                let ship_id: usize = parts[0].parse().map_err(|_| {
                    SnapshotError::Parse("ship_runtime id parse failed".to_string())
                })?;
                let Some(ship) = simulation.ships.get_mut(&ShipId(ship_id)) else {
                    continue;
                };
                ship.sub_light_speed = parts[1].parse().map_err(|_| {
                    SnapshotError::Parse("ship_runtime speed parse failed".to_string())
                })?;
                ship.segment_eta_remaining = parts[2].parse().map_err(|_| {
                    SnapshotError::Parse("ship_runtime eta parse failed".to_string())
                })?;
                ship.segment_progress_total = parts[3].parse().map_err(|_| {
                    SnapshotError::Parse("ship_runtime progress parse failed".to_string())
                })?;
                ship.current_segment_kind = if parts[4] == "none" {
                    None
                } else {
                    Some(segment_kind_from_code(parts[4]).ok_or_else(|| {
                        SnapshotError::Parse("ship_runtime kind parse failed".to_string())
                    })?)
                };
                ship.movement_queue.clear();
                if parts[5] != "none" {
                    for encoded in parts[5].split('|').filter(|value| !value.is_empty()) {
                        ship.movement_queue
                            .push_back(decode_route_segment(encoded)?);
                    }
                }
                ship.last_gate_arrival = if parts.len() == 7 {
                    let gate_raw: usize = parts[6].parse().map_err(|_| {
                        SnapshotError::Parse(
                            "ship_runtime last_gate_arrival parse failed".to_string(),
                        )
                    })?;
                    if gate_raw == usize::MAX {
                        None
                    } else {
                        Some(GateId(gate_raw))
                    }
                } else {
                    None
                };
                ship.eta_ticks_remaining = ship.segment_eta_remaining;
            }
        }

        if let Some(contracts_blob) = map.get("contracts") {
            for row in contracts_blob.split(',').filter(|v| !v.is_empty()) {
                let parts: Vec<&str> = row.split(':').collect();
                if parts.len() != 11 && parts.len() != 13 {
                    return Err(SnapshotError::Parse(format!("bad contract row: {row}")));
                }
                let id: usize = parts[0]
                    .parse()
                    .map_err(|_| SnapshotError::Parse("contract id parse failed".to_string()))?;
                if let Some(contract) = simulation.contracts.get_mut(&ContractId(id)) {
                    let (
                        delivered_idx,
                        penalty_idx,
                        misses_idx,
                        completed_idx,
                        failed_idx,
                        deadline_idx,
                        eval_idx,
                    ) = if parts.len() == 13 {
                        let origin_station: usize = parts[4].parse().map_err(|_| {
                            SnapshotError::Parse("contract origin_station parse failed".to_string())
                        })?;
                        let destination_station: usize = parts[5].parse().map_err(|_| {
                            SnapshotError::Parse(
                                "contract destination_station parse failed".to_string(),
                            )
                        })?;
                        contract.origin_station = StationId(origin_station);
                        contract.destination_station = StationId(destination_station);
                        (6, 7, 8, 9, 10, 11, 12)
                    } else {
                        contract.origin_station = simulation
                            .world
                            .first_station(contract.origin)
                            .unwrap_or(StationId(0));
                        contract.destination_station = simulation
                            .world
                            .first_station(contract.destination)
                            .unwrap_or(contract.origin_station);
                        (4, 5, 6, 7, 8, 9, 10)
                    };
                    contract.delivered_amount = parts[delivered_idx].parse().map_err(|_| {
                        SnapshotError::Parse("contract delivered parse failed".to_string())
                    })?;
                    contract.penalty = parts[penalty_idx].parse().map_err(|_| {
                        SnapshotError::Parse("contract penalty parse failed".to_string())
                    })?;
                    contract.missed_cycles = parts[misses_idx].parse().map_err(|_| {
                        SnapshotError::Parse("contract misses parse failed".to_string())
                    })?;
                    contract.completed = parts[completed_idx] == "1";
                    contract.failed = parts[failed_idx] == "1";
                    contract.deadline_tick = parts[deadline_idx].parse().map_err(|_| {
                        SnapshotError::Parse("contract deadline parse failed".to_string())
                    })?;
                    contract.last_eval_cycle = parts[eval_idx].parse().map_err(|_| {
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

        simulation.active_leases.clear();
        if let Some(leases_blob) = map.get("leases") {
            for row in leases_blob.split(',').filter(|v| !v.is_empty()) {
                let parts: Vec<&str> = row.split(':').collect();
                if parts.len() != 4 {
                    return Err(SnapshotError::Parse(format!("bad lease row: {row}")));
                }
                let system_id: usize = parts[0]
                    .parse()
                    .map_err(|_| SnapshotError::Parse("lease system parse failed".to_string()))?;
                let slot_type = slot_type_from_code(parts[1]).ok_or_else(|| {
                    SnapshotError::Parse("lease slot type parse failed".to_string())
                })?;
                let cycles_remaining: u32 = parts[2]
                    .parse()
                    .map_err(|_| SnapshotError::Parse("lease cycles parse failed".to_string()))?;
                let price_per_cycle: f64 = parts[3]
                    .parse()
                    .map_err(|_| SnapshotError::Parse("lease price parse failed".to_string()))?;

                simulation.active_leases.push(LeasePosition {
                    system_id: SystemId(system_id),
                    slot_type,
                    cycles_remaining,
                    price_per_cycle,
                });
            }
        }

        if let Some(companies_blob) = map.get("companies") {
            simulation.companies.clear();
            for row in companies_blob.split(',').filter(|v| !v.is_empty()) {
                let parts: Vec<&str> = row.split(':').collect();
                if parts.len() != 3 {
                    return Err(SnapshotError::Parse(format!("bad company row: {row}")));
                }
                let company_id: usize = parts[0]
                    .parse()
                    .map_err(|_| SnapshotError::Parse("company id parse failed".to_string()))?;
                let archetype = company_archetype_from_code(parts[1]).ok_or_else(|| {
                    SnapshotError::Parse("company archetype parse failed".to_string())
                })?;
                simulation.companies.insert(
                    CompanyId(company_id),
                    Company {
                        id: CompanyId(company_id),
                        name: restore_snapshot_text(parts[2]),
                        archetype,
                    },
                );
            }
        }

        if let Some(offers_blob) = map.get("offers") {
            simulation.contract_offers.clear();
            for row in offers_blob.split(',').filter(|v| !v.is_empty()) {
                let parts: Vec<&str> = row.split(':').collect();
                if parts.len() != 11 && parts.len() != 15 && parts.len() != 17 {
                    return Err(SnapshotError::Parse(format!("bad offer row: {row}")));
                }
                let offer_id: u64 = parts[0]
                    .parse()
                    .map_err(|_| SnapshotError::Parse("offer id parse failed".to_string()))?;
                let kind = contract_type_from_code(parts[1])
                    .ok_or_else(|| SnapshotError::Parse("offer kind parse failed".to_string()))?;
                let origin: usize = parts[2]
                    .parse()
                    .map_err(|_| SnapshotError::Parse("offer origin parse failed".to_string()))?;
                let destination: usize = parts[3].parse().map_err(|_| {
                    SnapshotError::Parse("offer destination parse failed".to_string())
                })?;
                let (
                    origin_station,
                    destination_station,
                    quantity_idx,
                    payout_idx,
                    penalty_idx,
                    eta_idx,
                    risk_idx,
                    margin_idx,
                ) = if parts.len() == 17 {
                    let origin_station: usize = parts[4].parse().map_err(|_| {
                        SnapshotError::Parse("offer origin station parse failed".to_string())
                    })?;
                    let destination_station: usize = parts[5].parse().map_err(|_| {
                        SnapshotError::Parse("offer destination station parse failed".to_string())
                    })?;
                    (
                        StationId(origin_station),
                        StationId(destination_station),
                        6,
                        7,
                        8,
                        9,
                        10,
                        11,
                    )
                } else {
                    let origin_station = simulation
                        .world
                        .first_station(SystemId(origin))
                        .unwrap_or(StationId(0));
                    let destination_station = simulation
                        .world
                        .first_station(SystemId(destination))
                        .unwrap_or(origin_station);
                    (origin_station, destination_station, 4, 5, 6, 7, 8, 9)
                };
                let quantity: f64 = parts[quantity_idx]
                    .parse()
                    .map_err(|_| SnapshotError::Parse("offer quantity parse failed".to_string()))?;
                let payout: f64 = parts[payout_idx]
                    .parse()
                    .map_err(|_| SnapshotError::Parse("offer payout parse failed".to_string()))?;
                let penalty: f64 = parts[penalty_idx]
                    .parse()
                    .map_err(|_| SnapshotError::Parse("offer penalty parse failed".to_string()))?;
                let eta_ticks: u32 = parts[eta_idx]
                    .parse()
                    .map_err(|_| SnapshotError::Parse("offer eta parse failed".to_string()))?;
                let risk_score: f64 = parts[risk_idx]
                    .parse()
                    .map_err(|_| SnapshotError::Parse("offer risk parse failed".to_string()))?;
                let margin_estimate: f64 = parts[margin_idx]
                    .parse()
                    .map_err(|_| SnapshotError::Parse("offer margin parse failed".to_string()))?;
                let (problem_tag, premium, profit_per_ton, route_gate_ids, expires_idx) = if parts
                    .len()
                    == 17
                {
                    let problem = offer_problem_from_code(parts[12]).ok_or_else(|| {
                        SnapshotError::Parse("offer problem parse failed".to_string())
                    })?;
                    let premium = parts[13] == "1";
                    let profit_per_ton: f64 = parts[14].parse().map_err(|_| {
                        SnapshotError::Parse("offer profit_per_ton parse failed".to_string())
                    })?;
                    let gate_ids = if parts[15] == "none" || parts[15].is_empty() {
                        Vec::new()
                    } else {
                        let mut ids = Vec::new();
                        for raw in parts[15].split('-').filter(|value| !value.is_empty()) {
                            let gate_id: usize = raw.parse().map_err(|_| {
                                SnapshotError::Parse("offer route gate id parse failed".to_string())
                            })?;
                            ids.push(GateId(gate_id));
                        }
                        ids
                    };
                    (problem, premium, profit_per_ton, gate_ids, 16)
                } else if parts.len() == 15 {
                    let problem = offer_problem_from_code(parts[10]).ok_or_else(|| {
                        SnapshotError::Parse("offer problem parse failed".to_string())
                    })?;
                    let premium = parts[11] == "1";
                    let profit_per_ton: f64 = parts[12].parse().map_err(|_| {
                        SnapshotError::Parse("offer profit_per_ton parse failed".to_string())
                    })?;
                    let gate_ids = if parts[13] == "none" || parts[13].is_empty() {
                        Vec::new()
                    } else {
                        let mut ids = Vec::new();
                        for raw in parts[13].split('-').filter(|value| !value.is_empty()) {
                            let gate_id: usize = raw.parse().map_err(|_| {
                                SnapshotError::Parse("offer route gate id parse failed".to_string())
                            })?;
                            ids.push(GateId(gate_id));
                        }
                        ids
                    };
                    (problem, premium, profit_per_ton, gate_ids, 14)
                } else {
                    (
                        OfferProblemTag::LowMargin,
                        false,
                        margin_estimate / quantity.max(1.0),
                        Vec::new(),
                        10,
                    )
                };
                let expires_cycle: u64 = parts[expires_idx]
                    .parse()
                    .map_err(|_| SnapshotError::Parse("offer expires parse failed".to_string()))?;

                simulation.contract_offers.insert(
                    offer_id,
                    ContractOffer {
                        id: offer_id,
                        kind,
                        origin: SystemId(origin),
                        destination: SystemId(destination),
                        origin_station,
                        destination_station,
                        quantity,
                        payout,
                        penalty,
                        eta_ticks,
                        risk_score,
                        margin_estimate,
                        route_gate_ids,
                        problem_tag,
                        premium,
                        profit_per_ton,
                        expires_cycle,
                    },
                );
            }
        }

        if let Some(milestones_blob) = map.get("milestones") {
            simulation.milestones.clear();
            for row in milestones_blob.split(',').filter(|v| !v.is_empty()) {
                let parts: Vec<&str> = row.split(':').collect();
                if parts.len() != 5 {
                    return Err(SnapshotError::Parse(format!("bad milestone row: {row}")));
                }
                let id = milestone_id_from_code(parts[0])
                    .ok_or_else(|| SnapshotError::Parse("milestone id parse failed".to_string()))?;
                let current: f64 = parts[1].parse().map_err(|_| {
                    SnapshotError::Parse("milestone current parse failed".to_string())
                })?;
                let target: f64 = parts[2].parse().map_err(|_| {
                    SnapshotError::Parse("milestone target parse failed".to_string())
                })?;
                let completed = parts[3] == "1";
                let completed_raw: u64 = parts[4].parse().map_err(|_| {
                    SnapshotError::Parse("milestone completed cycle parse failed".to_string())
                })?;
                simulation.milestones.push(MilestoneStatus {
                    id,
                    current,
                    target,
                    completed,
                    completed_cycle: if completed_raw == u64::MAX {
                        None
                    } else {
                        Some(completed_raw)
                    },
                });
            }
        }

        if let Some(gate_cycle_blob) = map.get("gate_cycle") {
            simulation.gate_traversals_cycle = decode_gate_counts_map(gate_cycle_blob)?;
        }
        if let Some(gate_window_blob) = map.get("gate_window") {
            simulation.gate_traversals_window.clear();
            for cycle_blob in gate_window_blob
                .split('|')
                .filter(|value| !value.is_empty())
            {
                simulation
                    .gate_traversals_window
                    .push_back(decode_gate_counts_map(cycle_blob)?);
            }
        }

        if let Some(ship_kpis_blob) = map.get("ship_kpis") {
            simulation.ship_idle_ticks_cycle.clear();
            simulation.ship_delay_ticks_cycle.clear();
            simulation.ship_runs_completed.clear();
            simulation.ship_profit_earned.clear();
            for row in ship_kpis_blob.split(',').filter(|value| !value.is_empty()) {
                let parts: Vec<&str> = row.split(':').collect();
                if parts.len() != 5 {
                    return Err(SnapshotError::Parse(format!("bad ship_kpi row: {row}")));
                }
                let ship_id =
                    ShipId(parts[0].parse().map_err(|_| {
                        SnapshotError::Parse("ship_kpi id parse failed".to_string())
                    })?);
                let idle: u32 = parts[1]
                    .parse()
                    .map_err(|_| SnapshotError::Parse("ship_kpi idle parse failed".to_string()))?;
                let delay: u32 = parts[2]
                    .parse()
                    .map_err(|_| SnapshotError::Parse("ship_kpi delay parse failed".to_string()))?;
                let runs: u32 = parts[3]
                    .parse()
                    .map_err(|_| SnapshotError::Parse("ship_kpi runs parse failed".to_string()))?;
                let profit: f64 = parts[4].parse().map_err(|_| {
                    SnapshotError::Parse("ship_kpi profit parse failed".to_string())
                })?;
                simulation.ship_idle_ticks_cycle.insert(ship_id, idle);
                simulation.ship_delay_ticks_cycle.insert(ship_id, delay);
                simulation.ship_runs_completed.insert(ship_id, runs);
                simulation.ship_profit_earned.insert(ship_id, profit);
            }
        }

        if let Some(recovery_log_blob) = map.get("recovery_log") {
            simulation.recovery_log.clear();
            for row in recovery_log_blob
                .split(',')
                .filter(|value| !value.is_empty())
            {
                let parts: Vec<&str> = row.split(':').collect();
                if parts.len() != 4 {
                    return Err(SnapshotError::Parse(format!("bad recovery row: {row}")));
                }
                simulation.recovery_log.push(RecoveryAction {
                    cycle: parts[0].parse().map_err(|_| {
                        SnapshotError::Parse("recovery cycle parse failed".to_string())
                    })?,
                    released_leases: parts[1].parse().map_err(|_| {
                        SnapshotError::Parse("recovery released parse failed".to_string())
                    })?,
                    capital_after: parts[2].parse().map_err(|_| {
                        SnapshotError::Parse("recovery capital parse failed".to_string())
                    })?,
                    debt_after: parts[3].parse().map_err(|_| {
                        SnapshotError::Parse("recovery debt parse failed".to_string())
                    })?,
                });
            }
        }

        if let Some(prev_prices_blob) = map.get("prev_prices") {
            simulation.previous_cycle_prices.clear();
            for row in prev_prices_blob
                .split(',')
                .filter(|value| !value.is_empty())
            {
                let parts: Vec<&str> = row.split(':').collect();
                if parts.len() != 3 {
                    return Err(SnapshotError::Parse(format!("bad prev price row: {row}")));
                }
                let system_id: usize = parts[0].parse().map_err(|_| {
                    SnapshotError::Parse("prev price system parse failed".to_string())
                })?;
                let commodity = commodity_from_code(parts[1]).ok_or_else(|| {
                    SnapshotError::Parse("prev price commodity parse failed".to_string())
                })?;
                let price: f64 = parts[2].parse().map_err(|_| {
                    SnapshotError::Parse("prev price value parse failed".to_string())
                })?;
                simulation
                    .previous_cycle_prices
                    .insert((SystemId(system_id), commodity), price);
            }
        }

        if let Some(gate_loads_blob) = map.get("gate_loads") {
            simulation.gate_queue_load.clear();
            for row in gate_loads_blob.split(',').filter(|value| !value.is_empty()) {
                let parts: Vec<&str> = row.split(':').collect();
                if parts.len() != 2 {
                    return Err(SnapshotError::Parse(format!("bad gate load row: {row}")));
                }
                let gate_id: usize = parts[0]
                    .parse()
                    .map_err(|_| SnapshotError::Parse("gate load id parse failed".to_string()))?;
                let load: f64 = parts[1].parse().map_err(|_| {
                    SnapshotError::Parse("gate load value parse failed".to_string())
                })?;
                simulation.gate_queue_load.insert(GateId(gate_id), load);
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

fn seed_stage_a_companies() -> BTreeMap<CompanyId, Company> {
    let mut companies = BTreeMap::new();
    companies.insert(
        CompanyId(0),
        Company {
            id: CompanyId(0),
            name: "Player Logistics".to_string(),
            archetype: CompanyArchetype::Player,
        },
    );
    companies.insert(
        CompanyId(1),
        Company {
            id: CompanyId(1),
            name: "Haulers Alpha".to_string(),
            archetype: CompanyArchetype::Hauler,
        },
    );
    companies.insert(
        CompanyId(2),
        Company {
            id: CompanyId(2),
            name: "Haulers Beta".to_string(),
            archetype: CompanyArchetype::Hauler,
        },
    );
    companies.insert(
        CompanyId(3),
        Company {
            id: CompanyId(3),
            name: "Miner Guild".to_string(),
            archetype: CompanyArchetype::Miner,
        },
    );
    companies.insert(
        CompanyId(4),
        Company {
            id: CompanyId(4),
            name: "Industrial Combine".to_string(),
            archetype: CompanyArchetype::Industrial,
        },
    );
    companies
}

fn seed_stage_a_ships(world: &World) -> BTreeMap<ShipId, Ship> {
    let mut ships = BTreeMap::new();
    if world.system_count() == 0 {
        return ships;
    }
    let sid = |idx: usize| SystemId(idx % world.system_count());
    let wp = |a: usize, b: usize| vec![sid(a), sid(b)];
    let configs = [
        (ShipId(0), CompanyId(0), sid(0), wp(0, 1)),
        (ShipId(1), CompanyId(1), sid(0), wp(0, 1)),
        (ShipId(2), CompanyId(1), sid(1), wp(1, 2)),
        (ShipId(3), CompanyId(2), sid(2), wp(2, 0)),
        (ShipId(4), CompanyId(2), sid(1), wp(1, 0)),
        (ShipId(5), CompanyId(3), sid(2), wp(2, 1)),
        (ShipId(6), CompanyId(4), sid(0), wp(0, 2)),
        (ShipId(7), CompanyId(4), sid(1), wp(1, 2)),
    ];
    for (ship_id, company_id, location, waypoints) in configs {
        ships.insert(
            ship_id,
            Ship {
                id: ship_id,
                company_id,
                location,
                eta_ticks_remaining: 0,
                sub_light_speed: 18.0,
                movement_queue: VecDeque::new(),
                segment_eta_remaining: 0,
                segment_progress_total: 0,
                current_segment_kind: None,
                active_contract: None,
                route_cursor: 0,
                policy: AutopilotPolicy {
                    waypoints,
                    ..AutopilotPolicy::default()
                },
                planned_path: Vec::new(),
                current_target: None,
                last_gate_arrival: None,
                last_risk_score: 0.0,
                reroutes: 0,
            },
        );
    }
    ships
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

fn slot_multiplier(slot_type: SlotType) -> f64 {
    match slot_type {
        SlotType::Dock => 1.30,
        SlotType::Storage => 1.00,
        SlotType::Factory => 1.50,
        SlotType::Market => 1.20,
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

fn slot_type_code(slot_type: SlotType) -> &'static str {
    match slot_type {
        SlotType::Dock => "dock",
        SlotType::Storage => "storage",
        SlotType::Factory => "factory",
        SlotType::Market => "market",
    }
}

fn slot_type_from_code(raw: &str) -> Option<SlotType> {
    match raw {
        "dock" => Some(SlotType::Dock),
        "storage" => Some(SlotType::Storage),
        "factory" => Some(SlotType::Factory),
        "market" => Some(SlotType::Market),
        _ => None,
    }
}

fn company_archetype_code(archetype: CompanyArchetype) -> &'static str {
    match archetype {
        CompanyArchetype::Player => "player",
        CompanyArchetype::Hauler => "hauler",
        CompanyArchetype::Miner => "miner",
        CompanyArchetype::Industrial => "industrial",
    }
}

fn company_archetype_from_code(raw: &str) -> Option<CompanyArchetype> {
    match raw {
        "player" => Some(CompanyArchetype::Player),
        "hauler" => Some(CompanyArchetype::Hauler),
        "miner" => Some(CompanyArchetype::Miner),
        "industrial" => Some(CompanyArchetype::Industrial),
        _ => None,
    }
}

fn contract_type_code(kind: ContractTypeStageA) -> &'static str {
    match kind {
        ContractTypeStageA::Delivery => "delivery",
        ContractTypeStageA::Supply => "supply",
    }
}

fn contract_type_from_code(raw: &str) -> Option<ContractTypeStageA> {
    match raw {
        "delivery" => Some(ContractTypeStageA::Delivery),
        "supply" => Some(ContractTypeStageA::Supply),
        _ => None,
    }
}

fn offer_problem_code(problem: OfferProblemTag) -> &'static str {
    match problem {
        OfferProblemTag::HighRisk => "high_risk",
        OfferProblemTag::CongestedRoute => "congested_route",
        OfferProblemTag::LowMargin => "low_margin",
        OfferProblemTag::FuelVolatility => "fuel_volatility",
    }
}

fn offer_problem_from_code(raw: &str) -> Option<OfferProblemTag> {
    match raw {
        "high_risk" => Some(OfferProblemTag::HighRisk),
        "congested_route" => Some(OfferProblemTag::CongestedRoute),
        "low_margin" => Some(OfferProblemTag::LowMargin),
        "fuel_volatility" => Some(OfferProblemTag::FuelVolatility),
        _ => None,
    }
}

fn segment_kind_code(kind: SegmentKind) -> &'static str {
    match kind {
        SegmentKind::InSystem => "in_system",
        SegmentKind::GateQueue => "gate_queue",
        SegmentKind::Warp => "warp",
        SegmentKind::Dock => "dock",
    }
}

fn segment_kind_from_code(raw: &str) -> Option<SegmentKind> {
    match raw {
        "in_system" => Some(SegmentKind::InSystem),
        "gate_queue" => Some(SegmentKind::GateQueue),
        "warp" => Some(SegmentKind::Warp),
        "dock" => Some(SegmentKind::Dock),
        _ => None,
    }
}

fn encode_route_segment(segment: &RouteSegment) -> String {
    format!(
        "{}/{}/{}/{}/{}/{}/{}/{}",
        segment.from.0,
        segment.to.0,
        segment.from_anchor.map_or(usize::MAX, |id| id.0),
        segment.to_anchor.map_or(usize::MAX, |id| id.0),
        segment.edge.map_or(usize::MAX, |id| id.0),
        segment_kind_code(segment.kind),
        segment.eta_ticks,
        segment.risk
    )
}

fn decode_route_segment(raw: &str) -> Result<RouteSegment, SnapshotError> {
    let parts: Vec<&str> = raw.split('/').collect();
    if parts.len() != 8 {
        return Err(SnapshotError::Parse(format!(
            "bad route segment row: {raw}"
        )));
    }
    let from: usize = parts[0]
        .parse()
        .map_err(|_| SnapshotError::Parse("route segment from parse failed".to_string()))?;
    let to: usize = parts[1]
        .parse()
        .map_err(|_| SnapshotError::Parse("route segment to parse failed".to_string()))?;
    let from_anchor_raw: usize = parts[2]
        .parse()
        .map_err(|_| SnapshotError::Parse("route segment from_anchor parse failed".to_string()))?;
    let to_anchor_raw: usize = parts[3]
        .parse()
        .map_err(|_| SnapshotError::Parse("route segment to_anchor parse failed".to_string()))?;
    let edge_raw: usize = parts[4]
        .parse()
        .map_err(|_| SnapshotError::Parse("route segment edge parse failed".to_string()))?;
    let kind = segment_kind_from_code(parts[5])
        .ok_or_else(|| SnapshotError::Parse("route segment kind parse failed".to_string()))?;
    let eta_ticks: u32 = parts[6]
        .parse()
        .map_err(|_| SnapshotError::Parse("route segment eta parse failed".to_string()))?;
    let risk: f64 = parts[7]
        .parse()
        .map_err(|_| SnapshotError::Parse("route segment risk parse failed".to_string()))?;

    Ok(RouteSegment {
        from: SystemId(from),
        to: SystemId(to),
        from_anchor: if from_anchor_raw == usize::MAX {
            None
        } else {
            Some(StationId(from_anchor_raw))
        },
        to_anchor: if to_anchor_raw == usize::MAX {
            None
        } else {
            Some(StationId(to_anchor_raw))
        },
        edge: if edge_raw == usize::MAX {
            None
        } else {
            Some(GateId(edge_raw))
        },
        kind,
        eta_ticks,
        risk,
    })
}

fn milestone_id_code(id: MilestoneId) -> &'static str {
    match id {
        MilestoneId::Capital => "capital",
        MilestoneId::MarketShare => "market_share",
        MilestoneId::ThroughputControl => "throughput",
        MilestoneId::Reputation => "reputation",
    }
}

fn milestone_id_from_code(raw: &str) -> Option<MilestoneId> {
    match raw {
        "capital" => Some(MilestoneId::Capital),
        "market_share" => Some(MilestoneId::MarketShare),
        "throughput" => Some(MilestoneId::ThroughputControl),
        "reputation" => Some(MilestoneId::Reputation),
        _ => None,
    }
}

fn sanitize_snapshot_text(raw: &str) -> String {
    raw.replace([':', ','], "_")
}

fn restore_snapshot_text(raw: &str) -> String {
    raw.to_string()
}

fn encode_gate_counts_map(map: &BTreeMap<GateId, BTreeMap<CompanyId, u32>>) -> String {
    let mut encoded = String::new();
    for (gate_id, by_company) in map {
        for (company_id, count) in by_company {
            encoded.push_str(&format!("{}-{}-{},", gate_id.0, company_id.0, count));
        }
    }
    encoded
}

fn decode_gate_counts_map(
    raw: &str,
) -> Result<BTreeMap<GateId, BTreeMap<CompanyId, u32>>, SnapshotError> {
    let mut map: BTreeMap<GateId, BTreeMap<CompanyId, u32>> = BTreeMap::new();
    for row in raw.split(',').filter(|value| !value.is_empty()) {
        let parts: Vec<&str> = row.split('-').collect();
        if parts.len() != 3 {
            return Err(SnapshotError::Parse(format!("bad gate count row: {row}")));
        }
        let gate_id: usize = parts[0]
            .parse()
            .map_err(|_| SnapshotError::Parse("gate count gate parse failed".to_string()))?;
        let company_id: usize = parts[1]
            .parse()
            .map_err(|_| SnapshotError::Parse("gate count company parse failed".to_string()))?;
        let count: u32 = parts[2]
            .parse()
            .map_err(|_| SnapshotError::Parse("gate count value parse failed".to_string()))?;
        map.entry(GateId(gate_id))
            .or_default()
            .insert(CompanyId(company_id), count);
    }
    Ok(map)
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

fn extract_json_u32_field(input: &str, field: &str) -> Option<u32> {
    let needle = format!("\"{field}\":");
    let start = input.find(&needle)? + needle.len();
    let tail = input[start..].trim_start();
    let end = tail
        .find(|ch: char| !ch.is_ascii_digit())
        .unwrap_or(tail.len());
    tail[..end].parse().ok()
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

fn parse_optional_u64_from_map(map: &BTreeMap<String, String>, key: &str) -> Option<u64> {
    map.get(key).and_then(|v| v.parse::<u64>().ok())
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

fn parse_optional_f64_from_map(map: &BTreeMap<String, String>, key: &str) -> Option<f64> {
    map.get(key).and_then(|v| v.parse::<f64>().ok())
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
                    || maybe_too_short
                        .expect("value checked")
                        .segments
                        .iter()
                        .filter(|segment| segment.kind == SegmentKind::Warp)
                        .count()
                        <= 1,
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
            route_after.as_ref().is_none_or(|route| {
                route
                    .segments
                    .iter()
                    .filter(|segment| segment.kind == SegmentKind::Warp)
                    .count()
                    <= 1
            }),
            "policy max_hops must constrain route selection"
        );

        assert!(
            route_before
                .segments
                .iter()
                .filter(|segment| segment.kind == SegmentKind::Warp)
                .count()
                >= route_after.as_ref().map_or(0, |route| {
                    route
                        .segments
                        .iter()
                        .filter(|segment| segment.kind == SegmentKind::Warp)
                        .count()
                }),
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
    fn gate_warp_segment_has_zero_eta_and_keeps_queue_delay() {
        let sim = Simulation::new(stage_a_config(), 301);
        let origin_station = sim
            .world
            .first_station(SystemId(0))
            .expect("origin station should exist");
        let destination_station = sim
            .world
            .first_station(SystemId(1))
            .expect("destination station should exist");
        let route = sim
            .build_station_route(
                origin_station,
                destination_station,
                AutopilotPolicy::default(),
            )
            .expect("station route should exist");

        assert!(
            route
                .segments
                .iter()
                .any(|segment| segment.kind == SegmentKind::Warp && segment.eta_ticks == 0),
            "warp segments must be teleport with zero eta"
        );
        assert!(
            route
                .segments
                .iter()
                .any(|segment| segment.kind == SegmentKind::GateQueue),
            "gate queue stage must remain in route"
        );
    }

    #[test]
    fn warp_completion_sets_last_gate_arrival() {
        let mut sim = Simulation::new(stage_a_config(), 303);
        sim.ships.retain(|id, _| *id == ShipId(0));
        let Some(edge) = sim.world.edges.first().cloned() else {
            return;
        };
        let ship_id = ShipId(0);
        if let Some(ship) = sim.ships.get_mut(&ship_id) {
            ship.location = edge.a;
            ship.movement_queue = VecDeque::from([RouteSegment {
                from: edge.a,
                to: edge.b,
                from_anchor: None,
                to_anchor: None,
                edge: Some(edge.id),
                kind: SegmentKind::Warp,
                eta_ticks: 0,
                risk: 0.0,
            }]);
            ship.segment_eta_remaining = 0;
            ship.segment_progress_total = 0;
            ship.current_segment_kind = None;
            ship.current_target = None;
            ship.last_gate_arrival = None;
        }
        sim.start_next_movement_segment(ship_id, 1.0);
        let ship = sim.ships.get(&ship_id).expect("ship should exist");
        assert_eq!(ship.location, edge.b);
        assert_eq!(ship.last_gate_arrival, Some(edge.id));
    }

    #[test]
    fn last_gate_arrival_cleared_on_new_station_route() {
        let mut sim = Simulation::new(stage_a_config(), 305);
        sim.ships.retain(|id, _| *id == ShipId(0));
        if sim.world.system_count() < 2 {
            return;
        }
        let ship_id = ShipId(0);
        let fallback_gate = sim.world.edges.first().map(|edge| edge.id);
        if let Some(ship) = sim.ships.get_mut(&ship_id) {
            ship.active_contract = None;
            ship.location = SystemId(0);
            ship.policy.waypoints = vec![SystemId(1)];
            ship.policy.max_risk_score = 10.0;
            ship.route_cursor = 0;
            ship.movement_queue.clear();
            ship.segment_eta_remaining = 0;
            ship.segment_progress_total = 0;
            ship.current_segment_kind = None;
            ship.current_target = None;
            ship.last_gate_arrival = fallback_gate;
        }

        sim.update_ship_movements();

        let ship = sim.ships.get(&ship_id).expect("ship should exist");
        assert_eq!(ship.last_gate_arrival, None);
    }

    #[test]
    fn station_route_contains_in_system_segments_between_gates_and_stations() {
        let sim = Simulation::new(stage_a_config(), 307);
        let origin_station = sim
            .world
            .first_station(SystemId(0))
            .expect("origin station should exist");
        let destination_station = sim
            .world
            .first_station(SystemId(sim.world.system_count().saturating_sub(1)))
            .expect("destination station should exist");
        let route = sim
            .build_station_route(
                origin_station,
                destination_station,
                AutopilotPolicy::default(),
            )
            .expect("station route should exist");

        assert!(
            route
                .segments
                .first()
                .is_some_and(|segment| segment.kind == SegmentKind::InSystem),
            "route must start with in-system movement from station"
        );
        assert!(
            route
                .segments
                .last()
                .is_some_and(|segment| segment.kind == SegmentKind::InSystem),
            "route must end with in-system movement to destination station"
        );
    }

    #[test]
    fn in_system_eta_uses_distance_over_sub_light_speed() {
        let sim = Simulation::new(stage_a_config(), 311);
        let system_id = SystemId(0);
        let stations = sim
            .world
            .stations_by_system
            .get(&system_id)
            .cloned()
            .expect("stations should exist");
        if stations.len() < 2 {
            return;
        }
        let from_station = stations[0];
        let to_station = stations[1];
        let from = sim
            .world
            .stations
            .iter()
            .find(|station| station.id == from_station)
            .expect("from station exists");
        let to = sim
            .world
            .stations
            .iter()
            .find(|station| station.id == to_station)
            .expect("to station exists");
        let speed = 9.0;
        let dx = to.x - from.x;
        let dy = to.y - from.y;
        let expected = ((dx * dx + dy * dy).sqrt() / speed).ceil().max(1.0) as u32;
        let route = sim
            .build_station_route_with_speed(
                from_station,
                to_station,
                AutopilotPolicy::default(),
                speed,
            )
            .expect("route should exist");
        let in_system = route
            .segments
            .iter()
            .find(|segment| segment.kind == SegmentKind::InSystem)
            .expect("in-system segment should exist");
        assert_eq!(in_system.eta_ticks, expected);
    }

    #[test]
    fn multi_hop_route_follows_station_gate_gate_station_pattern() {
        let sim = Simulation::new(stage_a_config(), 313);
        if sim.world.system_count() < 3 {
            return;
        }
        let mut route_with_hops = None;
        'search: for from_idx in 0..sim.world.system_count() {
            for to_idx in 0..sim.world.system_count() {
                if from_idx == to_idx {
                    continue;
                }
                let Some(route) = sim.build_station_route(
                    sim.world
                        .first_station(SystemId(from_idx))
                        .expect("station exists"),
                    sim.world
                        .first_station(SystemId(to_idx))
                        .expect("station exists"),
                    AutopilotPolicy {
                        max_hops: 16,
                        ..AutopilotPolicy::default()
                    },
                ) else {
                    continue;
                };
                let warp_count = route
                    .segments
                    .iter()
                    .filter(|segment| segment.kind == SegmentKind::Warp)
                    .count();
                if warp_count >= 2 {
                    route_with_hops = Some(route);
                    break 'search;
                }
            }
        }
        let Some(route) = route_with_hops else {
            return;
        };
        for segment in &route.segments {
            if segment.kind == SegmentKind::Warp {
                assert_eq!(segment.eta_ticks, 0);
            }
        }
    }

    #[test]
    fn delivery_completes_only_after_destination_station_reached() {
        let mut cfg = stage_a_config();
        cfg.pressure.ship_upkeep_per_tick = 0.0;
        cfg.pressure.gate_fee_per_jump = 0.0;
        let mut sim = Simulation::new(cfg, 317);
        sim.ships.retain(|id, _| *id == ShipId(0));
        let destination_system = if sim.world.system_count() > 1 {
            SystemId(1)
        } else {
            SystemId(0)
        };
        let destination_station = sim
            .world
            .stations_by_system
            .get(&destination_system)
            .and_then(|stations| stations.last().copied())
            .unwrap_or_else(|| {
                sim.world
                    .first_station(destination_system)
                    .unwrap_or(StationId(0))
            });
        if let Some(contract) = sim.contracts.get_mut(&ContractId(0)) {
            contract.completed = false;
            contract.failed = false;
            contract.destination = destination_system;
            contract.destination_station = destination_station;
            contract.assigned_ship = Some(ShipId(0));
            contract.deadline_tick = 10_000;
        }
        if let Some(ship) = sim.ships.get_mut(&ShipId(0)) {
            ship.location = SystemId(0);
            ship.active_contract = Some(ContractId(0));
            ship.policy.max_hops = 16;
        }

        let mut observed_destination_system_before_completion = false;
        for _ in 0..200 {
            sim.step_tick();
            let ship = sim.ships.get(&ShipId(0)).expect("ship should exist");
            let contract = sim
                .contracts
                .get(&ContractId(0))
                .expect("contract should exist");
            if ship.location == destination_system && !contract.completed {
                observed_destination_system_before_completion = true;
            }
            if contract.completed {
                break;
            }
        }
        let contract = sim
            .contracts
            .get(&ContractId(0))
            .expect("contract should exist");
        assert!(contract.completed, "contract should eventually complete");
        assert!(
            observed_destination_system_before_completion,
            "arrival to destination system must not auto-complete before final station segment"
        );
    }

    #[test]
    fn gate_fee_and_traversal_count_apply_on_teleport_segment() {
        let mut cfg = stage_a_config();
        cfg.pressure.ship_upkeep_per_tick = 0.0;
        cfg.pressure.gate_fee_per_jump = 4.0;
        let mut sim = Simulation::new(cfg, 331);
        sim.ships.retain(|id, _| *id == ShipId(0));
        if let Some(ship) = sim.ships.get_mut(&ShipId(0)) {
            ship.active_contract = None;
            ship.location = SystemId(0);
            ship.policy.waypoints = vec![SystemId(1)];
            ship.route_cursor = 0;
            ship.policy.max_risk_score = 10.0;
        }
        let route = sim
            .route_for_ship(ShipId(0), SystemId(1))
            .expect("route should exist");
        assert!(route
            .segments
            .iter()
            .any(|segment| segment.kind == SegmentKind::Warp && segment.eta_ticks == 0));

        let capital_before = sim.capital;
        let traversal_before = sim
            .gate_traversals_cycle
            .values()
            .flat_map(|by_company| by_company.values())
            .copied()
            .sum::<u32>();
        for _ in 0..120 {
            sim.step_tick();
            let traversal_after = sim
                .gate_traversals_cycle
                .values()
                .flat_map(|by_company| by_company.values())
                .copied()
                .sum::<u32>();
            if traversal_after > traversal_before {
                break;
            }
        }
        let traversal_after = sim
            .gate_traversals_cycle
            .values()
            .flat_map(|by_company| by_company.values())
            .copied()
            .sum::<u32>();
        assert!(traversal_after > traversal_before);
        assert!(
            capital_before - sim.capital >= 4.0,
            "gate fee should be charged on warp teleport segment start"
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

        assert_eq!(base_reports.len(), loaded_reports.len());
        for (base_report, loaded_report) in base_reports.iter().zip(loaded_reports.iter()) {
            assert_eq!(base_report.tick, loaded_report.tick);
            assert_eq!(base_report.cycle, loaded_report.cycle);
            assert_eq!(base_report.active_ships, loaded_report.active_ships);
            assert_eq!(base_report.active_contracts, loaded_report.active_contracts);
            assert!(
                (base_report.total_queue_delay as i64 - loaded_report.total_queue_delay as i64)
                    .abs()
                    <= 8,
                "queue delay should remain close after snapshot reload"
            );
            assert!(
                (base_report.avg_price_index - loaded_report.avg_price_index).abs() < 1e-6,
                "price index should stay stable after snapshot reload"
            );
        }
    }

    #[test]
    fn snapshot_v1_loads_with_default_station_mapping() {
        let cfg = stage_a_config();
        let state =
            "tick=1;cycle=0;capital=500;qdelay=0;reroutes=0;sla_s=0;sla_f=0;edges=;ships=;contracts=;markets=;modifiers=";
        let payload = format!("{{\"version\":1,\"state\":\"{state}\"}}\n");
        let tmp = std::env::temp_dir().join("gatebound_stage_a_snapshot_v1_station_map.json");
        fs::write(&tmp, payload).expect("snapshot fixture write should pass");
        let loaded = Simulation::load_snapshot(&tmp, cfg).expect("snapshot load should pass");
        for system in &loaded.world.systems {
            assert!(
                loaded
                    .world
                    .stations_by_system
                    .get(&system.id)
                    .is_some_and(|stations| !stations.is_empty()),
                "every system must keep default station anchors on v1 load"
            );
        }
        assert!(
            loaded
                .ships
                .values()
                .all(|ship| ship.last_gate_arrival.is_none()),
            "v1 load should default last_gate_arrival to None"
        );
    }

    #[test]
    fn snapshot_v2_round_trip_preserves_station_and_ship_segment_state() {
        let cfg = stage_a_config();
        let mut sim = Simulation::new(cfg.clone(), 337);
        sim.ships.retain(|id, _| *id == ShipId(0));
        let gate_id = sim.world.edges.first().map(|edge| edge.id);
        if let Some(ship) = sim.ships.get_mut(&ShipId(0)) {
            ship.active_contract = None;
            ship.location = SystemId(0);
            ship.policy.waypoints = vec![SystemId(1)];
            ship.route_cursor = 0;
            ship.policy.max_risk_score = 10.0;
            ship.last_gate_arrival = gate_id;
        }
        sim.step_tick();
        let ship_before = sim
            .ships
            .get(&ShipId(0))
            .cloned()
            .expect("ship should exist");

        let tmp = std::env::temp_dir().join("gatebound_stage_a_snapshot_v2_station_ship.json");
        sim.save_snapshot(&tmp).expect("snapshot save should pass");
        let loaded = Simulation::load_snapshot(&tmp, cfg).expect("snapshot load should pass");
        let ship_after = loaded
            .ships
            .get(&ShipId(0))
            .expect("loaded ship should exist");

        assert_eq!(loaded.world.stations, sim.world.stations);
        assert_eq!(
            loaded.world.stations_by_system,
            sim.world.stations_by_system
        );
        assert_eq!(ship_after.movement_queue, ship_before.movement_queue);
        assert_eq!(
            ship_after.current_segment_kind,
            ship_before.current_segment_kind
        );
        assert_eq!(
            ship_after.segment_eta_remaining,
            ship_before.segment_eta_remaining
        );
        assert_eq!(ship_after.last_gate_arrival, ship_before.last_gate_arrival);
        let loaded_contract = loaded
            .contracts
            .get(&ContractId(0))
            .expect("contract should exist");
        let base_contract = sim
            .contracts
            .get(&ContractId(0))
            .expect("contract should exist");
        assert_eq!(loaded_contract.origin_station, base_contract.origin_station);
        assert_eq!(
            loaded_contract.destination_station,
            base_contract.destination_station
        );
    }

    #[test]
    fn snapshot_v2_preserves_last_gate_arrival() {
        let cfg = stage_a_config();
        let mut sim = Simulation::new(cfg.clone(), 341);
        let Some(gate_id) = sim.world.edges.first().map(|edge| edge.id) else {
            return;
        };
        if let Some(ship) = sim.ships.get_mut(&ShipId(0)) {
            ship.last_gate_arrival = Some(gate_id);
        }

        let tmp = std::env::temp_dir().join("gatebound_stage_a_snapshot_v2_last_gate_arrival.json");
        sim.save_snapshot(&tmp).expect("snapshot save should pass");
        let loaded = Simulation::load_snapshot(&tmp, cfg).expect("snapshot load should pass");
        let loaded_ship = loaded.ships.get(&ShipId(0)).expect("ship should exist");
        assert_eq!(loaded_ship.last_gate_arrival, Some(gate_id));
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
    fn lease_price_responds_to_throughput_gate_and_congestion() {
        let mut sim = Simulation::new(stage_a_config(), 59);
        let degrees = sim.world.degree_map();
        let min_degree_system = degrees
            .iter()
            .min_by_key(|(_, degree)| **degree)
            .map(|(system_id, _)| *system_id)
            .expect("system should exist");
        let max_degree_system = degrees
            .iter()
            .max_by_key(|(_, degree)| **degree)
            .map(|(system_id, _)| *system_id)
            .expect("system should exist");

        let min_degree_price = sim
            .lease_market_for_system(min_degree_system)
            .into_iter()
            .find(|entry| entry.slot_type == SlotType::Storage)
            .expect("slot entry exists")
            .price_per_cycle;
        let mut max_degree_price = sim
            .lease_market_for_system(max_degree_system)
            .into_iter()
            .find(|entry| entry.slot_type == SlotType::Storage)
            .expect("slot entry exists")
            .price_per_cycle;
        assert!(
            max_degree_price >= min_degree_price,
            "higher degree should not produce lower base lease price"
        );

        if let Some(book) = sim.markets.get_mut(&max_degree_system) {
            for state in book.goods.values_mut() {
                state.cycle_inflow = 60.0;
                state.cycle_outflow = 60.0;
            }
        }
        let throughput_price = sim
            .lease_market_for_system(max_degree_system)
            .into_iter()
            .find(|entry| entry.slot_type == SlotType::Storage)
            .expect("slot entry exists")
            .price_per_cycle;
        assert!(
            throughput_price > max_degree_price,
            "throughput increase should raise lease price"
        );
        max_degree_price = throughput_price;

        if let Some(neighbors) = sim.world.adjacency.get(&max_degree_system) {
            for (_, gate_id) in neighbors {
                sim.gate_queue_load.insert(*gate_id, 3.0);
            }
        }
        let congestion_price = sim
            .lease_market_for_system(max_degree_system)
            .into_iter()
            .find(|entry| entry.slot_type == SlotType::Storage)
            .expect("slot entry exists")
            .price_per_cycle;
        assert!(
            congestion_price >= max_degree_price,
            "congestion should not decrease lease price"
        );
    }

    #[test]
    fn lease_slot_respects_capacity_and_cycles() {
        let mut sim = Simulation::new(stage_a_config(), 61);
        let sid = SystemId(0);

        assert_eq!(
            sim.lease_slot(sid, SlotType::Dock, 0),
            Err(LeaseError::InvalidCycles)
        );
        assert_eq!(
            sim.lease_slot(SystemId(usize::MAX), SlotType::Dock, 1),
            Err(LeaseError::UnknownSystem)
        );

        for _ in 0..4 {
            sim.lease_slot(sid, SlotType::Dock, 2)
                .expect("dock lease should fit capacity");
        }
        assert_eq!(
            sim.lease_slot(sid, SlotType::Dock, 2),
            Err(LeaseError::NoCapacity)
        );
    }

    #[test]
    fn lease_expiration_frees_capacity_on_cycle_boundary() {
        let mut sim = Simulation::new(stage_a_config(), 67);
        let sid = SystemId(0);

        sim.lease_slot(sid, SlotType::Market, 1)
            .expect("lease should succeed");
        assert_eq!(sim.active_leases.len(), 1);

        for _ in 0..sim.config.time.cycle_ticks {
            sim.step_tick();
        }
        assert!(sim.active_leases.is_empty(), "1-cycle lease should expire");
        assert!(
            sim.lease_slot(sid, SlotType::Market, 1).is_ok(),
            "expired lease should free capacity"
        );
    }

    #[test]
    fn tick_upkeep_includes_ship_and_active_lease_costs() {
        let mut sim = Simulation::new(stage_a_config(), 71);
        let start_capital = sim.capital;
        let ships_count = sim.ships.len();
        let cycle_ticks = sim.config.time.cycle_ticks as f64;
        sim.lease_slot(SystemId(0), SlotType::Storage, 3)
            .expect("lease should succeed");
        let lease_price = sim
            .active_leases
            .first()
            .expect("lease should exist")
            .price_per_cycle;

        sim.step_tick();

        let expected_drop = sim.config.pressure.ship_upkeep_per_tick * ships_count as f64
            + lease_price / cycle_ticks;
        let actual_drop = start_capital - sim.capital;
        assert!(
            (actual_drop - expected_drop).abs() < 1e-6,
            "tick upkeep should include ships and lease upkeep"
        );
    }

    #[test]
    fn soft_fail_triggers_emergency_loan_without_game_over() {
        let mut sim = Simulation::new(stage_a_config(), 73);
        sim.capital = -30.0;

        sim.step_cycle();

        assert!(
            sim.capital >= 0.0,
            "recovery must restore non-negative capital"
        );
        assert!(sim.outstanding_debt > 0.0, "recovery should add debt");
        assert_eq!(sim.recovery_events, 1, "recovery counter should increment");

        let tick_before = sim.tick;
        sim.step_tick();
        assert!(
            sim.tick > tick_before,
            "simulation should continue after recovery"
        );
    }

    #[test]
    fn repeated_recovery_increases_interest_and_reduces_reputation() {
        let mut sim = Simulation::new(stage_a_config(), 79);
        let base_rate = sim.current_loan_interest_rate;
        let base_rep = sim.reputation;

        sim.capital = -1.0;
        sim.step_cycle();
        let rate_after_first = sim.current_loan_interest_rate;
        let rep_after_first = sim.reputation;

        sim.capital = -1.0;
        sim.step_cycle();

        assert!(
            rate_after_first > base_rate,
            "first recovery should raise rate"
        );
        assert!(
            sim.current_loan_interest_rate >= rate_after_first,
            "repeated recovery should not decrease rate"
        );
        assert!(
            sim.current_loan_interest_rate <= sim.config.pressure.recovery_rate_max,
            "rate should stay clamped to configured max"
        );
        assert!(
            rep_after_first < base_rep,
            "first recovery should reduce reputation"
        );
        assert!(
            sim.reputation <= rep_after_first && sim.reputation >= 0.0,
            "reputation should continue down but stay clamped"
        );
    }

    #[test]
    fn snapshot_v2_round_trip_preserves_leases_debt_reputation() {
        let cfg = stage_a_config();
        let mut sim = Simulation::new(cfg.clone(), 83);
        sim.lease_slot(SystemId(0), SlotType::Factory, 4)
            .expect("lease should succeed");
        sim.outstanding_debt = 222.5;
        sim.reputation = 0.66;
        sim.current_loan_interest_rate = 0.09;
        sim.recovery_events = 2;

        let tmp = std::env::temp_dir().join("gatebound_stage_a_snapshot_v2.json");
        sim.save_snapshot(&tmp).expect("snapshot save should pass");
        let loaded = Simulation::load_snapshot(&tmp, cfg).expect("snapshot load should pass");

        assert_eq!(loaded.active_leases, sim.active_leases);
        assert!((loaded.outstanding_debt - sim.outstanding_debt).abs() < 1e-9);
        assert!((loaded.reputation - sim.reputation).abs() < 1e-9);
        assert!((loaded.current_loan_interest_rate - sim.current_loan_interest_rate).abs() < 1e-9);
        assert_eq!(loaded.recovery_events, sim.recovery_events);
    }

    #[test]
    fn snapshot_v1_still_loads_with_default_new_fields() {
        let cfg = stage_a_config();
        let state =
            "tick=1;cycle=0;capital=500;qdelay=0;reroutes=0;sla_s=0;sla_f=0;edges=;ships=;contracts=;markets=;modifiers=";
        let payload = format!("{{\"version\":1,\"state\":\"{state}\"}}\n");
        let tmp = std::env::temp_dir().join("gatebound_stage_a_snapshot_v1.json");
        fs::write(&tmp, payload).expect("snapshot fixture write should pass");

        let loaded =
            Simulation::load_snapshot(&tmp, cfg.clone()).expect("snapshot load should pass");
        assert_eq!(loaded.outstanding_debt, 0.0);
        assert!((loaded.reputation - 1.0).abs() < 1e-9);
        assert!((loaded.current_loan_interest_rate - cfg.pressure.loan_interest_rate).abs() < 1e-9);
        assert_eq!(loaded.recovery_events, 0);
        assert!(loaded.active_leases.is_empty());
    }

    #[test]
    fn offer_generation_reflects_market_imbalance_and_risk() {
        let mut sim = Simulation::new(stage_a_config(), 101);
        sim.refresh_contract_offers();
        let baseline = sim
            .contract_offers
            .values()
            .next()
            .expect("offer must exist")
            .quantity;

        if let Some(market) = sim.markets.get_mut(&SystemId(1)) {
            for state in market.goods.values_mut() {
                state.stock = 10.0;
                state.target_stock = 200.0;
                state.cycle_outflow = 70.0;
                state.cycle_inflow = 10.0;
            }
        }
        sim.refresh_contract_offers();
        let stressed = sim
            .contract_offers
            .values()
            .next()
            .expect("offer must exist")
            .quantity;
        assert!(
            stressed >= baseline,
            "higher imbalance should increase offer size"
        );
    }

    #[test]
    fn accept_offer_creates_contract_and_removes_offer() {
        let mut sim = Simulation::new(stage_a_config(), 103);
        if let Some(ship) = sim.ships.get_mut(&ShipId(0)) {
            ship.active_contract = None;
        }
        if let Some(contract) = sim.contracts.get_mut(&ContractId(0)) {
            contract.completed = true;
        }
        sim.refresh_contract_offers();
        let offer_id = *sim
            .contract_offers
            .keys()
            .next()
            .expect("offer must exist for acceptance");
        let cid = sim
            .accept_contract_offer(offer_id, ShipId(0))
            .expect("offer acceptance should pass");
        assert!(sim.contracts.contains_key(&cid));
        assert!(
            !sim.contract_offers.contains_key(&offer_id),
            "accepted offer should be removed"
        );
    }

    #[test]
    fn offer_expiration_and_refresh_work_by_cycle() {
        let mut cfg = stage_a_config();
        cfg.pressure.offer_refresh_cycles = 1;
        cfg.pressure.offer_ttl_cycles = 1;
        let mut sim = Simulation::new(cfg, 107);
        sim.refresh_contract_offers();
        let first_offer_ids = sim.contract_offers.keys().copied().collect::<Vec<_>>();

        sim.step_cycle();
        sim.step_cycle();

        let has_old = first_offer_ids
            .iter()
            .any(|offer_id| sim.contract_offers.contains_key(offer_id));
        assert!(!has_old, "expired offers should be replaced on refresh");
    }

    #[test]
    fn gate_fee_is_charged_per_warp_segment() {
        let mut cfg = stage_a_config();
        cfg.pressure.ship_upkeep_per_tick = 0.0;
        cfg.pressure.gate_fee_per_jump = 3.5;
        let mut sim = Simulation::new(cfg, 109);
        sim.ships.retain(|ship_id, _| *ship_id == ShipId(0));
        if let Some(ship) = sim.ships.get_mut(&ShipId(0)) {
            ship.active_contract = None;
            ship.location = SystemId(0);
            ship.policy.waypoints = vec![SystemId(1)];
            ship.route_cursor = 0;
            ship.policy.max_risk_score = 10.0;
        }
        let before = sim.capital;
        for _ in 0..32 {
            sim.step_tick();
            if before - sim.capital >= 3.5 {
                break;
            }
        }
        assert!(
            before - sim.capital >= 3.5,
            "gate fee should be charged when warp segment starts"
        );
    }

    #[test]
    fn market_fee_applies_to_payouts() {
        let mut cfg = stage_a_config();
        cfg.pressure.ship_upkeep_per_tick = 0.0;
        cfg.pressure.gate_fee_per_jump = 0.0;
        cfg.pressure.market_fee_rate = 0.2;
        let mut sim = Simulation::new(cfg, 113);
        sim.ships.retain(|ship_id, _| *ship_id == ShipId(0));
        if let Some(contract) = sim.contracts.get_mut(&ContractId(0)) {
            contract.completed = false;
            contract.failed = false;
            contract.destination = SystemId(0);
            contract.assigned_ship = Some(ShipId(0));
            contract.payout = 100.0;
            contract.deadline_tick = 1_000;
        }
        if let Some(ship) = sim.ships.get_mut(&ShipId(0)) {
            ship.location = SystemId(0);
            ship.eta_ticks_remaining = 0;
            ship.active_contract = Some(ContractId(0));
        }

        let before = sim.capital;
        sim.step_tick();
        let delta = sim.capital - before;
        assert!(
            (delta - 80.0).abs() < 1e-6,
            "payout should include market fee deduction"
        );
    }

    #[test]
    fn market_depth_caps_effective_supply_delivery() {
        let mut cfg = stage_a_config();
        cfg.pressure.market_depth_per_cycle = 5.0;
        let mut sim = Simulation::new(cfg, 127);
        let cid = sim.create_supply_contract(SystemId(0), SystemId(1), 10.0, 3);
        if let Some(contract) = sim.contracts.get_mut(&cid) {
            contract.delivered_amount = 10.0;
            contract.per_cycle = 10.0;
            contract.payout = 40.0;
            contract.penalty = 12.0;
        }
        let before = sim.capital;
        sim.step_cycle();
        assert!(
            sim.capital < before,
            "depth cap should turn apparent full delivery into shortfall penalty"
        );
    }

    #[test]
    fn npc_stage_a_baseline_roster_is_created() {
        let sim = Simulation::new(stage_a_config(), 131);
        assert_eq!(sim.companies.len(), 5);
        assert!(
            sim.ships.len() >= 7 && sim.ships.len() <= 11,
            "stage A ship count should stay in baseline range"
        );
        assert!(sim
            .companies
            .values()
            .any(|company| company.archetype == CompanyArchetype::Hauler));
        assert!(sim
            .companies
            .values()
            .any(|company| company.archetype == CompanyArchetype::Miner));
        assert!(sim
            .companies
            .values()
            .any(|company| company.archetype == CompanyArchetype::Industrial));
    }

    #[test]
    fn throughput_window_computes_player_share() {
        let mut sim = Simulation::new(stage_a_config(), 137);
        let gate = sim.world.edges.first().expect("edge exists").id;
        let mut cycle_map = BTreeMap::new();
        cycle_map.insert(
            gate,
            BTreeMap::from([(CompanyId(0), 3_u32), (CompanyId(1), 1_u32)]),
        );
        sim.gate_traversals_window.clear();
        sim.gate_traversals_window.push_back(cycle_map);

        let snapshot = sim
            .gate_throughput_view()
            .into_iter()
            .find(|entry| entry.gate_id == gate)
            .expect("gate throughput should exist");
        assert!((snapshot.player_share - 0.75).abs() < 1e-9);
    }

    #[test]
    fn milestones_complete_when_targets_reached() {
        let mut cfg = stage_a_config();
        cfg.pressure.milestone_capital_target = 100.0;
        cfg.pressure.milestone_throughput_target_share = 0.2;
        cfg.pressure.milestone_reputation_target = 0.4;
        let mut sim = Simulation::new(cfg, 149);
        sim.capital = 500.0;
        sim.reputation = 0.9;
        let gate = sim.world.edges.first().expect("edge exists").id;
        sim.gate_traversals_cycle.insert(
            gate,
            BTreeMap::from([(CompanyId(0), 2_u32), (CompanyId(1), 1_u32)]),
        );
        sim.step_cycle();
        assert!(
            sim.milestones.iter().all(|milestone| milestone.completed),
            "all milestones should complete once thresholds are crossed"
        );
    }

    #[test]
    fn snapshot_v1_v2_load_defaults_for_new_fields() {
        let cfg = stage_a_config();
        let v1_state =
            "tick=1;cycle=0;capital=500;qdelay=0;reroutes=0;sla_s=0;sla_f=0;edges=;ships=;contracts=;markets=;modifiers=";
        let v1_payload = format!("{{\"version\":1,\"state\":\"{v1_state}\"}}\n");
        let v1_tmp = std::env::temp_dir().join("gatebound_stage_a_snapshot_v1_defaults.json");
        fs::write(&v1_tmp, v1_payload).expect("snapshot fixture write should pass");
        let loaded_v1 =
            Simulation::load_snapshot(&v1_tmp, cfg.clone()).expect("v1 snapshot load should pass");
        assert!(!loaded_v1.companies.is_empty());
        assert!(!loaded_v1.milestones.is_empty());

        let mut sim = Simulation::new(cfg.clone(), 151);
        sim.refresh_contract_offers();
        let v2_tmp = std::env::temp_dir().join("gatebound_stage_a_snapshot_v2_defaults.json");
        sim.save_snapshot(&v2_tmp)
            .expect("snapshot save should pass");
        let loaded_v2 =
            Simulation::load_snapshot(&v2_tmp, cfg).expect("v2 snapshot load should pass");
        assert_eq!(loaded_v2.companies, sim.companies);
        assert_eq!(loaded_v2.milestones, sim.milestones);
        assert_eq!(loaded_v2.contract_offers, sim.contract_offers);
    }

    #[test]
    fn market_share_milestone_completes_on_window_share() {
        let mut cfg = stage_a_config();
        cfg.pressure.milestone_market_share_target = 0.5;
        let mut sim = Simulation::new(cfg, 211);
        let gate = sim.world.edges.first().expect("edge exists").id;
        sim.gate_traversals_window.clear();
        sim.gate_traversals_window.push_back(BTreeMap::from([(
            gate,
            BTreeMap::from([(CompanyId(0), 6_u32), (CompanyId(1), 2_u32)]),
        )]));
        sim.update_milestones();
        let market_share = sim
            .milestones
            .iter()
            .find(|milestone| milestone.id == MilestoneId::MarketShare)
            .expect("market share milestone exists");
        assert!(market_share.completed);
        assert!(market_share.current >= 0.5);
    }

    #[test]
    fn offer_generation_populates_route_gates_problem_and_profit_per_ton() {
        let mut sim = Simulation::new(stage_a_config(), 223);
        sim.refresh_contract_offers();
        let offer = sim
            .contract_offers
            .values()
            .next()
            .expect("offer should exist");
        assert!(offer.profit_per_ton.is_finite());
        assert!(offer.profit_per_ton.abs() < 1_000.0);
        assert!(matches!(
            offer.problem_tag,
            OfferProblemTag::HighRisk
                | OfferProblemTag::CongestedRoute
                | OfferProblemTag::LowMargin
                | OfferProblemTag::FuelVolatility
        ));
    }

    #[test]
    fn premium_offer_requires_reputation_threshold() {
        let mut cfg = stage_a_config();
        cfg.pressure.premium_offer_reputation_min = 0.9;
        let mut sim = Simulation::new(cfg, 227);
        sim.reputation = 0.5;
        sim.refresh_contract_offers();
        assert!(
            sim.contract_offers.values().all(|offer| !offer.premium),
            "low reputation should suppress premium offers"
        );
        sim.reputation = 0.95;
        sim.refresh_contract_offers();
        assert!(
            sim.contract_offers.values().all(|offer| offer.premium),
            "high reputation should enable premium offers"
        );
    }

    #[test]
    fn fleet_status_exposes_job_queue_and_kpis() {
        let mut sim = Simulation::new(stage_a_config(), 229);
        let ship_id = ShipId(0);
        sim.ship_idle_ticks_cycle.insert(ship_id, 5);
        sim.ship_delay_ticks_cycle.insert(ship_id, 12);
        sim.ship_runs_completed.insert(ship_id, 3);
        sim.ship_profit_earned.insert(ship_id, 90.0);
        if let Some(ship) = sim.ships.get_mut(&ship_id) {
            ship.planned_path = vec![SystemId(1), SystemId(2)];
            ship.active_contract = Some(ContractId(0));
        }

        let row = sim
            .fleet_status()
            .into_iter()
            .find(|row| row.ship_id == ship_id)
            .expect("ship row should exist");
        assert_eq!(row.idle_ticks_cycle, 5);
        assert!(row.avg_delay_ticks_cycle > 0.0);
        assert!(row.profit_per_run > 0.0);
        assert!(!row.job_queue.is_empty());
    }

    #[test]
    fn market_insights_produce_trend_forecast_and_factors() {
        let mut sim = Simulation::new(stage_a_config(), 233);
        let system_id = SystemId(0);
        if let Some(book) = sim.markets.get_mut(&system_id) {
            if let Some(fuel) = book.goods.get_mut(&Commodity::Fuel) {
                fuel.stock = 40.0;
                fuel.target_stock = 100.0;
                fuel.cycle_outflow = 15.0;
                fuel.cycle_inflow = 5.0;
            }
        }
        sim.capture_previous_cycle_prices();
        sim.update_market_prices();
        let rows = sim.market_insights(system_id);
        assert!(!rows.is_empty());
        let fuel_row = rows
            .iter()
            .find(|row| row.commodity == Commodity::Fuel)
            .expect("fuel row should exist");
        assert!(fuel_row.forecast_next.is_finite());
        assert!(fuel_row.imbalance_factor.is_finite());
        assert!(fuel_row.congestion_factor.is_finite());
    }

    #[test]
    fn soft_fail_releases_expensive_leases_and_records_recovery_action() {
        let mut sim = Simulation::new(stage_a_config(), 239);
        sim.lease_slot(SystemId(0), SlotType::Factory, 3)
            .expect("factory lease should succeed");
        sim.lease_slot(SystemId(0), SlotType::Dock, 3)
            .expect("dock lease should succeed");
        let before = sim.active_leases.len();
        sim.capital = -50.0;
        sim.step_cycle();
        assert!(sim.active_leases.len() < before);
        assert!(!sim.recovery_log.is_empty());
        let action = sim
            .recovery_log
            .last()
            .expect("recovery action should exist");
        assert!(action.released_leases >= 1);
    }

    #[test]
    fn snapshot_v1_v2_load_defaults_for_new_stage_a_fields() {
        let cfg = stage_a_config();
        let v1_state =
            "tick=1;cycle=0;capital=500;qdelay=0;reroutes=0;sla_s=0;sla_f=0;edges=;ships=;contracts=;markets=;modifiers=";
        let v1_payload = format!("{{\"version\":1,\"state\":\"{v1_state}\"}}\n");
        let v1_tmp =
            std::env::temp_dir().join("gatebound_stage_a_snapshot_v1_defaults_stage_a_fields.json");
        fs::write(&v1_tmp, v1_payload).expect("snapshot fixture write should pass");
        let loaded_v1 =
            Simulation::load_snapshot(&v1_tmp, cfg.clone()).expect("v1 snapshot load should pass");
        assert!(loaded_v1.ship_runs_completed.is_empty());
        assert!(loaded_v1.recovery_log.is_empty());

        let mut sim = Simulation::new(cfg.clone(), 241);
        sim.ship_runs_completed.insert(ShipId(0), 4);
        sim.ship_profit_earned.insert(ShipId(0), 80.0);
        sim.recovery_log.push(RecoveryAction {
            cycle: 3,
            released_leases: 1,
            capital_after: 42.0,
            debt_after: 180.0,
        });
        let v2_tmp =
            std::env::temp_dir().join("gatebound_stage_a_snapshot_v2_defaults_stage_a_fields.json");
        sim.save_snapshot(&v2_tmp)
            .expect("snapshot save should pass");
        let loaded_v2 =
            Simulation::load_snapshot(&v2_tmp, cfg).expect("v2 snapshot load should pass");
        assert_eq!(
            loaded_v2.ship_runs_completed.get(&ShipId(0)).copied(),
            Some(4)
        );
        assert_eq!(
            loaded_v2.ship_profit_earned.get(&ShipId(0)).copied(),
            Some(80.0)
        );
        assert_eq!(loaded_v2.recovery_log, sim.recovery_log);
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
