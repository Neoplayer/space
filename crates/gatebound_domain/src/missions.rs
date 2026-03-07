use serde::{Deserialize, Serialize};

use crate::{Commodity, GateId, MissionId, StationId, SystemId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MissionKind {
    Transport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MissionStatus {
    Accepted,
    InProgress,
    Completed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MissionOffer {
    pub id: u64,
    pub kind: MissionKind,
    pub commodity: Commodity,
    pub origin: SystemId,
    pub destination: SystemId,
    pub origin_station: StationId,
    pub destination_station: StationId,
    pub quantity: f64,
    pub reward: f64,
    pub penalty: f64,
    pub eta_ticks: u32,
    pub risk_score: f64,
    pub score: f64,
    pub route_gate_ids: Vec<GateId>,
    pub expires_cycle: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Mission {
    pub id: MissionId,
    pub kind: MissionKind,
    pub status: MissionStatus,
    pub commodity: Commodity,
    pub origin: SystemId,
    pub destination: SystemId,
    pub origin_station: StationId,
    pub destination_station: StationId,
    pub quantity: f64,
    pub reward: f64,
    pub penalty: f64,
    pub eta_ticks: u32,
    pub risk_score: f64,
    pub route_gate_ids: Vec<GateId>,
    pub accepted_tick: u64,
    pub accepted_cycle: u64,
    pub loaded_amount: f64,
    pub delivered_amount: f64,
}
