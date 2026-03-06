use crate::input::camera::{
    apply_escape, apply_station_context_open, apply_system_click, CameraMode, ClickTracker,
};
use crate::render::world::{
    segment_from_point, ship_is_visible_in_current_view, system_objects_visible_in_current_view,
    update_ship_motion_cache, ShipMotionCache,
};
use crate::runtime::sim::{
    apply_offer_filters, apply_panel_toggle, consume_ticks, hotkey_to_risk, panel_hotkey_to_index,
    ContractsFilterState, FinanceUiState, OfferSortMode, RiskHotkey, SimResource, StationUiState,
    UiKpiTracker,
};
use crate::ui::hud::build_hud_snapshot as build_hud_snapshot_v2;
use bevy::prelude::*;
use gatebound_domain::{
    ActiveLoan, CargoLoad, CargoSource, Commodity, ContractOffer, ContractTypeStageA, GateId,
    LoanOfferId, OfferProblemTag, PriorityMode, RouteSegment, RuntimeConfig, SegmentKind, ShipId,
    ShipRole, StationId, SystemId,
};
use gatebound_sim::{
    test_support::{
        FinanceStateFixture, MarketStatePatch, ShipCycleMetricsFixture, ShipPatch,
        SimulationScenarioBuilder,
    },
    Simulation,
};

#[allow(clippy::too_many_arguments)]
fn build_hud_snapshot(
    simulation: &Simulation,
    paused: bool,
    speed_multiplier: u32,
    camera_mode: CameraMode,
    selected_system_id: SystemId,
    selected_ship_id: Option<ShipId>,
    filters: ContractsFilterState,
    kpi: &UiKpiTracker,
) -> crate::ui::hud::HudSnapshot {
    let selected_station = simulation
        .camera_topology_view()
        .systems
        .iter()
        .find(|system| system.system_id == selected_system_id)
        .and_then(|system| system.stations.first().map(|station| station.station_id));
    build_hud_snapshot_v2(
        simulation,
        paused,
        speed_multiplier,
        camera_mode,
        selected_system_id,
        selected_station,
        selected_ship_id,
        filters,
        kpi,
    )
}

fn ship_docked_at(simulation: &Simulation, ship_id: ShipId, station_id: StationId) -> bool {
    simulation
        .station_ops_view(ship_id, station_id)
        .is_some_and(|view| view.docked)
}

#[test]
fn fixed_step_consumes_expected_ticks_for_speed_modes() {
    let mut clock_1x = crate::runtime::sim::SimClock::default();
    assert_eq!(consume_ticks(&mut clock_1x, 3.1, 1), 3);

    let mut clock_2x = crate::runtime::sim::SimClock {
        speed_multiplier: 2,
        ..crate::runtime::sim::SimClock::default()
    };
    assert_eq!(consume_ticks(&mut clock_2x, 1.6, 1), 3);

    let mut clock_4x = crate::runtime::sim::SimClock {
        speed_multiplier: 4,
        ..crate::runtime::sim::SimClock::default()
    };
    assert_eq!(consume_ticks(&mut clock_4x, 1.26, 1), 5);
}

#[test]
fn double_click_enters_system_and_escape_returns_to_galaxy() {
    let mut mode = CameraMode::Galaxy;
    let mut tracker = ClickTracker::default();

    assert!(!apply_system_click(
        &mut mode,
        &mut tracker,
        SystemId(2),
        0.0
    ));
    assert!(apply_system_click(
        &mut mode,
        &mut tracker,
        SystemId(2),
        0.2
    ));
    assert_eq!(mode, CameraMode::System(SystemId(2)));

    apply_escape(&mut mode, true);
    assert_eq!(mode, CameraMode::Galaxy);
}

#[test]
fn ship_motion_cache_progress_is_clamped() {
    assert_eq!(ShipMotionCache::progress_ratio(10, 10), 0.0);
    assert_eq!(ShipMotionCache::progress_ratio(10, 0), 1.0);
    assert_eq!(ShipMotionCache::progress_ratio(10, 20), 0.0);
}

