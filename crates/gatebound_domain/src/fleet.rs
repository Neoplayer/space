use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

use crate::{
    CargoLoad, CargoManifest, CargoSource, Commodity, CompanyId, GateId, RouteSegment, SegmentKind,
    ShipId, StationId, SystemId, TradeOrderId,
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
    Player,
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
            max_hops: 24,
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
    pub cargo_lots: Vec<CargoLoad>,
    pub cargo_total_amount: f64,
    pub cargo_used_capacity: f64,
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
    pub company_id: CompanyId,
    pub ship_id: ShipId,
    pub commodity: Commodity,
    pub amount: f64,
    #[serde(default)]
    pub purchased_amount: f64,
    #[serde(default)]
    pub cost_basis_total: f64,
    #[serde(default)]
    pub gate_fees_accrued: f64,
    pub source_station: StationId,
    pub destination_station: StationId,
    pub stage: TradeOrderStage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ShipClass {
    #[default]
    Courier,
    Hauler,
    Miner,
    Industrial,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShipModuleSlot {
    Command,
    Drive,
    Cargo,
    Utility,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShipModuleStatus {
    Optimal,
    Serviceable,
    Worn,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShipDescriptor {
    pub name: String,
    pub class: ShipClass,
    pub description: String,
}

impl Default for ShipDescriptor {
    fn default() -> Self {
        Self {
            name: "Registry Ghost".to_string(),
            class: ShipClass::Courier,
            description: "Recovered hull awaiting refreshed registry metadata.".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShipModule {
    pub slot: ShipModuleSlot,
    pub name: String,
    pub status: ShipModuleStatus,
    pub details: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShipTechnicalState {
    pub hull: f64,
    pub drive: f64,
    pub reactor: f64,
    pub sensors: f64,
    pub cargo_bay: f64,
    pub maintenance_note: String,
}

impl Default for ShipTechnicalState {
    fn default() -> Self {
        Self {
            hull: 82.0,
            drive: 78.0,
            reactor: 84.0,
            sensors: 76.0,
            cargo_bay: 81.0,
            maintenance_note: "Legacy registry restored without shipyard service history."
                .to_string(),
        }
    }
}

fn default_ship_modules() -> Vec<ShipModule> {
    vec![
        ShipModule {
            slot: ShipModuleSlot::Command,
            name: "Registry Bridge".to_string(),
            status: ShipModuleStatus::Serviceable,
            details: "Flight control and traffic handshake suite recovered from baseline hull."
                .to_string(),
        },
        ShipModule {
            slot: ShipModuleSlot::Drive,
            name: "Baseline Torch Drive".to_string(),
            status: ShipModuleStatus::Serviceable,
            details: "Sub-light propulsion tuned for standard intra-system hauling.".to_string(),
        },
        ShipModule {
            slot: ShipModuleSlot::Cargo,
            name: "Modular Cargo Lattice".to_string(),
            status: ShipModuleStatus::Serviceable,
            details: "General-purpose hold frame backfilled for legacy snapshot compatibility."
                .to_string(),
        },
    ]
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
    #[serde(default)]
    pub cargo: CargoManifest,
    pub trade_order_id: Option<TradeOrderId>,
    pub movement_queue: VecDeque<RouteSegment>,
    pub segment_eta_remaining: u32,
    pub segment_progress_total: u32,
    pub current_segment_kind: Option<SegmentKind>,
    pub route_cursor: usize,
    pub policy: AutopilotPolicy,
    pub planned_path: Vec<SystemId>,
    pub current_target: Option<SystemId>,
    pub last_gate_arrival: Option<GateId>,
    pub last_risk_score: f64,
    pub reroutes: u64,
    #[serde(default)]
    pub descriptor: ShipDescriptor,
    #[serde(default = "default_ship_modules")]
    pub modules: Vec<ShipModule>,
    #[serde(default)]
    pub technical_state: ShipTechnicalState,
}

impl Ship {
    pub fn cargo_lots(&self) -> &[CargoLoad] {
        self.cargo.lots()
    }

    pub fn cargo_total_amount(&self) -> f64 {
        self.cargo.total_amount()
    }

    pub fn cargo_used_capacity(&self) -> f64 {
        self.cargo_total_amount()
    }

    pub fn remaining_capacity(&self) -> f64 {
        self.cargo.remaining_capacity(self.cargo_capacity)
    }

    pub fn cargo_amount(&self, commodity: Commodity) -> f64 {
        self.cargo_lots()
            .iter()
            .filter(|cargo| cargo.commodity == commodity)
            .map(|cargo| cargo.amount)
            .sum()
    }

    pub fn amount_for(&self, commodity: Commodity, source: CargoSource) -> f64 {
        self.cargo.amount_for(commodity, source)
    }

    pub fn spot_amount(&self, commodity: Commodity) -> f64 {
        self.cargo.spot_amount(commodity)
    }

    pub fn has_locked_cargo(&self) -> bool {
        self.cargo.has_locked_cargo()
    }

    pub fn has_spot_cargo(&self) -> bool {
        self.cargo.has_spot_cargo()
    }

    pub fn largest_spot_commodity(&self) -> Option<Commodity> {
        self.cargo.largest_spot_commodity()
    }

    pub fn upsert_lot(&mut self, commodity: Commodity, source: CargoSource, amount: f64) {
        self.cargo.upsert_lot(commodity, source, amount);
    }

    pub fn remove_amount(&mut self, commodity: Commodity, source: CargoSource, amount: f64) -> f64 {
        self.cargo.remove_amount(commodity, source, amount)
    }
}
