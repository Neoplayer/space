use bevy::prelude::*;
use gatebound_core::{
    CargoSource, Commodity, ContractOffer, ContractTypeStageA, GateId, OfferProblemTag,
    PriorityMode, RecoveryAction, RouteSegment, RuntimeConfig, SegmentKind, ShipId, Simulation,
    SlotType, SystemId,
};
use std::collections::VecDeque;

use crate::hud::build_hud_snapshot as build_hud_snapshot_v2;
use crate::render_world::{
    segment_from_point, ship_is_visible_in_current_view, system_objects_visible_in_current_view,
    update_ship_motion_cache, ShipMotionCache,
};
use crate::sim_runtime::{
    apply_offer_filters, apply_panel_toggle, consume_ticks, hotkey_to_lease, hotkey_to_risk,
    panel_hotkey_to_index, ContractsFilterState, LeaseHotkey, OfferSortMode, RiskHotkey,
    SimResource, StationUiState, UiKpiTracker,
};
use crate::view_mode::{
    apply_escape, apply_station_context_open, apply_system_click, CameraMode, ClickTracker,
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
) -> crate::hud::HudSnapshot {
    let selected_station = simulation
        .world
        .stations_by_system
        .get(&selected_system_id)
        .and_then(|stations| stations.first().copied());
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

#[test]
fn fixed_step_consumes_expected_ticks_for_speed_modes() {
    let mut clock_1x = crate::sim_runtime::SimClock::default();
    assert_eq!(consume_ticks(&mut clock_1x, 3.1, 1), 3);

    let mut clock_2x = crate::sim_runtime::SimClock {
        speed_multiplier: 2,
        ..crate::sim_runtime::SimClock::default()
    };
    assert_eq!(consume_ticks(&mut clock_2x, 1.6, 1), 3);

    let mut clock_4x = crate::sim_runtime::SimClock {
        speed_multiplier: 4,
        ..crate::sim_runtime::SimClock::default()
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
fn lease_hotkey_mapping_matches_expected_actions() {
    assert!(matches!(hotkey_to_lease('z'), Some(LeaseHotkey::Dock)));
    assert!(matches!(hotkey_to_lease('x'), Some(LeaseHotkey::Storage)));
    assert!(matches!(hotkey_to_lease('c'), Some(LeaseHotkey::Factory)));
    assert!(matches!(hotkey_to_lease('v'), Some(LeaseHotkey::Market)));
    assert!(matches!(hotkey_to_lease('r'), Some(LeaseHotkey::Release)));
    assert!(hotkey_to_lease('q').is_none());
}

#[test]
fn panel_hotkeys_toggle_expected_windows() {
    let mut panels = crate::sim_runtime::UiPanelState::default();
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
            origin_station: gatebound_core::StationId(0),
            destination_station: gatebound_core::StationId(2),
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
            origin_station: gatebound_core::StationId(0),
            destination_station: gatebound_core::StationId(4),
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
            origin_station: gatebound_core::StationId(0),
            destination_station: gatebound_core::StationId(2),
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
            origin_station: gatebound_core::StationId(0),
            destination_station: gatebound_core::StationId(2),
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
    let mut sim = Simulation::new(RuntimeConfig::default(), 42);
    sim.contract_offers.insert(
        77,
        ContractOffer {
            id: 77,
            kind: ContractTypeStageA::Delivery,
            commodity: Commodity::Fuel,
            origin: SystemId(0),
            destination: SystemId(1),
            origin_station: gatebound_core::StationId(0),
            destination_station: gatebound_core::StationId(2),
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
        },
    );

    let snapshot = build_hud_snapshot(
        &sim,
        true,
        1,
        CameraMode::Galaxy,
        SystemId(0),
        Some(gatebound_core::ShipId(0)),
        ContractsFilterState::default(),
        &UiKpiTracker::default(),
    );

    let offer = snapshot
        .offers
        .iter()
        .find(|offer| offer.id == 77)
        .expect("offer should be visible");
    assert_eq!(offer.problem_tag, OfferProblemTag::HighRisk);
    assert!(!offer.route_gate_ids.is_empty());
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
    let mut sim = Simulation::new(RuntimeConfig::default(), 42);
    let ship_id = gatebound_core::ShipId(0);
    sim.ship_idle_ticks_cycle.insert(ship_id, 9);
    sim.ship_delay_ticks_cycle.insert(ship_id, 12);
    sim.ship_runs_completed.insert(ship_id, 3);
    sim.ship_profit_earned.insert(ship_id, 90.0);

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
    let mut sim = Simulation::new(RuntimeConfig::default(), 42);
    if let Some(ship) = sim.ships.get_mut(&gatebound_core::ShipId(0)) {
        ship.current_segment_kind = Some(SegmentKind::GateQueue);
        ship.segment_eta_remaining = 7;
        ship.eta_ticks_remaining = 7;
    }
    let snapshot = build_hud_snapshot(
        &sim,
        true,
        1,
        CameraMode::Galaxy,
        SystemId(0),
        Some(gatebound_core::ShipId(0)),
        ContractsFilterState::default(),
        &UiKpiTracker::default(),
    );
    assert!(
        snapshot
            .ship_lines
            .iter()
            .any(|line| line.contains("seg=GateQueue")),
        "fleet lines should render current segment kind"
    );
}

#[test]
fn fleet_snapshot_exposes_role_and_cargo_metadata() {
    let mut sim = Simulation::new(RuntimeConfig::default(), 42);
    let npc_id = sim
        .ships
        .iter()
        .find(|(_, ship)| ship.role == gatebound_core::ShipRole::NpcTrade)
        .map(|(ship_id, _)| *ship_id)
        .expect("npc ship should exist");
    if let Some(ship) = sim.ships.get_mut(&npc_id) {
        ship.cargo = Some(gatebound_core::CargoLoad {
            commodity: Commodity::Parts,
            amount: 5.5,
            source: gatebound_core::CargoSource::Spot,
        });
    }
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
    assert_eq!(row.role, gatebound_core::ShipRole::NpcTrade);
    assert_eq!(row.cargo_commodity, Some(Commodity::Parts));
    assert!((row.cargo_amount - 5.5).abs() < 1e-9);
}

#[test]
fn render_world_uses_station_anchor_positions_for_ship_motion() {
    let mut sim = Simulation::new(RuntimeConfig::default(), 42);
    let ship_id = gatebound_core::ShipId(0);
    let stations = sim
        .world
        .stations_by_system
        .get(&SystemId(0))
        .cloned()
        .expect("system stations should exist");
    let from_station = stations[0];
    let to_station = stations[1];
    let from = sim
        .station_position(from_station)
        .expect("from station coords should exist");
    let to = sim
        .station_position(to_station)
        .expect("to station coords should exist");
    if let Some(ship) = sim.ships.get_mut(&ship_id) {
        ship.location = SystemId(0);
        ship.movement_queue = VecDeque::from([RouteSegment {
            from: SystemId(0),
            to: SystemId(0),
            from_anchor: Some(from_station),
            to_anchor: Some(to_station),
            edge: None,
            kind: SegmentKind::InSystem,
            eta_ticks: 12,
            risk: 0.0,
        }]);
        ship.current_segment_kind = Some(SegmentKind::InSystem);
        ship.segment_progress_total = 12;
        ship.segment_eta_remaining = 12;
        ship.eta_ticks_remaining = 12;
    }

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
    let mut sim = Simulation::new(RuntimeConfig::default(), 52);
    let Some(edge) = sim.world.edges.first().cloned() else {
        return;
    };
    let ship_id = gatebound_core::ShipId(0);
    let target_system = edge.b;
    let destination_station = sim
        .world
        .first_station(target_system)
        .expect("destination station should exist");
    let (entry_x, entry_y) = sim
        .world
        .gate_coords(target_system, edge.id)
        .expect("entry gate coords should exist");
    let center = sim
        .world
        .systems
        .iter()
        .find(|system| system.id == target_system)
        .map(|system| Vec2::new(system.x as f32, system.y as f32))
        .expect("target system should exist");
    if let Some(ship) = sim.ships.get_mut(&ship_id) {
        ship.location = target_system;
        ship.last_gate_arrival = Some(edge.id);
        ship.movement_queue = VecDeque::from([RouteSegment {
            from: target_system,
            to: target_system,
            from_anchor: None,
            to_anchor: Some(destination_station),
            edge: None,
            kind: SegmentKind::InSystem,
            eta_ticks: 10,
            risk: 0.0,
        }]);
        ship.current_segment_kind = Some(SegmentKind::InSystem);
        ship.segment_progress_total = 10;
        ship.segment_eta_remaining = 10;
        ship.eta_ticks_remaining = 10;
    }

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
    let mut sim = Simulation::new(RuntimeConfig::default(), 54);
    let Some(edge) = sim.world.edges.first().cloned() else {
        return;
    };
    let ship_id = gatebound_core::ShipId(0);
    let target_system = edge.b;
    let destination_station = sim
        .world
        .first_station(target_system)
        .expect("destination station should exist");
    let destination = sim
        .station_position(destination_station)
        .expect("destination station coords should exist");
    if let Some(ship) = sim.ships.get_mut(&ship_id) {
        ship.location = target_system;
        ship.last_gate_arrival = Some(edge.id);
        ship.movement_queue = VecDeque::from([RouteSegment {
            from: target_system,
            to: target_system,
            from_anchor: None,
            to_anchor: Some(destination_station),
            edge: None,
            kind: SegmentKind::InSystem,
            eta_ticks: 8,
            risk: 0.0,
        }]);
        ship.current_segment_kind = Some(SegmentKind::InSystem);
        ship.segment_progress_total = 8;
        ship.segment_eta_remaining = 4;
        ship.eta_ticks_remaining = 4;
    }

    let ship = sim.ships.get(&ship_id).expect("ship should exist");
    let segment = ship
        .movement_queue
        .front()
        .expect("movement segment should exist");
    let from = segment_from_point(&sim, ship, segment);
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
    let mut sim = Simulation::new(RuntimeConfig::default(), 42);
    let system_id = SystemId(0);
    let stations = sim
        .world
        .stations_by_system
        .get(&system_id)
        .cloned()
        .expect("system stations should exist");
    if stations.len() < 2 {
        return;
    }
    let selected_station = stations[1];
    if let Some(first_market) = sim.markets.get_mut(&stations[0]) {
        if let Some(fuel) = first_market.goods.get_mut(&Commodity::Fuel) {
            fuel.stock = 9.0;
        }
    }
    if let Some(selected_market) = sim.markets.get_mut(&selected_station) {
        if let Some(fuel) = selected_market.goods.get_mut(&Commodity::Fuel) {
            fuel.stock = 77.0;
        }
    }

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
    let cfg = RuntimeConfig::default();
    let mut sim = Simulation::new(cfg, 42);
    let ship_id = gatebound_core::ShipId(0);
    if let Some(ship) = sim.ships.get_mut(&ship_id) {
        ship.policy.min_margin = 3.5;
        ship.policy.max_risk_score = 0.9;
        ship.policy.max_hops = 3;
        ship.policy.priority_mode = PriorityMode::Stability;
    }
    let ship = sim.ships.get(&ship_id).expect("ship should exist");
    assert!((ship.policy.min_margin - 3.5).abs() < 1e-9);
    assert!((ship.policy.max_risk_score - 0.9).abs() < 1e-9);
    assert_eq!(ship.policy.max_hops, 3);
    assert_eq!(ship.policy.priority_mode, PriorityMode::Stability);
}

#[test]
fn manual_vs_policy_kpi_updates_from_user_actions() {
    let sim = Simulation::new(RuntimeConfig::default(), 12);
    let mut kpi = UiKpiTracker::default();
    kpi.record_manual_action(sim.tick);
    kpi.record_policy_edit(sim.tick);
    kpi.update(&sim);
    assert!(kpi.manual_actions_per_min >= 1.0);
    assert!(kpi.policy_edits_per_min >= 1.0);
    assert!(kpi.avg_route_hops_player >= 0.0);
}

#[test]
fn hud_snapshot_includes_debt_reputation_and_recovery() {
    let cfg = RuntimeConfig::default();
    let mut sim = Simulation::new(cfg, 7);
    sim.outstanding_debt = 123.0;
    sim.reputation = 0.55;
    sim.current_loan_interest_rate = 0.07;
    sim.recovery_events = 3;
    sim.lease_slot(SystemId(0), SlotType::Dock, 2)
        .expect("lease should succeed");

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
    assert_eq!(snapshot.recovery_events, 3);
    assert!(snapshot.active_leases >= 1);
}

#[test]
fn assets_snapshot_shows_recovery_actions() {
    let mut sim = Simulation::new(RuntimeConfig::default(), 42);
    sim.recovery_log.push(RecoveryAction {
        cycle: 4,
        released_leases: 2,
        capital_after: 40.0,
        debt_after: 180.0,
    });

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
    assert_eq!(snapshot.recovery_actions.len(), 1);
    assert_eq!(snapshot.recovery_actions[0].released_leases, 2);
}

#[test]
fn hud_selected_system_lease_prices_rendered() {
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
    assert_eq!(snapshot.lease_market_lines.len(), 4);
    assert!(snapshot
        .lease_market_lines
        .iter()
        .any(|line| line.starts_with("Dock")));
    assert!(snapshot
        .lease_market_lines
        .iter()
        .any(|line| line.starts_with("Storage")));
}

#[test]
fn right_click_station_opens_context_menu_state() {
    let mut ui = StationUiState::default();
    assert!(!ui.context_menu_open);
    apply_station_context_open(&mut ui, gatebound_core::StationId(7));
    assert!(ui.context_menu_open);
    assert_eq!(ui.context_station_id, Some(gatebound_core::StationId(7)));
}

#[test]
fn station_context_fly_command_sets_ship_route() {
    let mut sim = Simulation::new(RuntimeConfig::default(), 88);
    let ship_id = ShipId(0);
    let target_station = sim
        .world
        .first_station(SystemId(1))
        .or_else(|| sim.world.first_station(SystemId(0)))
        .expect("target station should exist");

    sim.command_fly_to_station(ship_id, target_station)
        .expect("fly command should succeed");
    let ship = sim.ships.get(&ship_id).expect("ship should exist");
    assert!(
        ship.current_target.is_some() || !ship.movement_queue.is_empty(),
        "fly command should produce active movement state"
    );
}

#[test]
fn auto_dock_becomes_true_on_station_arrival() {
    let mut sim = Simulation::new(RuntimeConfig::default(), 91);
    let ship_id = ShipId(0);
    let target_station = sim
        .world
        .first_station(SystemId(1))
        .or_else(|| sim.world.first_station(SystemId(0)))
        .expect("target station should exist");

    sim.command_fly_to_station(ship_id, target_station)
        .expect("fly command should succeed");
    assert!(
        !sim.is_ship_docked_at(ship_id, target_station),
        "ship should not be docked immediately after command"
    );

    for _ in 0..400 {
        sim.step_tick();
        if sim.is_ship_docked_at(ship_id, target_station) {
            break;
        }
    }
    assert!(
        sim.is_ship_docked_at(ship_id, target_station),
        "ship should eventually auto-dock at destination station"
    );
}

#[test]
fn station_trade_actions_change_market_and_cargo_state() {
    let mut cfg = RuntimeConfig::default();
    cfg.pressure.market_fee_rate = 0.1;
    let mut sim = Simulation::new(cfg, 99);
    let ship_id = ShipId(0);
    let station_id = sim
        .world
        .first_station(SystemId(0))
        .expect("station should exist");
    if let Some(ship) = sim.ships.get_mut(&ship_id) {
        ship.current_station = Some(station_id);
        ship.location = SystemId(0);
        ship.eta_ticks_remaining = 0;
        ship.segment_eta_remaining = 0;
        ship.current_segment_kind = None;
        ship.movement_queue.clear();
    }

    let stock_before = sim
        .markets
        .get(&station_id)
        .and_then(|book| book.goods.get(&Commodity::Fuel))
        .map(|state| state.stock)
        .unwrap_or(0.0);
    sim.player_buy(ship_id, station_id, Commodity::Fuel, 6.0)
        .expect("buy should pass");
    sim.player_sell(ship_id, station_id, Commodity::Fuel, 2.0)
        .expect("sell should pass");

    let ship = sim.ships.get(&ship_id).expect("ship should exist");
    assert!(
        ship.cargo.is_some(),
        "ship should retain partial cargo after sell"
    );
    assert_eq!(
        ship.cargo.map(|cargo| cargo.source),
        Some(CargoSource::Spot),
        "spot trading should keep spot cargo source"
    );
    let stock_after = sim
        .markets
        .get(&station_id)
        .and_then(|book| book.goods.get(&Commodity::Fuel))
        .map(|state| state.stock)
        .unwrap_or(0.0);
    assert!(
        stock_after < stock_before,
        "net buy should reduce station stock"
    );
}
