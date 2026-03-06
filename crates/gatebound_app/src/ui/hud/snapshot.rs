use gatebound_domain::{
    ContractTypeStageA, FleetShipStatus, GateId, GateThroughputSnapshot, MarketInsightRow,
    MilestoneStatus, ShipId, StationId, StationProfile, SystemId,
};
use gatebound_sim::{ActiveLoanView, ContractOfferView, LoanOfferView, MarketRowView, Simulation};

use crate::input::camera::CameraMode;
use crate::runtime::sim::{
    apply_offer_filters, derive_cycle_report, ContractsFilterState, UiKpiTracker,
};

use super::labels::commodity_label;

#[derive(Debug, Clone, PartialEq)]
pub struct HudSnapshot {
    pub tick: u64,
    pub cycle: u64,
    pub capital: f64,
    pub debt: f64,
    pub interest_rate: f64,
    pub reputation: f64,
    pub active_contracts: usize,
    pub active_ships: usize,
    pub selected_system_id: SystemId,
    pub selected_station_id: Option<StationId>,
    pub selected_station_profile: Option<StationProfile>,
    pub selected_ship_id: Option<ShipId>,
    pub default_player_ship_id: Option<ShipId>,
    pub paused: bool,
    pub speed_multiplier: u32,
    pub sla_success_rate: f64,
    pub reroutes: u64,
    pub avg_price_index: f64,
    pub camera_mode: String,
    pub intel_staleness_ticks: u64,
    pub intel_confidence: f64,
    pub route_gate_options: Vec<GateId>,
    pub contract_lines: Vec<String>,
    pub ship_lines: Vec<String>,
    pub active_loan: Option<ActiveLoanView>,
    pub loan_offers: Vec<LoanOfferView>,
    pub offers: Vec<ContractOfferView>,
    pub fleet_rows: Vec<FleetShipStatus>,
    pub market_rows: Vec<MarketRowView>,
    pub system_market_rows: Vec<MarketRowView>,
    pub milestones: Vec<MilestoneStatus>,
    pub throughput_rows: Vec<GateThroughputSnapshot>,
    pub market_share: f64,
    pub market_insights: Vec<MarketInsightRow>,
    pub manual_actions_per_min: f64,
    pub policy_edits_per_min: f64,
    pub avg_route_hops_player: f64,
}

#[allow(clippy::too_many_arguments)]
pub fn build_hud_snapshot(
    simulation: &Simulation,
    paused: bool,
    speed_multiplier: u32,
    camera_mode: CameraMode,
    selected_system_id: SystemId,
    selected_station_id: Option<StationId>,
    selected_ship_id: Option<ShipId>,
    filters: ContractsFilterState,
    kpi: &UiKpiTracker,
) -> HudSnapshot {
    let cycle_report = derive_cycle_report(simulation);
    let overview = simulation.hud_overview_view();
    let contracts_board = simulation.contracts_board_view();
    let fleet_panel = simulation.fleet_panel_view();
    let render_snapshot = simulation.world_render_snapshot();
    let market_panel = simulation.market_panel_view(
        selected_system_id,
        selected_station_id,
        matches!(camera_mode, CameraMode::System(system_id) if system_id == selected_system_id),
    );
    let finance_panel = simulation.finance_panel_view();

    let contract_lines = contracts_board
        .active_contracts
        .iter()
        .take(8)
        .map(|contract| {
            let kind = match contract.kind {
                ContractTypeStageA::Delivery => "Delivery",
                ContractTypeStageA::Supply => "Supply",
            };
            format!(
                "#{} {kind} {} S{}:A{} -> S{}:A{} qty={:.1} deadline={} miss={}",
                contract.id.0,
                commodity_label(contract.commodity),
                contract.origin.0,
                contract.origin_station.0,
                contract.destination.0,
                contract.destination_station.0,
                contract.quantity,
                contract.deadline_tick,
                contract.missed_cycles,
            )
        })
        .collect::<Vec<_>>();

    let mut render_ships = render_snapshot.ships;
    render_ships.sort_by_key(|ship| ship.ship_id.0);
    let ship_lines = render_ships
        .iter()
        .take(10)
        .map(|ship| {
            let target = ship
                .current_target
                .map(|system_id| system_id.0.to_string())
                .unwrap_or_else(|| "-".to_string());
            let segment = ship
                .current_segment_kind
                .map(|kind| format!("{kind:?}"))
                .unwrap_or_else(|| "-".to_string());
            format!(
                "#{} c={} sys={} -> {} eta={} seg={} seg_eta={} risk={:.2} reroutes={}",
                ship.ship_id.0,
                ship.company_id.0,
                ship.location.0,
                target,
                ship.eta_ticks_remaining,
                segment,
                ship.segment_eta_remaining,
                ship.last_risk_score,
                ship.reroutes,
            )
        })
        .collect::<Vec<_>>();

    let offers = apply_offer_filters(
        contracts_board
            .offers
            .iter()
            .map(|entry| entry.offer.clone())
            .collect::<Vec<_>>(),
        filters,
    )
    .into_iter()
    .filter_map(|offer| {
        contracts_board
            .offers
            .iter()
            .find(|entry| entry.offer.id == offer.id)
            .cloned()
    })
    .collect::<Vec<_>>();

    HudSnapshot {
        tick: overview.tick,
        cycle: overview.cycle,
        capital: overview.capital,
        debt: overview.debt,
        interest_rate: overview.interest_rate,
        reputation: overview.reputation,
        active_contracts: overview.active_contracts,
        active_ships: overview.active_ships,
        selected_system_id,
        selected_station_id: market_panel.selected_station_id,
        selected_station_profile: market_panel.selected_station_profile,
        selected_ship_id,
        default_player_ship_id: fleet_panel.default_player_ship_id,
        paused,
        speed_multiplier,
        sla_success_rate: cycle_report.sla_success_rate,
        reroutes: overview.reroutes,
        avg_price_index: market_panel.avg_price_index,
        camera_mode: match camera_mode {
            CameraMode::Galaxy => "Galaxy".to_string(),
            CameraMode::System(system_id) => format!("System({})", system_id.0),
        },
        intel_staleness_ticks: market_panel.intel.map_or(0, |info| info.staleness_ticks),
        intel_confidence: market_panel.intel.map_or(1.0, |info| info.confidence),
        route_gate_options: contracts_board.route_gates,
        contract_lines,
        ship_lines,
        active_loan: finance_panel.active_loan,
        loan_offers: finance_panel.loan_offers,
        offers,
        fleet_rows: fleet_panel.rows,
        market_rows: market_panel.station_market_rows,
        system_market_rows: market_panel.system_market_rows,
        milestones: overview.milestones,
        throughput_rows: market_panel.throughput_rows,
        market_share: market_panel.market_share,
        market_insights: market_panel.market_insights,
        manual_actions_per_min: kpi.manual_actions_per_min,
        policy_edits_per_min: kpi.policy_edits_per_min,
        avg_route_hops_player: kpi.avg_route_hops_player,
    }
}
