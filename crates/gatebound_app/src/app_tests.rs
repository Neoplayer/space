use gatebound_core::{RuntimeConfig, Simulation, SystemId};

use crate::hud::build_hud_snapshot;
use crate::render_world::ShipMotionCache;
use crate::sim_runtime::{consume_ticks, hotkey_to_risk};
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
fn hud_snapshot_shows_only_active_contracts() {
    let cfg = RuntimeConfig::default();
    let mut sim = Simulation::new(cfg, 42);

    if let Some(contract) = sim.contracts.get_mut(&gatebound_core::ContractId(0)) {
        contract.failed = true;
    }

    let snapshot = build_hud_snapshot(&sim, true, 1, CameraMode::Galaxy);
    assert_eq!(snapshot.active_contracts, 0);
    assert!(snapshot.contract_lines.is_empty());
}

#[test]
fn hotkey_mapping_matches_stage_a_risk_events() {
    assert!(matches!(
        hotkey_to_risk('g'),
        Some(crate::sim_runtime::RiskHotkey::GateCongestion)
    ));
    assert!(matches!(
        hotkey_to_risk('d'),
        Some(crate::sim_runtime::RiskHotkey::DockCongestion)
    ));
    assert!(matches!(
        hotkey_to_risk('f'),
        Some(crate::sim_runtime::RiskHotkey::FuelShock)
    ));
    assert!(hotkey_to_risk('x').is_none());
}
