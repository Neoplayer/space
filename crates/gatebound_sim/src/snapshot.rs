use crate::{simulation::SnapshotError, Simulation};
use gatebound_domain::{
    ActiveLoan, Commodity, Company, CompanyId, Contract, ContractOffer, GateId, MarketState,
    MilestoneStatus, NpcCompanyRuntime, RiskStageA, RuntimeConfig, Ship, ShipId, StationId,
    TradeOrder,
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::Path;

const SNAPSHOT_VERSION: u32 = 4;

#[derive(Debug, Clone, Serialize)]
struct SnapshotEnvelope {
    version: u32,
    state: SnapshotState,
}

#[derive(Debug, Deserialize)]
struct SnapshotEnvelopeValue {
    version: u32,
    state: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SnapshotState {
    pub world_seed: u64,
    pub tick: u64,
    pub cycle: u64,
    pub capital: f64,
    pub active_loan: Option<ActiveLoan>,
    pub outstanding_debt: f64,
    pub reputation: f64,
    pub current_loan_interest_rate: f64,
    pub queue_delay_accumulator: u64,
    pub reroute_count: u64,
    pub sla_successes: u64,
    pub sla_failures: u64,
    pub next_offer_id: u64,
    pub next_trade_order_id: u64,
    pub edges: Vec<EdgeSnapshot>,
    pub companies: Vec<Company>,
    pub company_runtimes: Vec<NpcCompanyRuntime>,
    pub markets: Vec<MarketBookSnapshot>,
    #[serde(default)]
    pub player_station_storage: Vec<StationStorageSnapshot>,
    pub contracts: Vec<Contract>,
    pub contract_offers: Vec<ContractOffer>,
    pub trade_orders: Vec<TradeOrder>,
    pub ships: Vec<Ship>,
    pub milestones: Vec<MilestoneStatus>,
    pub gate_traversals_cycle: Vec<GateTraversalSnapshot>,
    pub gate_traversals_window: Vec<Vec<GateTraversalSnapshot>>,
    pub gate_queue_load: Vec<GateLoadSnapshot>,
    pub ship_kpis: Vec<ShipKpiSnapshot>,
    pub previous_cycle_prices: Vec<PreviousPriceSnapshot>,
    pub modifiers: Vec<ActiveModifierSnapshot>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub(crate) struct EdgeSnapshot {
    pub gate_id: GateId,
    pub capacity_factor: f64,
    pub blocked_until_tick: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct MarketBookSnapshot {
    pub station_id: StationId,
    pub goods: Vec<MarketGoodSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct MarketGoodSnapshot {
    pub commodity: Commodity,
    pub state: MarketState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct StationStorageSnapshot {
    pub station_id: StationId,
    pub goods: Vec<StoredCommoditySnapshot>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub(crate) struct StoredCommoditySnapshot {
    pub commodity: Commodity,
    pub amount: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct GateTraversalSnapshot {
    pub gate_id: GateId,
    pub by_company: Vec<CompanyTraversalSnapshot>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub(crate) struct CompanyTraversalSnapshot {
    pub company_id: CompanyId,
    pub count: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub(crate) struct GateLoadSnapshot {
    pub gate_id: GateId,
    pub load: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub(crate) struct ShipKpiSnapshot {
    pub ship_id: ShipId,
    pub idle_ticks_cycle: u32,
    pub delay_ticks_cycle: u32,
    pub runs_completed: u32,
    pub profit_earned: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub(crate) struct PreviousPriceSnapshot {
    pub station_id: StationId,
    pub commodity: Commodity,
    pub price: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub(crate) struct ActiveModifierSnapshot {
    pub until_tick: u64,
    pub gate: Option<GateId>,
    pub risk: RiskStageA,
    pub magnitude: f64,
}

pub fn save_snapshot(simulation: &Simulation, path: &Path) -> Result<(), SnapshotError> {
    let payload = serialize_snapshot(simulation)?;
    fs::write(path, format!("{payload}\n"))
        .map_err(|error| SnapshotError::Io(format!("save failed: {error}")))
}

pub fn load_snapshot(path: &Path, config: RuntimeConfig) -> Result<Simulation, SnapshotError> {
    let payload = fs::read_to_string(path)
        .map_err(|error| SnapshotError::Io(format!("load failed: {error}")))?;
    deserialize_snapshot(&payload, config)
}

pub fn serialize_snapshot(simulation: &Simulation) -> Result<String, SnapshotError> {
    let envelope = SnapshotEnvelope {
        version: SNAPSHOT_VERSION,
        state: simulation.snapshot_state(),
    };
    serde_json::to_string_pretty(&envelope)
        .map_err(|error| SnapshotError::Parse(format!("snapshot serialize failed: {error}")))
}

pub fn deserialize_snapshot(
    payload: &str,
    config: RuntimeConfig,
) -> Result<Simulation, SnapshotError> {
    let envelope: SnapshotEnvelopeValue = serde_json::from_str(payload)
        .map_err(|error| SnapshotError::Parse(format!("snapshot parse failed: {error}")))?;
    if envelope.version != 3 && envelope.version != SNAPSHOT_VERSION {
        return Err(SnapshotError::Parse(format!(
            "unsupported snapshot version: {}",
            envelope.version
        )));
    }
    let state: SnapshotState = serde_json::from_value(envelope.state)
        .map_err(|error| SnapshotError::Parse(format!("snapshot state parse failed: {error}")))?;
    Ok(Simulation::from_snapshot_state(config, state))
}

pub fn snapshot_hash(simulation: &Simulation) -> u64 {
    let payload = serde_json::to_vec(&simulation.snapshot_state())
        .expect("snapshot state serialization should be infallible");
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    payload.hash(&mut hasher);
    hasher.finish()
}