#[test]
fn galaxy_view_hides_all_ships() {
    assert!(!ship_is_visible_in_current_view(
        CameraMode::Galaxy,
        SystemId(0)
    ));
    assert!(!ship_is_visible_in_current_view(
        CameraMode::Galaxy,
        SystemId(999)
    ));
}

#[test]
fn system_view_shows_only_ships_from_selected_system() {
    assert!(ship_is_visible_in_current_view(
        CameraMode::System(SystemId(2)),
        SystemId(2)
    ));
    assert!(!ship_is_visible_in_current_view(
        CameraMode::System(SystemId(2)),
        SystemId(1)
    ));
}

#[test]
fn view_switch_preserves_filter_behavior() {
    let ship_system = SystemId(3);
    let sequence = [
        CameraMode::Galaxy,
        CameraMode::System(SystemId(3)),
        CameraMode::Galaxy,
    ];
    let visible: Vec<bool> = sequence
        .iter()
        .map(|mode| ship_is_visible_in_current_view(*mode, ship_system))
        .collect();
    assert_eq!(visible, vec![false, true, false]);
}

#[test]
fn galaxy_view_hides_system_objects() {
    assert!(!system_objects_visible_in_current_view(
        CameraMode::Galaxy,
        SystemId(0)
    ));
    assert!(!system_objects_visible_in_current_view(
        CameraMode::Galaxy,
        SystemId(5)
    ));
}

#[test]
fn system_view_shows_objects_only_for_selected_system() {
    assert!(system_objects_visible_in_current_view(
        CameraMode::System(SystemId(4)),
        SystemId(4)
    ));
    assert!(!system_objects_visible_in_current_view(
        CameraMode::System(SystemId(4)),
        SystemId(3)
    ));
}

#[test]
fn view_switch_preserves_system_objects_filter_behavior() {
    let system = SystemId(2);
    let sequence = [
        CameraMode::Galaxy,
        CameraMode::System(system),
        CameraMode::Galaxy,
    ];
    let visible: Vec<bool> = sequence
        .iter()
        .map(|mode| system_objects_visible_in_current_view(*mode, system))
        .collect();
    assert_eq!(visible, vec![false, true, false]);
}

#[test]
fn hotkey_mapping_matches_stage_a_risk_events() {
    assert!(matches!(
        hotkey_to_risk('g'),
        Some(RiskHotkey::GateCongestion)
    ));
    assert!(matches!(
        hotkey_to_risk('d'),
        Some(RiskHotkey::DockCongestion)
    ));
    assert!(matches!(hotkey_to_risk('f'), Some(RiskHotkey::FuelShock)));
    assert!(hotkey_to_risk('x').is_none());
}

#[test]
fn finance_ui_state_defaults_are_sensible() {
    let state = FinanceUiState::default();
    assert!(state.pending_offer.is_none());
    assert!(state.repayment_amount > 0.0);
}

#[test]
fn panel_hotkeys_toggle_expected_windows() {
    let mut panels = crate::runtime::sim::UiPanelState::default();
    assert_eq!(panel_hotkey_to_index('1'), Some(1));
    apply_panel_toggle(&mut panels, 1);
    assert!(!panels.contracts);
    apply_panel_toggle(&mut panels, 2);
    assert!(!panels.fleet);
    apply_panel_toggle(&mut panels, 3);
    assert!(!panels.markets);
    apply_panel_toggle(&mut panels, 4);
    assert!(!panels.assets);
    apply_panel_toggle(&mut panels, 5);
    assert!(!panels.policies);
    assert_eq!(panel_hotkey_to_index('6'), Some(6));
    apply_panel_toggle(&mut panels, 6);
    assert!(!panels.station_ops);
}

