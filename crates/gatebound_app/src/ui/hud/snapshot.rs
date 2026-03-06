use gatebound_domain::{
    AutopilotPolicy, CargoLoad, CompanyArchetype, Contract, ContractTypeStageA, FleetShipStatus,
    GateId, GateThroughputSnapshot, MarketInsightRow, MilestoneStatus, SegmentKind, ShipClass,
    ShipId, ShipModule, ShipRole, ShipTechnicalState, StationId, StationProfile, SystemId,
};
use gatebound_sim::{
    ActiveLoanView, ContractOfferView, CorporationRowView, LoanOfferView, MarketRowView,
    Simulation, StationTradeView, TimeSettingsView,
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
    pub selected_station_profile: Option<StationProfile>,
    pub selected_ship_id: Option<ShipId>,
    pub default_player_ship_id: Option<ShipId>,
    pub paused: bool,
    pub speed_multiplier: u32,
    pub time_label: String,
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
    pub corporation_rows: Vec<CorporationRowView>,
    pub market_rows: Vec<MarketRowView>,
    pub system_market_rows: Vec<MarketRowView>,
    pub milestones: Vec<MilestoneStatus>,
    pub throughput_rows: Vec<GateThroughputSnapshot>,
    pub market_share: f64,
    pub market_insights: Vec<MarketInsightRow>,
    pub manual_actions_per_min: f64,
    pub policy_edits_per_min: f64,
    pub avg_route_hops_player: f64,
    pub ship_card: Option<ShipCardSnapshot>,
    pub station_card: Option<StationCardSnapshot>,
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
        selected_station_id,
        matches!(camera_mode, CameraMode::System(system_id) if system_id == selected_system_id),
    );
    let finance_panel = simulation.finance_panel_view();
    let corporation_panel = simulation.corporation_panel_view();
    let resolved_ship_id = selected_ship_id.or(fleet_panel.default_player_ship_id);
    let station_card = station_card_station_id
        .or(market_panel.selected_station_id)
        .and_then(|station_id| resolved_ship_id.map(|ship_id| (ship_id, station_id)))
        .and_then(|(ship_id, station_id)| {
            build_station_card_snapshot(simulation, ship_id, station_id)
        });
    let ship_card =
        ship_card_ship_id.and_then(|ship_id| build_ship_card_snapshot(simulation, ship_id));

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
        time_label: format_time_label(overview.tick, time_settings),
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
        corporation_rows: corporation_panel.rows,
        market_rows: market_panel.station_market_rows,
        system_market_rows: market_panel.system_market_rows,
        milestones: overview.milestones,
        throughput_rows: market_panel.throughput_rows,
        market_share: market_panel.market_share,
        market_insights: market_panel.market_insights,
        manual_actions_per_min: kpi.manual_actions_per_min,
        policy_edits_per_min: kpi.policy_edits_per_min,
        avg_route_hops_player: kpi.avg_route_hops_player,
        ship_card,
        station_card,
    }
}

fn build_ship_card_snapshot(simulation: &Simulation, ship_id: ShipId) -> Option<ShipCardSnapshot> {
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

fn build_station_card_snapshot(
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
