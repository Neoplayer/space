use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

use crate::{
    CargoLoad, Commodity, CompanyId, ContractId, GateId, RouteSegment, SegmentKind, ShipId,
    StationId, SystemId, TradeOrderId,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PriorityMode {
    Profit,
    Stability,
    Hybrid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RepeatMode {
    Loop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShipRole {
    PlayerContract,
    NpcTrade,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FleetWarning {
    HighRisk,
    HighQueueDelay,
    NoRoute,
    ShipIdle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FleetJobKind {
    Pickup,
    Transit,
    GateQueue,
    Warp,
    Unload,
    LoopReturn,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct FleetJobStep {
    pub kind: FleetJobKind,
    pub system: SystemId,
    pub eta_ticks: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FleetShipStatus {
    pub ship_id: ShipId,
    pub company_id: CompanyId,
    pub role: ShipRole,
    pub location: SystemId,
    pub current_station: Option<StationId>,
    pub target: Option<SystemId>,
    pub eta: u32,
    pub active_contract: Option<ContractId>,
    pub cargo_commodity: Option<Commodity>,
    pub cargo_amount: f64,
    pub route_len: usize,
    pub reroutes: u64,
    pub warning: Option<FleetWarning>,
    pub job_queue: Vec<FleetJobStep>,
    pub idle_ticks_cycle: u32,
    pub avg_delay_ticks_cycle: f64,
    pub profit_per_run: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TradeOrderStage {
    ToPickup,
    ToDropoff,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TradeOrder {
    pub id: TradeOrderId,
    pub ship_id: ShipId,
    pub commodity: Commodity,
    pub amount: f64,
    pub source_station: StationId,
    pub destination_station: StationId,
    pub stage: TradeOrderStage,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Ship {
    pub id: ShipId,
    pub company_id: CompanyId,
    pub role: ShipRole,
    pub location: SystemId,
    pub current_station: Option<StationId>,
    pub eta_ticks_remaining: u32,
    pub sub_light_speed: f64,
    pub cargo_capacity: f64,
    pub cargo: Option<CargoLoad>,
    pub trade_order_id: Option<TradeOrderId>,
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