#[test]
fn contracts_filter_applies_route_gate_problem_and_premium() {
    let offers = vec![
        ContractOffer {
            id: 0,
            kind: ContractTypeStageA::Delivery,
            commodity: Commodity::Fuel,
            origin: SystemId(0),
            destination: SystemId(1),
            origin_station: StationId(0),
            destination_station: StationId(2),
            quantity: 10.0,
            payout: 30.0,
            penalty: 10.0,
            eta_ticks: 20,
            risk_score: 0.8,
            margin_estimate: 12.0,
            route_gate_ids: vec![GateId(7)],
            problem_tag: OfferProblemTag::CongestedRoute,
            premium: true,
            profit_per_ton: 1.2,
            expires_cycle: 10,
        },
        ContractOffer {
            id: 1,
            kind: ContractTypeStageA::Delivery,
            commodity: Commodity::Fuel,
            origin: SystemId(0),
            destination: SystemId(2),
            origin_station: StationId(0),
            destination_station: StationId(4),
            quantity: 10.0,
            payout: 30.0,
            penalty: 10.0,
            eta_ticks: 30,
            risk_score: 0.4,
            margin_estimate: 40.0,
            route_gate_ids: vec![GateId(8)],
            problem_tag: OfferProblemTag::LowMargin,
            premium: false,
            profit_per_ton: 4.0,
            expires_cycle: 10,
        },
    ];
    let filters = ContractsFilterState {
        min_margin: 10.0,
        max_risk: 1.0,
        max_eta: 120,
        commodity: None,
        route_gate: Some(GateId(7)),
        problem: Some(OfferProblemTag::CongestedRoute),
        premium_only: true,
        sort_mode: OfferSortMode::MarginDesc,
    };
    let filtered = apply_offer_filters(offers, filters);
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].id, 0);
}

#[test]
fn contracts_filter_applies_commodity() {
    let offers = vec![
        ContractOffer {
            id: 10,
            kind: ContractTypeStageA::Delivery,
            commodity: Commodity::Fuel,
            origin: SystemId(0),
            destination: SystemId(1),
            origin_station: StationId(0),
            destination_station: StationId(2),
            quantity: 8.0,
            payout: 20.0,
            penalty: 8.0,
            eta_ticks: 12,
            risk_score: 0.2,
            margin_estimate: 8.0,
            route_gate_ids: vec![],
            problem_tag: OfferProblemTag::LowMargin,
            premium: false,
            profit_per_ton: 1.0,
            expires_cycle: 4,
        },
        ContractOffer {
            id: 11,
            kind: ContractTypeStageA::Delivery,
            commodity: Commodity::Electronics,
            origin: SystemId(0),
            destination: SystemId(1),
            origin_station: StationId(0),
            destination_station: StationId(2),
            quantity: 8.0,
            payout: 20.0,
            penalty: 8.0,
            eta_ticks: 12,
            risk_score: 0.2,
            margin_estimate: 8.0,
            route_gate_ids: vec![],
            problem_tag: OfferProblemTag::LowMargin,
            premium: false,
            profit_per_ton: 1.0,
            expires_cycle: 4,
        },
    ];
    let filters = ContractsFilterState {
        min_margin: f64::NEG_INFINITY,
        max_risk: f64::INFINITY,
        max_eta: u32::MAX,
        commodity: Some(Commodity::Electronics),
        route_gate: None,
        problem: None,
        premium_only: false,
        sort_mode: OfferSortMode::MarginDesc,
    };
    let filtered = apply_offer_filters(offers, filters);
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].id, 11);
    assert_eq!(filtered[0].commodity, Commodity::Electronics);
}

#[test]
fn contracts_board_snapshot_includes_intel_and_problem_labels() {
    let mut builder = SimulationScenarioBuilder::stage_a(42);
    builder.with_contract_offer(ContractOffer {
        id: 77,
        kind: ContractTypeStageA::Delivery,
        commodity: Commodity::Fuel,
        origin: SystemId(0),
        destination: SystemId(1),
        origin_station: StationId(0),
        destination_station: StationId(2),
        quantity: 14.0,
        payout: 50.0,
        penalty: 15.0,
        eta_ticks: 40,
        risk_score: 1.1,
        margin_estimate: 11.0,
        route_gate_ids: vec![GateId(0)],
        problem_tag: OfferProblemTag::HighRisk,
        premium: true,
        profit_per_ton: 0.78,
        expires_cycle: 8,
    });
    let sim = builder.build();

    let snapshot = build_hud_snapshot(
        &sim,
        true,
        1,
        CameraMode::Galaxy,
        SystemId(0),
        Some(ShipId(0)),
        ContractsFilterState::default(),
        &UiKpiTracker::default(),
    );

    let offer = snapshot
        .offers
        .iter()
        .find(|offer| offer.offer.id == 77)
        .expect("offer should be visible");
    assert_eq!(offer.offer.problem_tag, OfferProblemTag::HighRisk);
    assert!(!offer.offer.route_gate_ids.is_empty());
    assert!(offer.destination_intel.is_some());
}

