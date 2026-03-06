use gatebound_domain::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeSettingsView {
    pub tick_seconds: u32,
    pub cycle_ticks: u32,
    pub rolling_window_cycles: u32,
    pub day_ticks: u32,
    pub days_per_month: u32,
    pub months_per_year: u32,
    pub start_year: u32,
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
    pub owner_faction_id: FactionId,
    pub faction_color_rgb: [u8; 3],
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
    pub owner_faction_id: FactionId,
    pub faction_color_rgb: [u8; 3],
    pub x: f64,
    pub y: f64,
    pub radius: f64,
    pub gate_nodes: Vec<RenderGateNodeView>,
    pub stations: Vec<RenderStationView>,
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

#[derive(Debug, Clone, PartialEq)]
pub struct CorporationRowView {
    pub company_id: CompanyId,
    pub name: String,
    pub archetype: CompanyArchetype,
    pub balance: f64,
    pub last_realized_profit: f64,
    pub idle_ships: usize,
    pub in_transit_ships: usize,
    pub active_orders: usize,
    pub next_plan_tick: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CorporationPanelView {
    pub rows: Vec<CorporationRowView>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MarketRowView {
    pub commodity: Commodity,
    pub price: f64,
    pub stock: f64,
    pub inflow: f64,
    pub outflow: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MarketGlobalKpisView {
    pub avg_price_index: f64,
    pub system_count: usize,
    pub station_count: usize,
    pub aggregate_stock: f64,
    pub aggregate_target_stock: f64,
    pub aggregate_stock_coverage: f64,
    pub aggregate_net_flow: f64,
    pub market_fee_rate: f64,
    pub rolling_window_total_flow: u64,
    pub player_market_share: f64,
    pub gate_congestion_active: bool,
    pub dock_congestion_active: bool,
    pub fuel_shock_active: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CommodityMarketRowView {
    pub commodity: Commodity,
    pub galaxy_avg_price: f64,
    pub min_price_station_id: Option<StationId>,
    pub min_price: f64,
    pub max_price_station_id: Option<StationId>,
    pub max_price: f64,
    pub spread_abs: f64,
    pub spread_pct: f64,
    pub cheapest_system_id: Option<SystemId>,
    pub cheapest_system_avg_price: f64,
    pub priciest_system_id: Option<SystemId>,
    pub priciest_system_avg_price: f64,
    pub total_stock: f64,
    pub total_target_stock: f64,
    pub stock_coverage: f64,
    pub inflow: f64,
    pub outflow: f64,
    pub net_flow: f64,
    pub trend_delta: f64,
    pub forecast_next_avg: f64,
    pub base_price: f64,
    pub price_vs_base: f64,
    pub stations_below_target: usize,
    pub stations_above_target: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SystemMarketStressRowView {
    pub system_id: SystemId,
    pub avg_price_index: f64,
    pub stock_coverage: f64,
    pub net_flow: f64,
    pub congestion: f64,
    pub fuel_stress: f64,
    pub stress_score: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StationCommodityHotspotView {
    pub station_id: StationId,
    pub system_id: SystemId,
    pub price: f64,
    pub stock_coverage: f64,
    pub net_flow: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SystemCommodityHotspotView {
    pub system_id: SystemId,
    pub avg_price: f64,
    pub stock_coverage: f64,
    pub net_flow: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CommodityHotspotsView {
    pub focused_commodity: Commodity,
    pub cheapest_stations: Vec<StationCommodityHotspotView>,
    pub priciest_stations: Vec<StationCommodityHotspotView>,
    pub cheapest_systems: Vec<SystemCommodityHotspotView>,
    pub priciest_systems: Vec<SystemCommodityHotspotView>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StationMarketAnomalyRowView {
    pub station_id: StationId,
    pub system_id: SystemId,
    pub price_index: f64,
    pub stock_coverage: f64,
    pub net_flow: f64,
    pub avg_price_deviation: f64,
    pub shortage_count: usize,
    pub surplus_count: usize,
    pub anomaly_score: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StationCommodityDetailView {
    pub commodity: Commodity,
    pub local_price: f64,
    pub galaxy_avg_price: f64,
    pub price_delta: f64,
    pub local_stock: f64,
    pub local_target_stock: f64,
    pub stock_coverage: f64,
    pub inflow: f64,
    pub outflow: f64,
    pub net_flow: f64,
    pub trend_delta: f64,
    pub forecast_next: f64,
    pub price_vs_base: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StationMarketDetailView {
    pub station_id: StationId,
    pub system_id: SystemId,
    pub price_index: f64,
    pub avg_price_deviation: f64,
    pub total_stock: f64,
    pub total_target_stock: f64,
    pub stock_coverage: f64,
    pub inflow: f64,
    pub outflow: f64,
    pub net_flow: f64,
    pub shortage_count: usize,
    pub surplus_count: usize,
    pub strongest_shortage_commodity: Option<Commodity>,
    pub strongest_surplus_commodity: Option<Commodity>,
    pub best_buy_commodity: Option<Commodity>,
    pub best_sell_commodity: Option<Commodity>,
    pub commodity_rows: Vec<StationCommodityDetailView>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MarketPanelView {
    pub focused_commodity: Commodity,
    pub global_kpis: MarketGlobalKpisView,
    pub commodity_rows: Vec<CommodityMarketRowView>,
    pub system_stress_rows: Vec<SystemMarketStressRowView>,
    pub commodity_hotspots: CommodityHotspotsView,
    pub station_anomaly_rows: Vec<StationMarketAnomalyRowView>,
    pub station_detail: Option<StationMarketDetailView>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TradePriceTone {
    Favorable,
    Neutral,
    Unfavorable,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StationTradeRowView {
    pub commodity: Commodity,
    pub station_stock: f64,
    pub station_target_stock: f64,
    pub player_cargo: f64,
    pub spot_price: f64,
    pub effective_buy_price: f64,
    pub effective_sell_price: f64,
    pub market_avg_price: f64,
    pub buy_tone: TradePriceTone,
    pub sell_tone: TradePriceTone,
    pub buy_cap: f64,
    pub sell_cap: f64,
    pub insufficient_capital: bool,
    pub can_buy: bool,
    pub can_sell: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StationTradeView {
    pub ship_id: ShipId,
    pub station_id: StationId,
    pub docked: bool,
    pub cargo: Option<CargoLoad>,
    pub cargo_capacity: f64,
    pub active_contract: Option<Contract>,
    pub market_fee_rate: f64,
    pub rows: Vec<StationTradeRowView>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShipPolicyView {
    pub ship_id: ShipId,
    pub policy: AutopilotPolicy,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShipCardView {
    pub ship_id: ShipId,
    pub company_id: CompanyId,
    pub owner_name: String,
    pub owner_archetype: CompanyArchetype,
    pub role: ShipRole,
    pub ship_name: String,
    pub ship_class: ShipClass,
    pub description: String,
    pub location: SystemId,
    pub current_station: Option<StationId>,
    pub current_target: Option<SystemId>,
    pub eta_ticks_remaining: u32,
    pub current_segment_kind: Option<SegmentKind>,
    pub cargo_capacity: f64,
    pub cargo: Option<CargoLoad>,
    pub active_contract: Option<Contract>,
    pub policy: AutopilotPolicy,
    pub route_len: usize,
    pub reroutes: u64,
    pub last_risk_score: f64,
    pub modules: Vec<ShipModule>,
    pub technical_state: ShipTechnicalState,
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
