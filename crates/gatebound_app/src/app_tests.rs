use gatebound_core::{
    ContractOffer, ContractTypeStageA, PriorityMode, RuntimeConfig, Simulation, SlotType, SystemId,
};

use crate::hud::build_hud_snapshot;
use crate::render_world::ShipMotionCache;
use crate::sim_runtime::{
    apply_offer_filters, apply_panel_toggle, consume_ticks, hotkey_to_lease, hotkey_to_risk,
    panel_hotkey_to_index, ContractsFilterState, LeaseHotkey, OfferSortMode, RiskHotkey,
};
use crate::view_mode::{apply_escape, apply_system_click, CameraMode, ClickTracker};

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
}

#[test]
fn contracts_filter_state_applies_margin_risk_eta() {
    let offers = vec![
        ContractOffer {
            id: 0,
            kind: ContractTypeStageA::Delivery,
            origin: SystemId(0),
            destination: SystemId(1),
            quantity: 10.0,
            payout: 30.0,
            penalty: 10.0,
            eta_ticks: 20,
            risk_score: 0.8,
            margin_estimate: 12.0,
            expires_cycle: 10,
        },
        ContractOffer {
            id: 1,
            kind: ContractTypeStageA::Delivery,
            origin: SystemId(0),
            destination: SystemId(2),
            quantity: 10.0,
            payout: 30.0,
            penalty: 10.0,
            eta_ticks: 300,
            risk_score: 0.4,
            margin_estimate: 40.0,
            expires_cycle: 10,
        },
        ContractOffer {
            id: 2,
            kind: ContractTypeStageA::Supply,
            origin: SystemId(1),
            destination: SystemId(2),
            quantity: 8.0,
            payout: 24.0,
            penalty: 9.0,
            eta_ticks: 15,
            risk_score: 1.6,
            margin_estimate: 18.0,
            expires_cycle: 10,
        },
    ];
    let filters = ContractsFilterState {
        min_margin: 10.0,
        max_risk: 1.0,
        max_eta: 120,
        sort_mode: OfferSortMode::MarginDesc,
    };
    let filtered = apply_offer_filters(offers, filters);
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].id, 0);
}

#[test]
fn fleet_snapshot_contains_warning_reason() {
    let cfg = RuntimeConfig::default();
    let mut sim = Simulation::new(cfg, 42);
    if let Some(ship) = sim.ships.get_mut(&gatebound_core::ShipId(0)) {
        ship.active_contract = None;
        ship.eta_ticks_remaining = 0;
        ship.current_target = None;
        ship.planned_path.clear();
    }

    let snapshot = build_hud_snapshot(
        &sim,
        true,
        1,
        CameraMode::Galaxy,
        SystemId(0),
        Some(gatebound_core::ShipId(0)),
        ContractsFilterState::default(),
    );
    let row = snapshot
        .fleet_rows
        .iter()
        .find(|row| row.ship_id == gatebound_core::ShipId(0))
        .expect("ship row should exist");
    assert!(row.warning.is_some());
}

#[test]
fn markets_panel_uses_selected_system_or_fallback() {
    let cfg = RuntimeConfig::default();
    let sim = Simulation::new(cfg, 42);
    let galaxy_snapshot = build_hud_snapshot(
        &sim,
        true,
        1,
        CameraMode::Galaxy,
        SystemId(0),
        None,
        ContractsFilterState::default(),
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
    );
    assert_eq!(system_snapshot.selected_system_id, SystemId(1));
    assert!(!system_snapshot.market_rows.is_empty());
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
    );
    assert!((snapshot.debt - 123.0).abs() < 1e-9);
    assert!((snapshot.reputation - 0.55).abs() < 1e-9);
    assert!((snapshot.interest_rate - 0.07).abs() < 1e-9);
    assert_eq!(snapshot.recovery_events, 3);
    assert!(snapshot.active_leases >= 1);
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