#[test]
fn contracts_snapshot_renders_station_endpoints() {
    let sim = Simulation::new(RuntimeConfig::default(), 42);
    let snapshot = build_hud_snapshot(
        &sim,
        true,
        1,
        CameraMode::Galaxy,
        SystemId(0),
        None,
        ContractsFilterState::default(),
        &UiKpiTracker::default(),
    );
    assert!(
        snapshot
            .contract_lines
            .iter()
            .any(|line| line.contains(":A")),
        "contract lines should expose station endpoints"
    );
}

#[test]
fn fleet_snapshot_contains_job_queue_idle_delay_profit() {
    let ship_id = ShipId(0);
    let mut builder = SimulationScenarioBuilder::stage_a(42);
    builder.with_ship_cycle_metrics(
        ship_id,
        ShipCycleMetricsFixture {
            idle_ticks_cycle: 9,
            delay_ticks_cycle: 12,
            runs_completed: 3,
            profit_earned: 90.0,
        },
    );
    let sim = builder.build();

    let snapshot = build_hud_snapshot(
        &sim,
        true,
        1,
        CameraMode::Galaxy,
        SystemId(0),
        Some(ship_id),
        ContractsFilterState::default(),
        &UiKpiTracker::default(),
    );
    let row = snapshot
        .fleet_rows
        .iter()
        .find(|row| row.ship_id == ship_id)
        .expect("ship row should exist");
    assert_eq!(row.idle_ticks_cycle, 9);
    assert!(row.avg_delay_ticks_cycle > 0.0);
    assert!(row.profit_per_run > 0.0);
    assert!(!row.job_queue.is_empty());
}

#[test]
fn fleet_snapshot_renders_current_segment_kind() {
    let mut builder = SimulationScenarioBuilder::stage_a(42);
    builder.with_ship_patch(
        ShipId(0),
        ShipPatch {
            current_segment_kind: Some(Some(SegmentKind::GateQueue)),
            segment_eta_remaining: Some(7),
            eta_ticks_remaining: Some(7),
            movement_queue: Some(vec![RouteSegment {
                from: SystemId(0),
                to: SystemId(1),
                from_anchor: None,
                to_anchor: None,
                edge: Some(GateId(0)),
                kind: SegmentKind::GateQueue,
                eta_ticks: 7,
                risk: 0.0,
            }]),
            ..ShipPatch::default()
        },
    );
    let sim = builder.build();
    let snapshot = build_hud_snapshot(
        &sim,
        true,
        1,
        CameraMode::Galaxy,
        SystemId(0),
        Some(ShipId(0)),
        ContractsFilterState::default(),
        &UiKpiTracker::default(),
    );
    assert!(
        snapshot
            .ship_lines
            .iter()
            .any(|line| line.contains("seg=GateQueue")),
        "fleet snapshot should surface current segment kind"
    );
}

#[test]
fn fleet_snapshot_exposes_role_and_cargo_metadata() {
    let mut builder = SimulationScenarioBuilder::stage_a(42);
    let npc_id = builder.first_npc_ship_id().expect("npc ship should exist");
    builder.with_ship_patch(
        npc_id,
        ShipPatch {
            cargo: Some(Some(CargoLoad {
                commodity: Commodity::Parts,
                amount: 5.5,
                source: CargoSource::Spot,
            })),
            ..ShipPatch::default()
        },
    );
    let sim = builder.build();
    let snapshot = build_hud_snapshot(
        &sim,
        true,
        1,
        CameraMode::System(SystemId(0)),
        SystemId(0),
        Some(npc_id),
        ContractsFilterState::default(),
        &UiKpiTracker::default(),
    );
    let row = snapshot
        .fleet_rows
        .iter()
        .find(|row| row.ship_id == npc_id)
        .expect("npc row should exist");
    assert_eq!(row.role, ShipRole::NpcTrade);
    assert_eq!(row.cargo_commodity, Some(Commodity::Parts));
    assert!((row.cargo_amount - 5.5).abs() < 1e-9);
}

