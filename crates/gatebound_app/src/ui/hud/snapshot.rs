use gatebound_domain::{
    AutopilotPolicy, CargoLoad, Commodity, CompanyArchetype, Contract, ContractTypeStageA,
    FleetShipStatus, GateId, MilestoneStatus, SegmentKind, ShipClass, ShipId, ShipModule, ShipRole,
    ShipTechnicalState, StationId, StationProfile, SystemId,
};
use gatebound_sim::{
    ActiveLoanView, CommodityHotspotsView, CommodityMarketRowView, ContractOfferView,
    CorporationRowView, LoanOfferView, MarketGlobalKpisView, MarketPanelView, Simulation,
    StationCommodityDetailView, StationCommodityHotspotView, StationMarketAnomalyRowView,
    StationMarketDetailView, StationTradeView, SystemCommodityHotspotView, SystemDetailsView,
    SystemMarketStressRowView, SystemShipSummaryView, SystemStationSummaryView,
    SystemsPanelRowView, SystemsPanelView, TimeSettingsView,
};

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
    pub selected_ship_id: Option<ShipId>,
    pub default_player_ship_id: Option<ShipId>,
    pub paused: bool,
    pub speed_multiplier: u32,
    pub time_label: String,
    pub sla_success_rate: f64,
    pub reroutes: u64,
    pub camera_mode: String,
    pub route_gate_options: Vec<GateId>,
    pub contract_lines: Vec<String>,
    pub ship_lines: Vec<String>,
    pub active_loan: Option<ActiveLoanView>,
    pub loan_offers: Vec<LoanOfferView>,
    pub offers: Vec<ContractOfferView>,
    pub fleet_rows: Vec<FleetShipStatus>,
    pub fleet_list_rows: Vec<FleetListRowSnapshot>,
    pub systems_list_rows: Vec<SystemsListRowSnapshot>,
    pub corporation_rows: Vec<CorporationRowView>,
    pub markets: MarketsDashboardSnapshot,
    pub milestones: Vec<MilestoneStatus>,
    pub manual_actions_per_min: f64,
    pub policy_edits_per_min: f64,
    pub avg_route_hops_player: f64,
    pub system_panel: Option<SystemPanelSnapshot>,
    pub ship_card: Option<ShipCardSnapshot>,
    pub station_card: Option<StationCardSnapshot>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SystemRefSnapshot {
    pub system_id: SystemId,
    pub system_name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StationRefSnapshot {
    pub station_id: StationId,
    pub station_name: String,
    pub system_id: SystemId,
    pub system_name: String,
    pub profile: StationProfile,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MarketsGlobalKpiSnapshot {
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

#[derive(Debug, Clone, PartialEq)]
pub struct CommodityMarketRowSnapshot {
    pub commodity: Commodity,
    pub galaxy_avg_price: f64,
    pub min_price_station: Option<StationRefSnapshot>,
    pub min_price: f64,
    pub max_price_station: Option<StationRefSnapshot>,
    pub max_price: f64,
    pub spread_abs: f64,
    pub spread_pct: f64,
    pub cheapest_system: Option<SystemRefSnapshot>,
    pub cheapest_system_avg_price: f64,
    pub priciest_system: Option<SystemRefSnapshot>,
    pub priciest_system_avg_price: f64,
    pub total_stock: f64,
    pub total_target_stock: f64,
    pub stock_coverage: f64,
    pub inflow: f64,
    pub outflow: f64,
    pub net_flow: f64,
    pub trend_delta: f64,
    pub forecast_next_avg: f64,
    pub price_vs_base: f64,
    pub stations_below_target: usize,
    pub stations_above_target: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SystemMarketStressSnapshot {
    pub system_id: SystemId,
    pub system_name: String,
    pub avg_price_index: f64,
    pub stock_coverage: f64,
    pub net_flow: f64,
    pub congestion: f64,
    pub fuel_stress: f64,
    pub stress_score: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StationCommodityHotspotSnapshot {
    pub station_id: StationId,
    pub station_name: String,
    pub system_id: SystemId,
    pub system_name: String,
    pub profile: StationProfile,
    pub price: f64,
    pub stock_coverage: f64,
    pub net_flow: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SystemCommodityHotspotSnapshot {
    pub system_id: SystemId,
    pub system_name: String,
    pub avg_price: f64,
    pub stock_coverage: f64,
    pub net_flow: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MarketsHotspotsSnapshot {
    pub focused_commodity: Commodity,
    pub cheapest_stations: Vec<StationCommodityHotspotSnapshot>,
    pub priciest_stations: Vec<StationCommodityHotspotSnapshot>,
    pub cheapest_systems: Vec<SystemCommodityHotspotSnapshot>,
    pub priciest_systems: Vec<SystemCommodityHotspotSnapshot>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StationMarketAnomalySnapshot {
    pub station_id: StationId,
    pub station_name: String,
    pub system_id: SystemId,
    pub system_name: String,
    pub profile: StationProfile,
    pub price_index: f64,
    pub stock_coverage: f64,
    pub net_flow: f64,
    pub avg_price_deviation: f64,
    pub shortage_count: usize,
    pub surplus_count: usize,
    pub anomaly_score: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StationCommodityDetailSnapshot {
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
pub struct MarketsStationDetailSnapshot {
    pub station_id: StationId,
    pub station_name: String,
    pub system_id: SystemId,
    pub system_name: String,
    pub profile: StationProfile,
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
    pub commodity_rows: Vec<StationCommodityDetailSnapshot>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MarketsDashboardSnapshot {
    pub focused_commodity: Commodity,
    pub global_kpis: MarketsGlobalKpiSnapshot,
    pub commodity_rows: Vec<CommodityMarketRowSnapshot>,
    pub system_stress_rows: Vec<SystemMarketStressSnapshot>,
    pub hotspots: MarketsHotspotsSnapshot,
    pub station_anomaly_rows: Vec<StationMarketAnomalySnapshot>,
    pub station_detail: Option<MarketsStationDetailSnapshot>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShipCardSnapshot {
    pub ship_id: ShipId,
    pub company_id: gatebound_domain::CompanyId,
    pub owner_name: String,
    pub owner_archetype: CompanyArchetype,
    pub role: ShipRole,
    pub ship_name: String,
    pub ship_class: ShipClass,
    pub description: String,
    pub system_id: SystemId,
    pub system_name: String,
    pub current_station: Option<StationId>,
    pub current_station_name: Option<String>,
    pub current_target: Option<SystemId>,
    pub target_system_name: Option<String>,
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
pub struct SystemPanelSnapshot {
    pub system_id: SystemId,
    pub system_name: String,
    pub owner_faction_id: gatebound_domain::FactionId,
    pub owner_faction_name: String,
    pub owner_faction_color_rgb: [u8; 3],
    pub dock_capacity: f64,
    pub outgoing_gate_count: usize,
    pub station_count: usize,
    pub ship_count: usize,
    pub avg_price_index: f64,
    pub stock_coverage: f64,
    pub net_flow: f64,
    pub congestion: f64,
    pub fuel_stress: f64,
    pub stress_score: f64,
    pub stations: Vec<SystemStationSnapshot>,
    pub ships: Vec<SystemShipSnapshot>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SystemStationSnapshot {
    pub station_id: StationId,
    pub station_name: String,
    pub profile: StationProfile,
    pub host_body_name: String,
    pub orbit_label: String,
    pub price_index: f64,
    pub stock_coverage: f64,
    pub strongest_shortage_commodity: Option<Commodity>,
    pub strongest_surplus_commodity: Option<Commodity>,
    pub best_buy_commodity: Option<Commodity>,
    pub best_sell_commodity: Option<Commodity>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SystemShipSnapshot {
    pub ship_id: ShipId,
    pub ship_name: String,
    pub owner_name: String,
    pub owner_archetype: CompanyArchetype,
    pub role: ShipRole,
    pub ship_class: ShipClass,
    pub system_id: SystemId,
    pub current_station_name: Option<String>,
    pub target_system_name: Option<String>,
    pub eta_ticks_remaining: u32,
    pub last_risk_score: f64,
    pub reroutes: u64,
    pub status_text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FleetListRowSnapshot {
    pub ship_id: ShipId,
    pub ship_name: String,
    pub location_text: String,
    pub status_text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SystemsListRowSnapshot {
    pub system_id: SystemId,
    pub system_name: String,
    pub owner_faction_name: String,
    pub owner_faction_color_rgb: [u8; 3],
    pub station_count: usize,
    pub ship_count: usize,
    pub outgoing_gate_count: usize,
    pub stock_coverage: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StationCardSnapshot {
    pub station_id: StationId,
    pub system_id: SystemId,
    pub profile: StationProfile,
    pub station_name: String,
    pub system_name: String,
    pub host_body_name: String,
    pub orbit_label: String,
    pub profile_summary: String,
    pub imports: Vec<String>,
    pub exports: Vec<String>,
    pub station_x: f64,
    pub station_y: f64,
    pub docked: bool,
    pub trade: StationTradeView,
}

fn format_time_label(tick: u64, time: TimeSettingsView) -> String {
    let day_ticks = u64::from(time.day_ticks.max(1));
    let days_per_month = u64::from(time.days_per_month.max(1));
    let months_per_year = u64::from(time.months_per_year.max(1));
    let days_per_year = days_per_month.saturating_mul(months_per_year).max(1);

    let total_days = tick / day_ticks;
    let ticks_into_day = tick % day_ticks;
    let minutes_into_day = ticks_into_day.saturating_mul(24 * 60) / day_ticks;
    let hours = minutes_into_day / 60;
    let minutes = minutes_into_day % 60;
    let year = u64::from(time.start_year) + total_days / days_per_year;
    let month = (total_days / days_per_month) % months_per_year + 1;
    let day = total_days % days_per_month + 1;

    format!("{year:04}-{month:02}-{day:02} {hours:02}:{minutes:02}")
}

#[allow(clippy::too_many_arguments)]
pub fn build_hud_snapshot(
    simulation: &Simulation,
    paused: bool,
    speed_multiplier: u32,
    camera_mode: CameraMode,
    selected_system_id: SystemId,
    selected_station_id: Option<StationId>,
    markets_detail_station_id: Option<StationId>,
    focused_commodity: Commodity,
    station_card_station_id: Option<StationId>,
    ship_card_ship_id: Option<ShipId>,
    selected_ship_id: Option<ShipId>,
    filters: ContractsFilterState,
    kpi: &UiKpiTracker,
) -> HudSnapshot {
    let cycle_report = derive_cycle_report(simulation);
    let overview = simulation.hud_overview_view();
    let time_settings = simulation.time_settings_view();
    let contracts_board = simulation.contracts_board_view();
    let fleet_panel = simulation.fleet_panel_view();
    let render_snapshot = simulation.world_render_snapshot();
    let market_panel = simulation.market_panel_view(
        selected_system_id,
        markets_detail_station_id,
        focused_commodity,
    );
    let finance_panel = simulation.finance_panel_view();
    let corporation_panel = simulation.corporation_panel_view();
    let systems_panel = simulation.systems_panel_view();
    let resolved_ship_id = selected_ship_id.or(fleet_panel.default_player_ship_id);
    let fleet_list_rows = build_fleet_list_rows(simulation, &fleet_panel.player_ship_ids);
    let systems_list_rows = build_systems_list_rows(&systems_panel);
    let station_card = station_card_station_id
        .and_then(|station_id| resolved_ship_id.map(|ship_id| (ship_id, station_id)))
        .and_then(|(ship_id, station_id)| {
            build_station_card_snapshot_for_ui(simulation, ship_id, station_id)
        });
    let system_panel = match camera_mode {
        CameraMode::Galaxy => None,
        CameraMode::System(system_id) => simulation
            .system_details_view(system_id)
            .and_then(|view| build_system_panel_snapshot(simulation, view)),
    };
    let ship_card =
        ship_card_ship_id.and_then(|ship_id| build_ship_card_snapshot_for_ui(simulation, ship_id));

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
        selected_station_id,
        selected_ship_id,
        default_player_ship_id: fleet_panel.default_player_ship_id,
        paused,
        speed_multiplier,
        time_label: format_time_label(overview.tick, time_settings),
        sla_success_rate: cycle_report.sla_success_rate,
        reroutes: overview.reroutes,
        camera_mode: match camera_mode {
            CameraMode::Galaxy => "Galaxy".to_string(),
            CameraMode::System(system_id) => format!("System({})", system_id.0),
        },
        route_gate_options: contracts_board.route_gates,
        contract_lines,
        ship_lines,
        active_loan: finance_panel.active_loan,
        loan_offers: finance_panel.loan_offers,
        offers,
        fleet_rows: fleet_panel.rows,
        fleet_list_rows,
        systems_list_rows,
        corporation_rows: corporation_panel.rows,
        markets: build_markets_snapshot(simulation, &market_panel),
        milestones: overview.milestones,
        manual_actions_per_min: kpi.manual_actions_per_min,
        policy_edits_per_min: kpi.policy_edits_per_min,
        avg_route_hops_player: kpi.avg_route_hops_player,
        system_panel,
        ship_card,
        station_card,
    }
}

fn build_markets_snapshot(
    simulation: &Simulation,
    panel: &MarketPanelView,
) -> MarketsDashboardSnapshot {
    MarketsDashboardSnapshot {
        focused_commodity: panel.focused_commodity,
        global_kpis: build_market_global_kpis_snapshot(panel.global_kpis),
        commodity_rows: panel
            .commodity_rows
            .iter()
            .map(|row| build_commodity_market_row_snapshot(simulation, *row))
            .collect(),
        system_stress_rows: panel
            .system_stress_rows
            .iter()
            .map(|row| build_system_stress_snapshot(*row))
            .collect(),
        hotspots: build_hotspots_snapshot(simulation, &panel.commodity_hotspots),
        station_anomaly_rows: panel
            .station_anomaly_rows
            .iter()
            .map(|row| build_station_anomaly_snapshot(simulation, *row))
            .collect(),
        station_detail: panel
            .station_detail
            .as_ref()
            .and_then(|detail| build_station_detail_snapshot(simulation, detail)),
    }
}

fn build_market_global_kpis_snapshot(view: MarketGlobalKpisView) -> MarketsGlobalKpiSnapshot {
    MarketsGlobalKpiSnapshot {
        avg_price_index: view.avg_price_index,
        system_count: view.system_count,
        station_count: view.station_count,
        aggregate_stock: view.aggregate_stock,
        aggregate_target_stock: view.aggregate_target_stock,
        aggregate_stock_coverage: view.aggregate_stock_coverage,
        aggregate_net_flow: view.aggregate_net_flow,
        market_fee_rate: view.market_fee_rate,
        rolling_window_total_flow: view.rolling_window_total_flow,
        player_market_share: view.player_market_share,
        gate_congestion_active: view.gate_congestion_active,
        dock_congestion_active: view.dock_congestion_active,
        fuel_shock_active: view.fuel_shock_active,
    }
}

fn build_commodity_market_row_snapshot(
    simulation: &Simulation,
    row: CommodityMarketRowView,
) -> CommodityMarketRowSnapshot {
    CommodityMarketRowSnapshot {
        commodity: row.commodity,
        galaxy_avg_price: row.galaxy_avg_price,
        min_price_station: row
            .min_price_station_id
            .and_then(|station_id| station_ref_snapshot(simulation, station_id)),
        min_price: row.min_price,
        max_price_station: row
            .max_price_station_id
            .and_then(|station_id| station_ref_snapshot(simulation, station_id)),
        max_price: row.max_price,
        spread_abs: row.spread_abs,
        spread_pct: row.spread_pct,
        cheapest_system: row.cheapest_system_id.map(system_ref_snapshot),
        cheapest_system_avg_price: row.cheapest_system_avg_price,
        priciest_system: row.priciest_system_id.map(system_ref_snapshot),
        priciest_system_avg_price: row.priciest_system_avg_price,
        total_stock: row.total_stock,
        total_target_stock: row.total_target_stock,
        stock_coverage: row.stock_coverage,
        inflow: row.inflow,
        outflow: row.outflow,
        net_flow: row.net_flow,
        trend_delta: row.trend_delta,
        forecast_next_avg: row.forecast_next_avg,
        price_vs_base: row.price_vs_base,
        stations_below_target: row.stations_below_target,
        stations_above_target: row.stations_above_target,
    }
}

fn build_system_stress_snapshot(row: SystemMarketStressRowView) -> SystemMarketStressSnapshot {
    SystemMarketStressSnapshot {
        system_id: row.system_id,
        system_name: generated_system_name(row.system_id),
        avg_price_index: row.avg_price_index,
        stock_coverage: row.stock_coverage,
        net_flow: row.net_flow,
        congestion: row.congestion,
        fuel_stress: row.fuel_stress,
        stress_score: row.stress_score,
    }
}

fn build_hotspots_snapshot(
    simulation: &Simulation,
    hotspots: &CommodityHotspotsView,
) -> MarketsHotspotsSnapshot {
    MarketsHotspotsSnapshot {
        focused_commodity: hotspots.focused_commodity,
        cheapest_stations: hotspots
            .cheapest_stations
            .iter()
            .filter_map(|row| build_station_hotspot_snapshot(simulation, *row))
            .collect(),
        priciest_stations: hotspots
            .priciest_stations
            .iter()
            .filter_map(|row| build_station_hotspot_snapshot(simulation, *row))
            .collect(),
        cheapest_systems: hotspots
            .cheapest_systems
            .iter()
            .map(|row| build_system_hotspot_snapshot(*row))
            .collect(),
        priciest_systems: hotspots
            .priciest_systems
            .iter()
            .map(|row| build_system_hotspot_snapshot(*row))
            .collect(),
    }
}

fn build_station_hotspot_snapshot(
    simulation: &Simulation,
    row: StationCommodityHotspotView,
) -> Option<StationCommodityHotspotSnapshot> {
    let reference = station_ref_snapshot(simulation, row.station_id)?;
    Some(StationCommodityHotspotSnapshot {
        station_id: row.station_id,
        station_name: reference.station_name,
        system_id: row.system_id,
        system_name: reference.system_name,
        profile: reference.profile,
        price: row.price,
        stock_coverage: row.stock_coverage,
        net_flow: row.net_flow,
    })
}

fn build_system_hotspot_snapshot(
    row: SystemCommodityHotspotView,
) -> SystemCommodityHotspotSnapshot {
    SystemCommodityHotspotSnapshot {
        system_id: row.system_id,
        system_name: generated_system_name(row.system_id),
        avg_price: row.avg_price,
        stock_coverage: row.stock_coverage,
        net_flow: row.net_flow,
    }
}

fn build_station_anomaly_snapshot(
    simulation: &Simulation,
    row: StationMarketAnomalyRowView,
) -> StationMarketAnomalySnapshot {
    let reference = station_ref_snapshot(simulation, row.station_id)
        .expect("station anomaly rows should reference real stations");
    StationMarketAnomalySnapshot {
        station_id: row.station_id,
        station_name: reference.station_name,
        system_id: row.system_id,
        system_name: reference.system_name,
        profile: reference.profile,
        price_index: row.price_index,
        stock_coverage: row.stock_coverage,
        net_flow: row.net_flow,
        avg_price_deviation: row.avg_price_deviation,
        shortage_count: row.shortage_count,
        surplus_count: row.surplus_count,
        anomaly_score: row.anomaly_score,
    }
}

fn build_station_detail_snapshot(
    simulation: &Simulation,
    detail: &StationMarketDetailView,
) -> Option<MarketsStationDetailSnapshot> {
    let reference = station_ref_snapshot(simulation, detail.station_id)?;
    Some(MarketsStationDetailSnapshot {
        station_id: detail.station_id,
        station_name: reference.station_name,
        system_id: detail.system_id,
        system_name: reference.system_name,
        profile: reference.profile,
        price_index: detail.price_index,
        avg_price_deviation: detail.avg_price_deviation,
        total_stock: detail.total_stock,
        total_target_stock: detail.total_target_stock,
        stock_coverage: detail.stock_coverage,
        inflow: detail.inflow,
        outflow: detail.outflow,
        net_flow: detail.net_flow,
        shortage_count: detail.shortage_count,
        surplus_count: detail.surplus_count,
        strongest_shortage_commodity: detail.strongest_shortage_commodity,
        strongest_surplus_commodity: detail.strongest_surplus_commodity,
        best_buy_commodity: detail.best_buy_commodity,
        best_sell_commodity: detail.best_sell_commodity,
        commodity_rows: detail
            .commodity_rows
            .iter()
            .map(|row| build_station_commodity_detail_snapshot(*row))
            .collect(),
    })
}

fn build_station_commodity_detail_snapshot(
    row: StationCommodityDetailView,
) -> StationCommodityDetailSnapshot {
    StationCommodityDetailSnapshot {
        commodity: row.commodity,
        local_price: row.local_price,
        galaxy_avg_price: row.galaxy_avg_price,
        price_delta: row.price_delta,
        local_stock: row.local_stock,
        local_target_stock: row.local_target_stock,
        stock_coverage: row.stock_coverage,
        inflow: row.inflow,
        outflow: row.outflow,
        net_flow: row.net_flow,
        trend_delta: row.trend_delta,
        forecast_next: row.forecast_next,
        price_vs_base: row.price_vs_base,
    }
}

fn build_system_panel_snapshot(
    simulation: &Simulation,
    view: SystemDetailsView,
) -> Option<SystemPanelSnapshot> {
    let topology = simulation.camera_topology_view();
    let system = topology
        .systems
        .iter()
        .find(|system| system.system_id == view.system_id)?;

    Some(SystemPanelSnapshot {
        system_id: view.system_id,
        system_name: generated_system_name(view.system_id),
        owner_faction_id: view.owner_faction_id,
        owner_faction_name: view.owner_faction_name,
        owner_faction_color_rgb: view.faction_color_rgb,
        dock_capacity: view.dock_capacity,
        outgoing_gate_count: view.outgoing_gate_count,
        station_count: view.stations.len(),
        ship_count: view.ships.len(),
        avg_price_index: view.avg_price_index,
        stock_coverage: view.stock_coverage,
        net_flow: view.net_flow,
        congestion: view.congestion,
        fuel_stress: view.fuel_stress,
        stress_score: view.stress_score,
        stations: view
            .stations
            .iter()
            .map(|station| build_system_station_snapshot(system, station))
            .collect(),
        ships: view
            .ships
            .iter()
            .map(|ship| build_system_ship_snapshot(simulation, ship))
            .collect(),
    })
}

fn build_system_station_snapshot(
    system: &gatebound_sim::CameraSystemView,
    station: &SystemStationSummaryView,
) -> SystemStationSnapshot {
    let orbit_ratio = orbit_ratio(system.x, system.y, station.x, station.y, system.radius);
    SystemStationSnapshot {
        station_id: station.station_id,
        station_name: generated_station_name(station.station_id, station.profile),
        profile: station.profile,
        host_body_name: generated_host_body_name(system.system_id, station.station_id, orbit_ratio),
        orbit_label: orbit_label(orbit_ratio).to_string(),
        price_index: station.price_index,
        stock_coverage: station.stock_coverage,
        strongest_shortage_commodity: station.strongest_shortage_commodity,
        strongest_surplus_commodity: station.strongest_surplus_commodity,
        best_buy_commodity: station.best_buy_commodity,
        best_sell_commodity: station.best_sell_commodity,
    }
}

fn build_system_ship_snapshot(
    simulation: &Simulation,
    ship: &SystemShipSummaryView,
) -> SystemShipSnapshot {
    let topology = simulation.camera_topology_view();
    let current_station_name = ship.current_station.and_then(|station_id| {
        topology
            .systems
            .iter()
            .flat_map(|system| system.stations.iter())
            .find(|station| station.station_id == station_id)
            .map(|station| generated_station_name(station.station_id, station.profile))
    });
    let target_system_name = ship.current_target.map(generated_system_name);
    let status_text = if let Some(station_name) = current_station_name.as_ref() {
        format!("Docked at {station_name}")
    } else if let Some(target_name) = target_system_name.as_ref() {
        format!(
            "In transit to {target_name} • ETA {}",
            ship.eta_ticks_remaining
        )
    } else if let Some(kind) = ship.current_segment_kind {
        format!("{kind:?} • ETA {}", ship.eta_ticks_remaining)
    } else {
        format!("Idle in {}", generated_system_name(ship.location))
    };

    SystemShipSnapshot {
        ship_id: ship.ship_id,
        ship_name: ship.ship_name.clone(),
        owner_name: ship.owner_name.clone(),
        owner_archetype: ship.owner_archetype,
        role: ship.role,
        ship_class: ship.ship_class,
        system_id: ship.location,
        current_station_name,
        target_system_name,
        eta_ticks_remaining: ship.eta_ticks_remaining,
        last_risk_score: ship.last_risk_score,
        reroutes: ship.reroutes,
        status_text,
    }
}

fn system_ref_snapshot(system_id: SystemId) -> SystemRefSnapshot {
    SystemRefSnapshot {
        system_id,
        system_name: generated_system_name(system_id),
    }
}

fn station_ref_snapshot(
    simulation: &Simulation,
    station_id: StationId,
) -> Option<StationRefSnapshot> {
    simulation
        .camera_topology_view()
        .systems
        .into_iter()
        .find_map(|system| {
            system
                .stations
                .into_iter()
                .find(|station| station.station_id == station_id)
                .map(|station| StationRefSnapshot {
                    station_id,
                    station_name: generated_station_name(station.station_id, station.profile),
                    system_id: system.system_id,
                    system_name: generated_system_name(system.system_id),
                    profile: station.profile,
                })
        })
}

fn build_fleet_list_rows(
    simulation: &Simulation,
    player_ship_ids: &[ShipId],
) -> Vec<FleetListRowSnapshot> {
    let topology = simulation.camera_topology_view();
    let mut rows = player_ship_ids
        .iter()
        .filter_map(|ship_id| {
            let view = simulation.ship_card_view(*ship_id)?;
            let system_name = generated_system_name(view.location);
            let current_station_name = view.current_station.and_then(|station_id| {
                topology
                    .systems
                    .iter()
                    .flat_map(|system| system.stations.iter())
                    .find(|station| station.station_id == station_id)
                    .map(|station| generated_station_name(station.station_id, station.profile))
            });

            let location_text = current_station_name
                .as_ref()
                .map(|station_name| format!("{station_name}, {system_name}"))
                .unwrap_or_else(|| system_name.clone());
            let status_text = if let Some(station_name) = current_station_name.as_ref() {
                format!("Docked at {station_name}")
            } else if let Some(target_system) = view.current_target {
                format!(
                    "In transit to {} • ETA {}",
                    generated_system_name(target_system),
                    view.eta_ticks_remaining
                )
            } else if view.eta_ticks_remaining > 0 || view.current_segment_kind.is_some() {
                format!("In transit • ETA {}", view.eta_ticks_remaining)
            } else {
                format!("Idle in {system_name}")
            };

            Some(FleetListRowSnapshot {
                ship_id: view.ship_id,
                ship_name: view.ship_name,
                location_text,
                status_text,
            })
        })
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        left.ship_name
            .cmp(&right.ship_name)
            .then_with(|| left.ship_id.0.cmp(&right.ship_id.0))
    });
    rows
}

fn build_systems_list_rows(panel: &SystemsPanelView) -> Vec<SystemsListRowSnapshot> {
    panel
        .rows
        .iter()
        .map(build_systems_list_row_snapshot)
        .collect()
}

fn build_systems_list_row_snapshot(row: &SystemsPanelRowView) -> SystemsListRowSnapshot {
    SystemsListRowSnapshot {
        system_id: row.system_id,
        system_name: row.system_name.clone(),
        owner_faction_name: row.owner_faction_name.clone(),
        owner_faction_color_rgb: row.owner_faction_color_rgb,
        station_count: row.station_count,
        ship_count: row.ship_count,
        outgoing_gate_count: row.outgoing_gate_count,
        stock_coverage: row.stock_coverage,
    }
}

pub(crate) fn build_ship_card_snapshot_for_ui(
    simulation: &Simulation,
    ship_id: ShipId,
) -> Option<ShipCardSnapshot> {
    let view = simulation.ship_card_view(ship_id)?;
    let topology = simulation.camera_topology_view();
    let current_station_name = view.current_station.and_then(|station_id| {
        topology
            .systems
            .iter()
            .flat_map(|system| system.stations.iter())
            .find(|station| station.station_id == station_id)
            .map(|station| generated_station_name(station.station_id, station.profile))
    });

    Some(ShipCardSnapshot {
        ship_id: view.ship_id,
        company_id: view.company_id,
        owner_name: view.owner_name,
        owner_archetype: view.owner_archetype,
        role: view.role,
        ship_name: view.ship_name,
        ship_class: view.ship_class,
        description: view.description,
        system_id: view.location,
        system_name: generated_system_name(view.location),
        current_station: view.current_station,
        current_station_name,
        current_target: view.current_target,
        target_system_name: view.current_target.map(generated_system_name),
        eta_ticks_remaining: view.eta_ticks_remaining,
        current_segment_kind: view.current_segment_kind,
        cargo_capacity: view.cargo_capacity,
        cargo: view.cargo,
        active_contract: view.active_contract,
        policy: view.policy,
        route_len: view.route_len,
        reroutes: view.reroutes,
        last_risk_score: view.last_risk_score,
        modules: view.modules,
        technical_state: view.technical_state,
    })
}

pub(crate) fn build_station_card_snapshot_for_ui(
    simulation: &Simulation,
    ship_id: ShipId,
    station_id: StationId,
) -> Option<StationCardSnapshot> {
    let trade = simulation.station_trade_view(ship_id, station_id)?;
    let topology = simulation.camera_topology_view();
    let (system, station) = topology.systems.iter().find_map(|system| {
        system
            .stations
            .iter()
            .find(|station| station.station_id == station_id)
            .map(|station| (system, station))
    })?;

    let system_name = generated_system_name(system.system_id);
    let station_name = generated_station_name(station.station_id, station.profile);
    let orbit_ratio = orbit_ratio(system.x, system.y, station.x, station.y, system.radius);
    let orbit_label = orbit_label(orbit_ratio).to_string();
    let host_body_name =
        generated_host_body_name(system.system_id, station.station_id, orbit_ratio);
    let profile_summary = profile_summary(station.profile).to_string();
    let (imports, exports) = trade_flow_notes(&trade);

    Some(StationCardSnapshot {
        station_id,
        system_id: system.system_id,
        profile: station.profile,
        station_name,
        system_name,
        host_body_name,
        orbit_label,
        profile_summary,
        imports,
        exports,
        station_x: station.x,
        station_y: station.y,
        docked: trade.docked,
        trade,
    })
}

fn generated_system_name(system_id: SystemId) -> String {
    const PREFIXES: [&str; 8] = [
        "Aster", "Cinder", "Helios", "Kepler", "Lyra", "Nimbus", "Orion", "Vega",
    ];
    const SUFFIXES: [&str; 8] = [
        "Reach", "Gate", "Haven", "Drift", "Span", "Crown", "Verge", "Anchor",
    ];

    let prefix = PREFIXES[system_id.0 % PREFIXES.len()];
    let suffix = SUFFIXES[(system_id.0 / PREFIXES.len()) % SUFFIXES.len()];
    format!("{prefix} {suffix}")
}

fn generated_station_name(station_id: StationId, profile: StationProfile) -> String {
    let role = match profile {
        StationProfile::Civilian => "Concourse",
        StationProfile::Industrial => "Foundry",
        StationProfile::Research => "Array",
    };
    format!("{role}-{:03}", station_id.0)
}

fn generated_host_body_name(
    system_id: SystemId,
    station_id: StationId,
    orbit_ratio: f64,
) -> String {
    const INNER: [&str; 4] = ["Cinder", "Basalt", "Icarus", "Morrow"];
    const MID: [&str; 4] = ["Pelago", "Nysa", "Tethys", "Ariel"];
    const OUTER: [&str; 4] = ["Vesper", "Isolde", "Khepri", "Halo"];

    let names = if orbit_ratio < 0.4 {
        &INNER
    } else if orbit_ratio < 0.65 {
        &MID
    } else {
        &OUTER
    };
    let idx = (system_id.0 + station_id.0) % names.len();
    format!("{} {}", names[idx], (system_id.0 % 7) + 1)
}

fn orbit_ratio(system_x: f64, system_y: f64, station_x: f64, station_y: f64, radius: f64) -> f64 {
    let dx = station_x - system_x;
    let dy = station_y - system_y;
    let distance = (dx * dx + dy * dy).sqrt();
    if radius <= 0.0 {
        0.0
    } else {
        distance / radius
    }
}

fn orbit_label(orbit_ratio: f64) -> &'static str {
    if orbit_ratio < 0.4 {
        "inner logistics orbit"
    } else if orbit_ratio < 0.65 {
        "mid transfer orbit"
    } else {
        "outer anchor orbit"
    }
}

fn profile_summary(profile: StationProfile) -> &'static str {
    match profile {
        StationProfile::Civilian => {
            "Habitat-and-market hub focused on life support, retail demand, and stable regional distribution."
        }
        StationProfile::Industrial => {
            "Heavy fabrication yard turning raw mass into bulk metals, fuel, and serviceable ship parts."
        }
        StationProfile::Research => {
            "Precision laboratory complex optimized for advanced parts, electronics, and volatile specialist demand."
        }
    }
}

fn trade_flow_notes(trade: &StationTradeView) -> (Vec<String>, Vec<String>) {
    let mut imports = trade
        .rows
        .iter()
        .filter_map(|row| {
            let target = row.station_target_stock.max(1.0);
            let pressure = ((target - row.station_stock) / target).max(0.0);
            (pressure > 0.0).then_some((row.commodity, pressure))
        })
        .collect::<Vec<_>>();
    imports.sort_by(|a, b| b.1.total_cmp(&a.1));

    let mut exports = trade
        .rows
        .iter()
        .filter_map(|row| {
            let target = row.station_target_stock.max(1.0);
            let pressure = ((row.station_stock - target) / target).max(0.0);
            (pressure > 0.0).then_some((row.commodity, pressure))
        })
        .collect::<Vec<_>>();
    exports.sort_by(|a, b| b.1.total_cmp(&a.1));

    let imports = summarize_trade_pressures(imports, "short");
    let exports = summarize_trade_pressures(exports, "surplus");

    (imports, exports)
}

fn summarize_trade_pressures(
    entries: Vec<(gatebound_domain::Commodity, f64)>,
    label: &str,
) -> Vec<String> {
    let mut notes = entries
        .into_iter()
        .take(3)
        .map(|(commodity, pressure)| {
            format!(
                "{} ({} {:.0}%)",
                commodity_label(commodity),
                label,
                pressure * 100.0
            )
        })
        .collect::<Vec<_>>();

    if notes.is_empty() {
        notes.push("Balanced inventory profile".to_string());
    }

    notes
}
