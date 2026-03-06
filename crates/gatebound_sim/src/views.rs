use gatebound_domain::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeSettingsView {
    pub tick_seconds: u32,
    pub cycle_ticks: u32,
    pub rolling_window_cycles: u32,
    pub day_ticks: u32,
    pub days_per_month: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CameraStationView {
    pub station_id: StationId,
    pub profile: StationProfile,
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CameraSystemView {
    pub system_id: SystemId,
    pub x: f64,
    pub y: f64,
    pub radius: f64,
    pub stations: Vec<CameraStationView>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CameraTopologyView {
    pub systems: Vec<CameraSystemView>,
    pub gate_ids: Vec<GateId>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenderGateNodeView {
    pub gate_id: GateId,
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenderStationView {
    pub station_id: StationId,
    pub profile: StationProfile,
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenderSystemView {
    pub system_id: SystemId,
    pub x: f64,
    pub y: f64,
    pub radius: f64,
    pub gate_nodes: Vec<RenderGateNodeView>,
    pub stations: Vec<RenderStationView>,
    pub dock_congestion: f32,
    pub fuel_stress: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenderEdgeView {
    pub gate_id: GateId,
    pub from_system: SystemId,
    pub to_system: SystemId,
    pub load: f64,
    pub effective_capacity: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenderShipView {
    pub ship_id: ShipId,
    pub company_id: CompanyId,
    pub location: SystemId,
    pub current_station: Option<StationId>,
    pub current_target: Option<SystemId>,
    pub eta_ticks_remaining: u32,
    pub segment_eta_remaining: u32,
    pub segment_progress_total: u32,
    pub current_segment_kind: Option<SegmentKind>,
    pub front_segment: Option<RouteSegment>,
    pub cargo: Option<CargoLoad>,
    pub last_gate_arrival: Option<GateId>,
    pub last_risk_score: f64,
    pub reroutes: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WorldRenderSnapshot {
    pub tick: u64,
    pub systems: Vec<RenderSystemView>,
    pub edges: Vec<RenderEdgeView>,
    pub ships: Vec<RenderShipView>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ContractOfferView {
    pub offer: ContractOffer,
    pub destination_intel: Option<MarketIntel>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ContractsBoardView {
    pub active_contracts: Vec<Contract>,
    pub route_gates: Vec<GateId>,
    pub offers: Vec<ContractOfferView>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FleetPanelView {
    pub rows: Vec<FleetShipStatus>,
    pub player_ship_ids: Vec<ShipId>,
    pub default_player_ship_id: Option<ShipId>,
    pub avg_route_hops_player: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MarketRowView {
    pub commodity: Commodity,
    pub price: f64,
    pub stock: f64,
    pub inflow: f64,
    pub outflow: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MarketPanelView {
    pub selected_system_id: SystemId,
    pub selected_station_id: Option<StationId>,
    pub selected_station_profile: Option<StationProfile>,
    pub intel: Option<MarketIntel>,
    pub station_market_rows: Vec<MarketRowView>,
    pub system_market_rows: Vec<MarketRowView>,
    pub throughput_rows: Vec<GateThroughputSnapshot>,
    pub market_share: f64,
    pub market_insights: Vec<MarketInsightRow>,
    pub avg_price_index: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LoanOfferView {
    pub id: LoanOfferId,
    pub label: &'static str,
    pub principal: f64,
    pub monthly_interest_rate: f64,
    pub term_months: u32,
    pub monthly_payment: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ActiveLoanView {
    pub offer_id: LoanOfferId,
    pub principal_remaining: f64,
    pub monthly_interest_rate: f64,
    pub remaining_months: u32,
    pub next_payment: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FinancePanelView {
    pub debt: f64,
    pub interest_rate: f64,
    pub reputation: f64,
    pub active_loan: Option<ActiveLoanView>,
    pub loan_offers: Vec<LoanOfferView>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StationOpsView {
    pub ship_id: ShipId,
    pub station_id: StationId,
    pub docked: bool,
    pub cargo: Option<CargoLoad>,
    pub cargo_capacity: f64,
    pub active_contract: Option<Contract>,
    pub market_rows: Vec<MarketRowView>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShipPolicyView {
    pub ship_id: ShipId,
    pub policy: AutopilotPolicy,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HudOverviewView {
    pub tick: u64,
    pub cycle: u64,
    pub capital: f64,
    pub debt: f64,
    pub interest_rate: f64,
    pub reputation: f64,
    pub active_contracts: usize,
    pub active_ships: usize,
    pub sla_success_rate: f64,
    pub reroutes: u64,
    pub avg_price_index: f64,
    pub market_share: f64,
    pub milestones: Vec<MilestoneStatus>,
}