#[test]
fn render_world_uses_station_anchor_positions_for_ship_motion() {
    let mut builder = SimulationScenarioBuilder::stage_a(42);
    let ship_id = ShipId(0);
    let stations = builder.stations_in_system(SystemId(0));
    let from_station = stations[0];
    let to_station = stations[1];
    let from = builder
        .station_coords(from_station)
        .expect("from station coords should exist");
    let to = builder
        .station_coords(to_station)
        .expect("to station coords should exist");
    builder.with_ship_patch(
        ship_id,
        ShipPatch {
            location: Some(SystemId(0)),
            movement_queue: Some(vec![RouteSegment {
                from: SystemId(0),
                to: SystemId(0),
                from_anchor: Some(from_station),
                to_anchor: Some(to_station),
                edge: None,
                kind: SegmentKind::InSystem,
                eta_ticks: 12,
                risk: 0.0,
            }]),
            current_segment_kind: Some(Some(SegmentKind::InSystem)),
            segment_progress_total: Some(12),
            segment_eta_remaining: Some(12),
            eta_ticks_remaining: Some(12),
            ..ShipPatch::default()
        },
    );
    let sim = builder.build();

    let mut app = App::new();
    app.insert_resource(SimResource::new(sim))
        .insert_resource(ShipMotionCache::default())
        .add_systems(Update, update_ship_motion_cache);
    app.update();

    let cache = app.world().resource::<ShipMotionCache>();
    let state = cache
        .segments
        .get(&ship_id)
        .copied()
        .expect("ship motion state should exist");
    assert_eq!(state.from, Vec2::new(from.0 as f32, from.1 as f32));
    assert_eq!(state.to, Vec2::new(to.0 as f32, to.1 as f32));
}

#[test]
fn ship_motion_after_warp_starts_at_entry_gate_not_center() {
    let mut builder = SimulationScenarioBuilder::stage_a(52);
    let Some(edge) = builder.first_edge() else {
        return;
    };
    let ship_id = ShipId(0);
    let target_system = edge.to_system;
    let destination_station = builder
        .first_station_in_system(target_system)
        .expect("destination station should exist");
    let (entry_x, entry_y) = builder
        .gate_position(target_system, edge.gate_id)
        .expect("entry gate coords should exist");
    let center = builder
        .system_position(target_system)
        .map(|(x, y)| Vec2::new(x as f32, y as f32))
        .expect("target system should exist");
    builder.with_ship_patch(
        ship_id,
        ShipPatch {
            location: Some(target_system),
            last_gate_arrival: Some(Some(edge.gate_id)),
            movement_queue: Some(vec![RouteSegment {
                from: target_system,
                to: target_system,
                from_anchor: None,
                to_anchor: Some(destination_station),
                edge: None,
                kind: SegmentKind::InSystem,
                eta_ticks: 10,
                risk: 0.0,
            }]),
            current_segment_kind: Some(Some(SegmentKind::InSystem)),
            segment_progress_total: Some(10),
            segment_eta_remaining: Some(10),
            eta_ticks_remaining: Some(10),
            ..ShipPatch::default()
        },
    );
    let sim = builder.build();

    let mut app = App::new();
    app.insert_resource(SimResource::new(sim))
        .insert_resource(ShipMotionCache::default())
        .add_systems(Update, update_ship_motion_cache);
    app.update();

    let cache = app.world().resource::<ShipMotionCache>();
    let state = cache
        .segments
        .get(&ship_id)
        .copied()
        .expect("ship motion state should exist");
    assert_eq!(state.from, Vec2::new(entry_x as f32, entry_y as f32));
    assert_ne!(state.from, center);
}

