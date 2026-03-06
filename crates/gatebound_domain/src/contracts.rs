use serde::{Deserialize, Serialize};

use crate::{Commodity, ContractId, GateId, ShipId, StationId, SystemId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContractTypeStageA {
    Delivery,
    Supply,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContractProgress {
    AwaitPickup,
    InTransit,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OfferProblemTag {
    HighRisk,
    CongestedRoute,
    LowMargin,
    FuelVolatility,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContractOffer {
    pub id: u64,
    pub kind: ContractTypeStageA,
    pub commodity: Commodity,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Contract {
    pub id: ContractId,
    pub kind: ContractTypeStageA,
    pub progress: ContractProgress,
    pub commodity: Commodity,
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
    pub loaded_amount: f64,
    pub delivered_cycle_amount: f64,
    pub delivered_amount: f64,
    pub missed_cycles: u32,
    pub completed: bool,
    pub failed: bool,
    pub last_eval_cycle: u64,
}
