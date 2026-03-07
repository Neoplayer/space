use serde::{Deserialize, Serialize};

use crate::{Commodity, GateId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RiskStageA {
    GateCongestion,
    DockCongestion,
    FuelShock,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MilestoneId {
    Capital,
    MarketShare,
    ThroughputControl,
    Reputation,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MilestoneStatus {
    pub id: MilestoneId,
    pub current: f64,
    pub target: f64,
    pub completed: bool,
    pub completed_cycle: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GateThroughputSnapshot {
    pub gate_id: GateId,
    pub player_share: f64,
    pub total_flow: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TickReport {
    pub tick: u64,
    pub cycle: u64,
    pub active_ships: usize,
    pub active_missions: usize,
    pub total_queue_delay: u64,
    pub avg_price_index: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CycleReport {
    pub cycle: u64,
    pub sla_success_rate: f64,
    pub reroute_count: u64,
    pub economy_stress_index: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MarketInsightRow {
    pub commodity: Commodity,
    pub trend_delta: f64,
    pub forecast_next: f64,
    pub imbalance_factor: f64,
    pub congestion_factor: f64,
    pub fuel_factor: f64,
}