#[test]
fn in_system_motion_entry_gate_to_station_interpolates_correctly() {
    let mut builder = SimulationScenarioBuilder::stage_a(54);
    let Some(edge) = builder.first_edge() else {
        return;
    };
    let ship_id = ShipId(0);
    let target_system = edge.to_system;
    let destination_station = builder
        .first_station_in_system(target_system)
        .expect("destination station should exist");
    let destination = builder
        .station_coords(destination_station)
        .expect("destination station coords should exist");
    builder.with_ship_patch(
        ship_id,
        ShipPatch {
            location: Some(target_system),
            last_gate_arrival: Some(Some(edge.gate_id)),
            movement_queue: Some(vec![RouteSegment {
                from: target_system,
                to: target_system,
                from_anchor: None,
                to_anchor: Some(destination_station),
                edge: None,
                kind: SegmentKind::InSystem,
                eta_ticks: 8,
                risk: 0.0,
            }]),
            current_segment_kind: Some(Some(SegmentKind::InSystem)),
            segment_progress_total: Some(8),
            segment_eta_remaining: Some(4),
            eta_ticks_remaining: Some(4),
            ..ShipPatch::default()
        },
    );
    let sim = builder.build();
    let snapshot = sim.world_render_snapshot();
    let ship = snapshot
        .ships
        .iter()
        .find(|ship| ship.ship_id == ship_id)
        .expect("ship should exist");
    let segment = ship
        .front_segment
        .as_ref()
        .expect("movement segment should exist");
    let from = segment_from_point(&snapshot, ship, segment);
    let to = Vec2::new(destination.0 as f32, destination.1 as f32);
    let t =
        ShipMotionCache::progress_ratio(ship.segment_progress_total, ship.segment_eta_remaining);
    let interpolated = from.lerp(to, t);
    let expected = from.lerp(to, 0.5);
    assert!((interpolated.x - expected.x).abs() < 1e-4);
    assert!((interpolated.y - expected.y).abs() < 1e-4);
}

#[test]
fn markets_snapshot_contains_trend_forecast_and_factors() {
    let sim = Simulation::new(RuntimeConfig::default(), 42);
    let snapshot = build_hud_snapshot(
        &sim,
        true,
        1,
        CameraMode::System(SystemId(0)),
        SystemId(0),
        None,
        ContractsFilterState::default(),
        &UiKpiTracker::default(),
    );
    assert!(!snapshot.market_insights.is_empty());
    assert!(snapshot.market_insights[0].forecast_next.is_finite());
}

#[test]
fn markets_panel_uses_selected_system_or_fallback() {
    let sim = Simulation::new(RuntimeConfig::default(), 42);
    let galaxy_snapshot = build_hud_snapshot(
        &sim,
        true,
        1,
        CameraMode::Galaxy,
        SystemId(0),
        None,
        ContractsFilterState::default(),
        &UiKpiTracker::default(),
    );
    assert_eq!(galaxy_snapshot.selected_system_id, SystemId(0));
    assert!(!galaxy_snapshot.market_rows.is_empty());

    let system_snapshot = build_hud_snapshot(
        &sim,
        true,
        1,
        CameraMode::System(SystemId(1)),
        SystemId(1),
        None,
        ContractsFilterState::default(),
        &UiKpiTracker::default(),
    );
    assert_eq!(system_snapshot.selected_system_id, SystemId(1));
    assert!(!system_snapshot.market_rows.is_empty());
}

#[test]
fn markets_panel_uses_selected_station_market() {
    let mut builder = SimulationScenarioBuilder::stage_a(42);
    let system_id = SystemId(0);
    let stations = builder.stations_in_system(system_id);
    if stations.len() < 2 {
        return;
    }
    let selected_station = stations[1];
    builder.with_market_state_patch(
        stations[0],
        Commodity::Fuel,
        MarketStatePatch {
            stock: Some(9.0),
            ..MarketStatePatch::default()
        },
    );
    builder.with_market_state_patch(
        selected_station,
        Commodity::Fuel,
        MarketStatePatch {
            stock: Some(77.0),
            ..MarketStatePatch::default()
        },
    );
    let sim = builder.build();

    let snapshot = build_hud_snapshot_v2(
        &sim,
        true,
        1,
        CameraMode::System(system_id),
        system_id,
        Some(selected_station),
        None,
        ContractsFilterState::default(),
        &UiKpiTracker::default(),
    );
    let fuel_row = snapshot
        .market_rows
        .iter()
        .find(|row| row.commodity == Commodity::Fuel)
        .expect("fuel row should exist");
    assert_eq!(snapshot.selected_station_id, Some(selected_station));
    assert!((fuel_row.stock - 77.0).abs() < 1e-9);
}

#[test]
fn policy_edit_updates_ship_policy() {
    let mut sim = Simulation::new(RuntimeConfig::default(), 42);
    let ship_id = ShipId(0);
    let mut policy = sim
        .ship_policy_view(ship_id)
        .expect("ship policy view should exist")
        .policy;
    policy.min_margin = 3.5;
    policy.max_risk_score = 0.9;
    policy.max_hops = 3;
    policy.priority_mode = PriorityMode::Stability;
    sim.update_ship_policy(ship_id, policy)
        .expect("policy update should succeed");
    let policy = sim
        .ship_policy_view(ship_id)
        .expect("ship policy view should exist")
        .policy;
    assert!((policy.min_margin - 3.5).abs() < 1e-9);
    assert!((policy.max_risk_score - 0.9).abs() < 1e-9);
    assert_eq!(policy.max_hops, 3);
    assert_eq!(policy.priority_mode, PriorityMode::Stability);
}

#[test]
fn manual_vs_policy_kpi_updates_from_user_actions() {
    let sim = Simulation::new(RuntimeConfig::default(), 12);
    let mut kpi = UiKpiTracker::default();
    kpi.record_manual_action(sim.tick());
    kpi.record_policy_edit(sim.tick());
    kpi.update(&sim);
    assert!(kpi.manual_actions_per_min >= 1.0);
    assert!(kpi.policy_edits_per_min >= 1.0);
    assert!(kpi.avg_route_hops_player >= 0.0);
}

#[test]
fn hud_snapshot_includes_debt_reputation_and_active_loan() {
    let mut builder = SimulationScenarioBuilder::stage_a(7);
    builder.with_finance_state(FinanceStateFixture {
        active_loan: Some(ActiveLoan {
            offer_id: LoanOfferId::Growth,
            principal_remaining: 123.0,
            monthly_interest_rate: 0.07,
            remaining_months: 5,
            next_payment: 31.5,
        }),
        reputation: 0.55,
    });
    let sim = builder.build();

    let snapshot = build_hud_snapshot(
        &sim,
        false,
        2,
        CameraMode::Galaxy,
        SystemId(0),
        None,
        ContractsFilterState::default(),
        &UiKpiTracker::default(),
    );
    assert!((snapshot.debt - 123.0).abs() < 1e-9);
    assert!((snapshot.reputation - 0.55).abs() < 1e-9);
    assert!((snapshot.interest_rate - 0.07).abs() < 1e-9);
    assert_eq!(
        snapshot.active_loan.expect("active loan").remaining_months,
        5
    );
}

#[test]
fn finance_snapshot_exposes_fixed_credit_offers_without_active_loan() {
    let sim = Simulation::new(RuntimeConfig::default(), 42);

    let snapshot = build_hud_snapshot(
        &sim,
        true,
        1,
        CameraMode::Galaxy,
        SystemId(0),
        None,
        ContractsFilterState::default(),
        &UiKpiTracker::default(),
    );
    assert_eq!(snapshot.loan_offers.len(), 3);
    assert!(snapshot.active_loan.is_none());
    assert!(snapshot
        .loan_offers
        .iter()
        .any(|offer| offer.id == LoanOfferId::Starter));
    assert!(snapshot
        .loan_offers
        .iter()
        .any(|offer| offer.id == LoanOfferId::Growth));
    assert!(snapshot
        .loan_offers
        .iter()
        .any(|offer| offer.id == LoanOfferId::Expansion));
}

#[test]
fn finance_snapshot_preserves_selected_system_context() {
    let cfg = RuntimeConfig::default();
    let sim = Simulation::new(cfg, 42);

    let snapshot = build_hud_snapshot(
        &sim,
        true,
        1,
        CameraMode::System(SystemId(0)),
        SystemId(0),
        None,
        ContractsFilterState::default(),
        &UiKpiTracker::default(),
    );
    assert_eq!(snapshot.selected_system_id, SystemId(0));
    assert_eq!(snapshot.loan_offers.len(), 3);
}

#[test]
fn right_click_station_opens_context_menu_state() {
    let mut ui = StationUiState::default();
    assert!(!ui.context_menu_open);
    apply_station_context_open(&mut ui, StationId(7));
    assert!(ui.context_menu_open);
    assert_eq!(ui.context_station_id, Some(StationId(7)));
}

#[test]
fn station_context_fly_command_sets_ship_route() {
    let mut sim = Simulation::new(RuntimeConfig::default(), 88);
    let ship_id = ShipId(0);
    let target_station = sim
        .camera_topology_view()
        .systems
        .iter()
        .find(|system| system.system_id == SystemId(1))
        .and_then(|system| system.stations.first().map(|station| station.station_id))
        .or_else(|| {
            sim.camera_topology_view()
                .systems
                .iter()
                .find(|system| system.system_id == SystemId(0))
                .and_then(|system| system.stations.first().map(|station| station.station_id))
        })
        .expect("target station should exist");

    sim.command_fly_to_station(ship_id, target_station)
        .expect("fly command should succeed");
    let snapshot = sim.world_render_snapshot();
    let ship = snapshot
        .ships
        .iter()
        .find(|ship| ship.ship_id == ship_id)
        .expect("ship should exist");
    assert!(
        ship.current_target.is_some() || ship.front_segment.is_some(),
        "fly command should produce active movement state"
    );
}

#[test]
fn auto_dock_becomes_true_on_station_arrival() {
    let mut sim = Simulation::new(RuntimeConfig::default(), 91);
    let ship_id = ShipId(0);
    let target_station = sim
        .camera_topology_view()
        .systems
        .iter()
        .find(|system| system.system_id == SystemId(1))
        .and_then(|system| system.stations.first().map(|station| station.station_id))
        .or_else(|| {
            sim.camera_topology_view()
                .systems
                .iter()
                .find(|system| system.system_id == SystemId(0))
                .and_then(|system| system.stations.first().map(|station| station.station_id))
        })
        .expect("target station should exist");

    sim.command_fly_to_station(ship_id, target_station)
        .expect("fly command should succeed");
    assert!(
        !ship_docked_at(&sim, ship_id, target_station),
        "ship should not be docked immediately after command"
    );

    for _ in 0..400 {
        sim.step_tick();
        if ship_docked_at(&sim, ship_id, target_station) {
            break;
        }
    }
    assert!(
        ship_docked_at(&sim, ship_id, target_station),
        "ship should eventually auto-dock at destination station"
    );
}

#[test]
fn station_trade_actions_change_market_and_cargo_state() {
    let mut cfg = RuntimeConfig::default();
    cfg.pressure.market_fee_rate = 0.1;
    let mut builder = SimulationScenarioBuilder::new(cfg, 99);
    let ship_id = ShipId(0);
    let station_id = builder
        .first_station_in_system(SystemId(0))
        .expect("station should exist");
    builder.dock_ship_at(ship_id, station_id);
    let mut sim = builder.build();

    let stock_before = sim
        .station_ops_view(ship_id, station_id)
        .and_then(|view| {
            view.market_rows
                .into_iter()
                .find(|row| row.commodity == Commodity::Fuel)
                .map(|row| row.stock)
        })
        .unwrap_or(0.0);
    sim.player_buy(ship_id, station_id, Commodity::Fuel, 6.0)
        .expect("buy should pass");
    sim.player_sell(ship_id, station_id, Commodity::Fuel, 2.0)
        .expect("sell should pass");

    let ops_view = sim
        .station_ops_view(ship_id, station_id)
        .expect("ship station ops view should exist");
    assert!(
        ops_view.cargo.is_some(),
        "ship should retain partial cargo after sell"
    );
    assert_eq!(
        ops_view.cargo.map(|cargo| cargo.source),
        Some(CargoSource::Spot),
        "spot trading should keep spot cargo source"
    );
    let stock_after = ops_view
        .market_rows
        .iter()
        .find(|row| row.commodity == Commodity::Fuel)
        .map(|row| row.stock)
        .unwrap_or(0.0);
    assert!(
        stock_after < stock_before,
        "net buy should reduce station stock"
    );
}
