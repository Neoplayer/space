use crate::input::camera::{
    apply_escape, apply_galaxy_pan_drag, apply_ship_context_open, apply_station_context_open,
    apply_system_click, clamp_galaxy_pan, clamp_zoom, galaxy_pan_bounds, should_start_galaxy_pan,
    zoom_level_for_camera_mode, zoom_level_with_delta, CameraMode, CameraUiState, ClickTracker,
};
use crate::render::world::{
    company_color, faction_color, segment_from_point, ship_is_visible_in_current_view,
    system_objects_visible_in_current_view, update_ship_motion_cache, ShipMotionCache,
};
use crate::runtime::save::{
    apply_loaded_simulation, auto_save_name_from_timestamp, sort_save_summaries,
    storage_manifest_key, storage_payload_key, toggle_save_menu, GameSaveSummary,
    PendingSaveAction, SaveMenuState, SaveStorage,
};
use crate::runtime::sim::{
    apply_offer_filters, apply_panel_toggle, consume_ticks, hotkey_to_risk, open_ship_card,
    open_station_card, open_system_ship_inspector_selection,
    open_system_station_inspector_selection, open_system_view, panel_button_specs,
    panel_hotkey_to_index, seed_markets_ui_state, set_time_speed, toggle_pause, track_ship,
    ContractsFilterState, FinanceUiState, MarketsUiState, OfferSortMode, RiskHotkey, SelectedShip,
    SelectedStation, SelectedSystem, ShipCardTab, ShipUiState, SimResource, StationCardTab,
    StationUiState, TrackedShip, UiKpiTracker, UiPanelState,
};
use crate::ui::hud::{
    build_hud_snapshot as build_hud_snapshot_v2, build_ship_card_snapshot_for_ui,
    build_station_card_snapshot_for_ui, player_fleet_rows,
};
use bevy::prelude::*;
use gatebound_domain::{
    ActiveLoan, CargoLoad, CargoSource, Commodity, CompanyId, ContractOffer, ContractTypeStageA,
    FactionId, GateId, LoanOfferId, OfferProblemTag, PriorityMode, RouteSegment, RuntimeConfig,
    SegmentKind, ShipClass, ShipDescriptor, ShipId, ShipRole, StationId, StationProfile, SystemId,
};
use gatebound_sim::{
    test_support::{
        FinanceStateFixture, MarketStatePatch, ShipCycleMetricsFixture, ShipPatch,
        SimulationScenarioBuilder,
    },
    CameraSystemView, CameraTopologyView, Simulation,
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
        selected_station,
        Commodity::Fuel,
        selected_station,
        None,
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

fn expected_system_name(system_id: SystemId) -> String {
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

fn expected_station_name(station_id: StationId, profile: StationProfile) -> String {
    let role = match profile {
        StationProfile::Civilian => "Concourse",
        StationProfile::Industrial => "Foundry",
        StationProfile::Research => "Array",
    };
    format!("{role}-{:03}", station_id.0)
}

fn test_camera_topology(systems: &[(SystemId, f32, f32, f32)]) -> CameraTopologyView {
    CameraTopologyView {
        systems: systems
            .iter()
            .map(|(system_id, x, y, radius)| CameraSystemView {
                system_id: *system_id,
                owner_faction_id: FactionId(0),
                faction_color_rgb: [255, 255, 255],
                x: f64::from(*x),
                y: f64::from(*y),
                radius: f64::from(*radius),
                stations: Vec::new(),
            })
            .collect(),
        gate_ids: Vec::new(),
    }
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
fn shared_time_controls_update_pause_and_speed() {
    let mut clock = crate::runtime::sim::SimClock::default();
    assert!(!clock.paused);
    assert_eq!(clock.speed_multiplier, 1);

    toggle_pause(&mut clock);
    assert!(clock.paused);

    toggle_pause(&mut clock);
    assert!(!clock.paused);

    set_time_speed(&mut clock, 2);
    assert_eq!(clock.speed_multiplier, 2);

    set_time_speed(&mut clock, 4);
    assert_eq!(clock.speed_multiplier, 4);
}

#[test]
fn double_click_enters_system_and_escape_does_not_exit_system_view() {
    let mut mode = CameraMode::Galaxy;
    let mut tracker = ClickTracker::default();
    let mut helper_mode = CameraMode::Galaxy;

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
    open_system_view(&mut helper_mode, SystemId(2));
    assert_eq!(helper_mode, CameraMode::System(SystemId(2)));
    assert_eq!(mode, CameraMode::System(SystemId(2)));
    assert_eq!(mode, helper_mode);

    apply_escape(&mut mode, true);
    assert_eq!(mode, CameraMode::System(SystemId(2)));
}

#[test]
fn save_menu_toggle_pauses_and_restores_previous_clock_state() {
    let mut menu = SaveMenuState::default();
    let mut clock = crate::runtime::sim::SimClock::default();

    toggle_save_menu(&mut menu, &mut clock);
    assert!(menu.open);
    assert!(clock.paused);

    toggle_save_menu(&mut menu, &mut clock);
    assert!(!menu.open);
    assert!(!clock.paused);

    clock.paused = true;
    toggle_save_menu(&mut menu, &mut clock);
    assert!(menu.open);
    assert!(clock.paused);

    toggle_save_menu(&mut menu, &mut clock);
    assert!(!menu.open);
    assert!(clock.paused);
}

#[test]
fn save_summary_helpers_format_sort_and_name_storage_keys() {
    let mut entries = vec![
        GameSaveSummary {
            id: "older".to_string(),
            display_name: "Older Save".to_string(),
            saved_at_unix: 10,
            world_time_label: "3500-01-01 00:00".to_string(),
            capital: 400.0,
            debt: 50.0,
            reputation: 0.4,
        },
        GameSaveSummary {
            id: "newer".to_string(),
            display_name: "Newer Save".to_string(),
            saved_at_unix: 25,
            world_time_label: "3500-01-02 00:00".to_string(),
            capital: 450.0,
            debt: 20.0,
            reputation: 0.6,
        },
    ];

    sort_save_summaries(&mut entries);

    assert_eq!(entries[0].id, "newer");
    assert_eq!(entries[1].id, "older");
    assert_eq!(
        auto_save_name_from_timestamp(0),
        "Save 1970-01-01 00:00:00 UTC"
    );
    assert_eq!(storage_manifest_key(), "gatebound.saves.manifest.v1");
    assert_eq!(
        storage_payload_key("slot-42"),
        "gatebound.saves.payload.v1.slot-42"
    );
}

#[test]
fn loading_save_resets_runtime_ui_state_and_closes_menu() {
    let original = Simulation::new(RuntimeConfig::default(), 42);
    let loaded = Simulation::new(RuntimeConfig::default(), 77);
    let loaded_hash = loaded.snapshot_hash();

    let mut sim_resource = SimResource::new(original);
    let mut clock = crate::runtime::sim::SimClock {
        paused: false,
        speed_multiplier: 4,
        accumulator_seconds: 3.5,
    };
    let mut camera = CameraUiState {
        mode: CameraMode::System(SystemId(2)),
        galaxy_pan: Vec2::new(12.0, -7.0),
        zoom_level: 2.0,
        ..CameraUiState::default()
    };
    let mut selected_system = SelectedSystem {
        system_id: SystemId(2),
    };
    let mut selected_station = SelectedStation {
        station_id: Some(StationId(3)),
    };
    let mut selected_ship = SelectedShip {
        ship_id: Some(ShipId(1)),
    };
    let mut panels = UiPanelState {
        contracts: true,
        fleet: true,
        markets: true,
        assets: true,
        policies: true,
        station_ops: true,
        corporations: true,
        systems: true,
    };
    let mut filters = ContractsFilterState {
        min_margin: 9.0,
        max_risk: 0.2,
        max_eta: 12,
        commodity: Some(Commodity::Ore),
        route_gate: Some(GateId(1)),
        problem: Some(OfferProblemTag::LowMargin),
        premium_only: true,
        sort_mode: OfferSortMode::EtaAsc,
    };
    let mut tracked_ship = TrackedShip {
        ship_id: Some(ShipId(1)),
    };
    let mut ship_ui = ShipUiState {
        context_ship_id: Some(ShipId(1)),
        card_ship_id: Some(ShipId(1)),
        context_menu_open: true,
        card_open: true,
        card_tab: ShipCardTab::Technical,
    };
    let mut station_ui = StationUiState {
        context_station_id: Some(StationId(3)),
        card_station_id: Some(StationId(3)),
        context_menu_open: true,
        station_panel_open: true,
        card_tab: StationCardTab::Trade,
        trade_commodity: Commodity::Ore,
        trade_quantity: 42.0,
        storage_commodity: Commodity::Fuel,
        storage_quantity: 7.0,
    };
    let mut markets_ui = MarketsUiState {
        detail_station_id: Some(StationId(3)),
        focused_commodity: Commodity::Ore,
        seeded_from_world_selection: true,
    };
    let mut finance_ui = FinanceUiState {
        pending_offer: Some(LoanOfferId::Growth),
        repayment_amount: 77.0,
    };
    let mut kpi = UiKpiTracker::default();
    kpi.record_manual_action(3);
    kpi.record_policy_edit(3);
    let mut messages = crate::ui::hud::HudMessages {
        entries: vec!["stale".to_string()],
    };
    let mut menu = SaveMenuState {
        open: true,
        selected_entry_id: Some("save-77".to_string()),
        entries: vec![GameSaveSummary {
            id: "save-77".to_string(),
            display_name: "Save 77".to_string(),
            saved_at_unix: 77,
            world_time_label: "3500-01-01 00:00".to_string(),
            capital: 700.0,
            debt: 15.0,
            reputation: 0.9,
        }],
        pending_action: Some(PendingSaveAction::Load("save-77".to_string())),
        last_error: Some("old error".to_string()),
        paused_before_open: Some(false),
    };

    apply_loaded_simulation(
        loaded,
        "Save 77",
        &mut sim_resource,
        &mut clock,
        &mut camera,
        &mut selected_system,
        &mut selected_station,
        &mut selected_ship,
        &mut panels,
        &mut filters,
        &mut tracked_ship,
        &mut ship_ui,
        &mut station_ui,
        &mut markets_ui,
        &mut finance_ui,
        &mut kpi,
        &mut messages,
        &mut menu,
    );

    assert_eq!(sim_resource.simulation.snapshot_hash(), loaded_hash);
    assert_eq!(
        clock,
        crate::runtime::sim::SimClock {
            paused: true,
            speed_multiplier: 1,
            accumulator_seconds: 0.0,
        }
    );
    assert_eq!(camera, CameraUiState::default());
    assert_eq!(selected_system, SelectedSystem::default());
    assert_eq!(selected_station, SelectedStation::default());
    assert_eq!(selected_ship, SelectedShip::default());
    assert_eq!(panels, UiPanelState::default());
    assert_eq!(filters, ContractsFilterState::default());
    assert_eq!(tracked_ship, TrackedShip::default());
    assert_eq!(ship_ui, ShipUiState::default());
    assert_eq!(station_ui, StationUiState::default());
    assert_eq!(markets_ui, MarketsUiState::default());
    assert_eq!(finance_ui, FinanceUiState::default());
    assert_eq!(kpi, UiKpiTracker::default());
    assert_eq!(menu, SaveMenuState::default());
    assert!(messages
        .entries
        .last()
        .is_some_and(|entry| entry.contains("Loaded save Save 77")));
}

#[test]
fn save_storage_create_overwrite_and_load_round_trip() {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("time should be monotonic enough for test dir")
        .as_nanos();
    let save_dir = std::env::temp_dir().join(format!("gatebound_app_save_flow_{unique}"));
    let storage = SaveStorage::for_test_desktop_dir(save_dir);

    let mut builder = SimulationScenarioBuilder::stage_a(91);
    let station_id = builder
        .first_station_in_system(SystemId(0))
        .expect("system 0 should have a station");
    builder.with_ship_patch(
        ShipId(0),
        ShipPatch {
            location: Some(SystemId(0)),
            current_station: Some(Some(station_id)),
            eta_ticks_remaining: Some(0),
            segment_eta_remaining: Some(0),
            segment_progress_total: Some(0),
            movement_queue: Some(Vec::new()),
            active_contract: Some(None),
            cargo: Some(Some(CargoLoad {
                commodity: Commodity::Fuel,
                amount: 6.0,
                source: CargoSource::Spot,
            })),
            ..ShipPatch::default()
        },
    );
    let mut sim = builder.build();
    sim.player_unload_to_station_storage(ShipId(0), station_id, 4.0)
        .expect("station storage unload should pass");
    let original_payload = sim
        .snapshot_payload()
        .expect("snapshot payload should serialize");
    let created = storage
        .create_new_save(&sim)
        .expect("create save should pass");
    assert!(created.display_name.starts_with("Save "));

    let summaries = storage.list_summaries().expect("list should pass");
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].id, created.id);

    let loaded_envelope = storage.load_save(&created.id).expect("load should pass");
    assert_eq!(loaded_envelope.payload, original_payload);
    let loaded = loaded_envelope
        .into_simulation(RuntimeConfig::default())
        .expect("payload should deserialize");
    assert_eq!(loaded.tick(), sim.tick());
    assert_eq!(loaded.cycle(), sim.cycle());
    assert!((loaded.capital() - sim.capital()).abs() < 1e-9);
    let fuel_row = loaded
        .station_storage_view(ShipId(0), station_id)
        .expect("station storage view should exist")
        .rows
        .into_iter()
        .find(|row| row.commodity == Commodity::Fuel)
        .expect("fuel storage row should persist");
    assert!((fuel_row.stored_amount - 4.0).abs() < 1e-9);

    let mut updated_builder = SimulationScenarioBuilder::stage_a(91);
    updated_builder.with_ship_patch(
        ShipId(0),
        ShipPatch {
            location: Some(SystemId(0)),
            current_station: Some(Some(station_id)),
            eta_ticks_remaining: Some(0),
            segment_eta_remaining: Some(0),
            segment_progress_total: Some(0),
            movement_queue: Some(Vec::new()),
            active_contract: Some(None),
            cargo: Some(Some(CargoLoad {
                commodity: Commodity::Fuel,
                amount: 8.0,
                source: CargoSource::Spot,
            })),
            ..ShipPatch::default()
        },
    );
    let mut updated = updated_builder.build();
    updated.step_tick();
    updated
        .player_unload_to_station_storage(ShipId(0), station_id, 5.0)
        .expect("updated station storage unload should pass");
    let updated_payload = updated
        .snapshot_payload()
        .expect("snapshot payload should serialize");

    let overwritten = storage
        .overwrite_save(&created.id, &updated)
        .expect("overwrite should pass");
    assert_eq!(overwritten.id, created.id);
    assert_eq!(overwritten.display_name, created.display_name);

    let summaries = storage.list_summaries().expect("list should pass");
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].id, created.id);

    let loaded_envelope = storage.load_save(&created.id).expect("load should pass");
    assert_eq!(loaded_envelope.payload, updated_payload);
    let loaded = loaded_envelope
        .into_simulation(RuntimeConfig::default())
        .expect("payload should deserialize");
    assert_eq!(loaded.tick(), updated.tick());
    assert_eq!(loaded.cycle(), updated.cycle());
    assert!((loaded.capital() - updated.capital()).abs() < 1e-9);
    let fuel_row = loaded
        .station_storage_view(ShipId(0), station_id)
        .expect("station storage view should exist")
        .rows
        .into_iter()
        .find(|row| row.commodity == Commodity::Fuel)
        .expect("fuel storage row should persist");
    assert!((fuel_row.stored_amount - 5.0).abs() < 1e-9);
}

#[test]
fn galaxy_pan_drag_applies_world_space_delta() {
    let topology = test_camera_topology(&[
        (SystemId(0), -600.0, -450.0, 50.0),
        (SystemId(1), 600.0, 450.0, 50.0),
    ]);

    let pan = apply_galaxy_pan_drag(
        Vec2::ZERO,
        Vec2::new(10.0, 20.0),
        Vec2::new(35.0, -10.0),
        &topology,
        Vec2::new(200.0, 100.0),
        1.0,
    );

    assert_eq!(pan, Vec2::new(-25.0, 30.0));
}

#[test]
fn zoom_step_is_smoother_for_scroll_input() {
    assert_eq!(clamp_zoom(1.0, 1.0, 0.3, 4.0), 0.92);
    assert_eq!(clamp_zoom(1.0, -1.0, 0.3, 4.0), 1.08);
}

#[test]
fn system_view_ignores_zoom_delta_but_galaxy_view_keeps_existing_behavior() {
    assert_eq!(
        zoom_level_with_delta(CameraMode::System(SystemId(2)), 1.7, 1.0, 0.3, 4.0),
        1.7
    );
    assert_eq!(
        zoom_level_with_delta(CameraMode::Galaxy, 1.0, 1.0, 0.3, 4.0),
        0.92
    );
}

#[test]
fn system_view_uses_fixed_zoom_min_scale_while_galaxy_uses_stored_zoom() {
    assert_eq!(
        zoom_level_for_camera_mode(CameraMode::System(SystemId(4)), 1.7, 0.3),
        0.3
    );
    assert_eq!(
        zoom_level_for_camera_mode(CameraMode::Galaxy, 1.7, 0.3),
        1.7
    );
}

#[test]
fn galaxy_pan_clamps_to_bounds_and_recenters_large_viewports() {
    let topology = test_camera_topology(&[
        (SystemId(0), -100.0, 0.0, 50.0),
        (SystemId(1), 100.0, 0.0, 50.0),
    ]);
    let bounds = galaxy_pan_bounds(&topology).expect("topology should produce pan bounds");

    assert_eq!(
        clamp_galaxy_pan(Vec2::new(500.0, -500.0), bounds, Vec2::new(80.0, 60.0)),
        Vec2::new(130.0, -50.0)
    );
    assert_eq!(
        clamp_galaxy_pan(Vec2::new(90.0, 45.0), bounds, Vec2::new(400.0, 300.0)),
        Vec2::ZERO
    );
}

#[test]
fn galaxy_pan_and_zoom_survive_manual_system_exit_after_escape_noop() {
    let mut ui = CameraUiState {
        zoom_level: 1.7,
        galaxy_pan: Vec2::new(180.0, -75.0),
        ..CameraUiState::default()
    };
    let mut tracker = ClickTracker::default();

    assert!(!apply_system_click(
        &mut ui.mode,
        &mut tracker,
        SystemId(2),
        0.0
    ));
    assert!(apply_system_click(
        &mut ui.mode,
        &mut tracker,
        SystemId(2),
        0.2
    ));

    apply_escape(&mut ui.mode, true);

    assert_eq!(ui.mode, CameraMode::System(SystemId(2)));
    ui.mode = CameraMode::Galaxy;
    assert_eq!(ui.mode, CameraMode::Galaxy);
    assert_eq!(ui.zoom_level, 1.7);
    assert_eq!(ui.galaxy_pan, Vec2::new(180.0, -75.0));
}

#[test]
fn galaxy_pan_only_starts_for_unblocked_galaxy_view() {
    assert!(should_start_galaxy_pan(CameraMode::Galaxy, false));
    assert!(!should_start_galaxy_pan(CameraMode::Galaxy, true));
    assert!(!should_start_galaxy_pan(
        CameraMode::System(SystemId(3)),
        false
    ));
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
fn station_ui_state_defaults_include_info_tab() {
    let state = StationUiState::default();
    assert_eq!(state.card_tab, StationCardTab::Info);
    assert_eq!(state.card_station_id, None);
    assert_eq!(state.trade_commodity, Commodity::Fuel);
    assert!(state.trade_quantity > 0.0);
    assert_eq!(state.storage_commodity, Commodity::Fuel);
    assert!(state.storage_quantity > 0.0);
}

#[test]
fn markets_ui_state_defaults_focus_fuel_and_have_no_detail_station() {
    let state = MarketsUiState::default();
    assert_eq!(state.detail_station_id, None);
    assert_eq!(state.focused_commodity, Commodity::Fuel);
    assert!(!state.seeded_from_world_selection);
}

#[test]
fn seed_markets_ui_state_uses_world_selection_once_and_preserves_manual_pick() {
    let sim = Simulation::new(RuntimeConfig::default(), 42);
    let mut state = MarketsUiState::default();

    seed_markets_ui_state(&mut state, &sim, SystemId(0), Some(StationId(1)));
    assert_eq!(state.detail_station_id, Some(StationId(1)));
    assert!(state.seeded_from_world_selection);

    state.detail_station_id = Some(StationId(3));
    seed_markets_ui_state(&mut state, &sim, SystemId(0), Some(StationId(0)));
    assert_eq!(state.detail_station_id, Some(StationId(3)));
}

#[test]
fn seed_markets_ui_state_does_not_fallback_to_other_system_station() {
    let sim = Simulation::new(RuntimeConfig::default(), 42);
    let mut state = MarketsUiState::default();

    seed_markets_ui_state(&mut state, &sim, SystemId(999), None);

    assert_eq!(state.detail_station_id, None);
    assert!(state.seeded_from_world_selection);
}

#[test]
fn panel_hotkeys_toggle_expected_windows() {
    let mut panels = crate::runtime::sim::UiPanelState::default();
    assert!(!panels.contracts);
    assert!(!panels.fleet);
    assert!(!panels.markets);
    assert!(!panels.assets);
    assert!(!panels.policies);
    assert!(!panels.station_ops);
    assert!(!panels.corporations);
    assert!(!panels.systems);

    assert_eq!(panel_hotkey_to_index('1'), Some(1));
    apply_panel_toggle(&mut panels, 1);
    assert!(panels.contracts);
    apply_panel_toggle(&mut panels, 2);
    assert!(panels.fleet);
    apply_panel_toggle(&mut panels, 3);
    assert!(panels.markets);
    apply_panel_toggle(&mut panels, 4);
    assert!(panels.assets);
    apply_panel_toggle(&mut panels, 5);
    assert!(panels.policies);
    assert_eq!(panel_hotkey_to_index('6'), Some(6));
    apply_panel_toggle(&mut panels, 6);
    assert!(panels.station_ops);
    assert_eq!(panel_hotkey_to_index('7'), Some(7));
    apply_panel_toggle(&mut panels, 7);
    assert!(panels.corporations);
    assert_eq!(panel_hotkey_to_index('8'), Some(8));
    apply_panel_toggle(&mut panels, 8);
    assert!(panels.systems);
}

#[test]
fn left_panel_buttons_cover_all_windows() {
    let buttons: Vec<_> = panel_button_specs()
        .iter()
        .map(|button| (button.index, button.label, button.hotkey))
        .collect();

    assert_eq!(
        buttons,
        vec![
            (1, "Contracts", "F1"),
            (2, "MyShip", "F2"),
            (3, "Markets", "F3"),
            (4, "Finance", "F4"),
            (5, "Policies", "F5"),
            (6, "Station", "F6"),
            (7, "Corps", "F7"),
            (8, "Systems", "F8"),
        ]
    );
}

#[test]
fn corporations_snapshot_exposes_balance_and_ship_counts() {
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
    assert_eq!(snapshot.corporation_rows.len(), 6);
    let row = snapshot
        .corporation_rows
        .iter()
        .find(|row| row.company_id == CompanyId(1))
        .expect("haulers alpha row should exist");
    assert_eq!(row.name, "Haulers Alpha");
    assert_eq!(row.idle_ships, 10);
    assert_eq!(row.in_transit_ships, 0);
    assert_eq!(row.active_orders, 0);
    assert!((row.balance - 1400.0).abs() < 1e-9);
    assert_eq!(row.next_plan_tick, 1);
}

#[test]
fn company_palette_is_distinct_for_player_and_all_npc_corps() {
    let colors = (0..=6)
        .map(|company_id| company_color(company_id).to_srgba())
        .collect::<Vec<_>>();
    for i in 0..colors.len() {
        for j in (i + 1)..colors.len() {
            assert_ne!(colors[i], colors[j]);
        }
    }
}

#[test]
fn faction_color_uses_exact_config_rgb_triplet() {
    let color = faction_color([12, 34, 56]).to_srgba();
    assert!((color.red - 12.0 / 255.0).abs() < 1e-6);
    assert!((color.green - 34.0 / 255.0).abs() < 1e-6);
    assert!((color.blue - 56.0 / 255.0).abs() < 1e-6);
    assert!((color.alpha - 1.0).abs() < 1e-6);
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
fn fleet_manager_rows_only_include_player_ships() {
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

    let rows = player_fleet_rows(&snapshot.fleet_rows);
    assert_eq!(
        rows.len(),
        1,
        "stage_a fleet manager should show one player ship"
    );
    assert!(rows.iter().all(|row| row.company_id == CompanyId(0)));
}

#[test]
fn fleet_list_rows_only_include_player_ships_and_sort_by_name() {
    let mut builder = SimulationScenarioBuilder::stage_a(42);
    let extra_player_ship_id = builder.first_npc_ship_id().expect("npc ship should exist");
    builder.with_ship_patch(
        extra_player_ship_id,
        ShipPatch {
            company_id: Some(CompanyId(0)),
            role: Some(ShipRole::PlayerContract),
            descriptor: Some(ShipDescriptor {
                name: "Aquila Runner".to_string(),
                class: ShipClass::Courier,
                description: "Converted player courier".to_string(),
            }),
            ..ShipPatch::default()
        },
    );
    builder.with_ship_patch(
        ShipId(0),
        ShipPatch {
            descriptor: Some(ShipDescriptor {
                name: "Zephyr Mule".to_string(),
                class: ShipClass::Hauler,
                description: "Primary player hauler".to_string(),
            }),
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

    let ship_names = snapshot
        .fleet_list_rows
        .iter()
        .map(|row| row.ship_name.as_str())
        .collect::<Vec<_>>();
    let ship_ids = snapshot
        .fleet_list_rows
        .iter()
        .map(|row| row.ship_id)
        .collect::<Vec<_>>();

    assert_eq!(ship_names, vec!["Aquila Runner", "Zephyr Mule"]);
    assert_eq!(ship_ids, vec![extra_player_ship_id, ShipId(0)]);
}

#[test]
fn fleet_list_rows_render_human_readable_location_and_status() {
    let mut builder = SimulationScenarioBuilder::stage_a(42);
    let docked_ship_id = ShipId(0);
    let transit_ship_id = builder.first_npc_ship_id().expect("npc ship should exist");
    let idle_ship_id = ShipId(2);
    let docked_station_id = builder
        .first_station_in_system(SystemId(0))
        .expect("system 0 should have a station");
    builder.dock_ship_at(docked_ship_id, docked_station_id);
    builder.with_ship_patch(
        docked_ship_id,
        ShipPatch {
            descriptor: Some(ShipDescriptor {
                name: "Atlas Docked".to_string(),
                class: ShipClass::Hauler,
                description: "Docked player ship".to_string(),
            }),
            ..ShipPatch::default()
        },
    );
    builder.with_ship_patch(
        transit_ship_id,
        ShipPatch {
            company_id: Some(CompanyId(0)),
            role: Some(ShipRole::PlayerContract),
            location: Some(SystemId(1)),
            current_station: Some(None),
            current_target: Some(Some(SystemId(2))),
            eta_ticks_remaining: Some(17),
            current_segment_kind: Some(Some(SegmentKind::InSystem)),
            descriptor: Some(ShipDescriptor {
                name: "Beacon Transit".to_string(),
                class: ShipClass::Courier,
                description: "Transit player ship".to_string(),
            }),
            ..ShipPatch::default()
        },
    );
    builder.with_ship_patch(
        idle_ship_id,
        ShipPatch {
            company_id: Some(CompanyId(0)),
            role: Some(ShipRole::PlayerContract),
            location: Some(SystemId(3)),
            current_station: Some(None),
            current_target: Some(None),
            eta_ticks_remaining: Some(0),
            current_segment_kind: Some(None),
            descriptor: Some(ShipDescriptor {
                name: "Comet Idle".to_string(),
                class: ShipClass::Courier,
                description: "Idle player ship".to_string(),
            }),
            ..ShipPatch::default()
        },
    );
    let sim = builder.build();
    let docked_station = sim
        .camera_topology_view()
        .systems
        .into_iter()
        .flat_map(|system| system.stations.into_iter())
        .find(|station| station.station_id == docked_station_id)
        .expect("docked station should exist");
    let docked_station_name = expected_station_name(docked_station_id, docked_station.profile);
    let docked_system_name = expected_system_name(SystemId(0));
    let transit_system_name = expected_system_name(SystemId(1));
    let transit_target_name = expected_system_name(SystemId(2));
    let idle_system_name = expected_system_name(SystemId(3));

    let snapshot = build_hud_snapshot(
        &sim,
        true,
        1,
        CameraMode::Galaxy,
        SystemId(0),
        Some(docked_ship_id),
        ContractsFilterState::default(),
        &UiKpiTracker::default(),
    );

    let docked_row = snapshot
        .fleet_list_rows
        .iter()
        .find(|row| row.ship_id == docked_ship_id)
        .expect("docked row should exist");
    assert_eq!(
        docked_row.location_text,
        format!("{docked_station_name}, {docked_system_name}")
    );
    assert_eq!(
        docked_row.status_text,
        format!("Docked at {docked_station_name}")
    );

    let transit_row = snapshot
        .fleet_list_rows
        .iter()
        .find(|row| row.ship_id == transit_ship_id)
        .expect("transit row should exist");
    assert_eq!(transit_row.location_text, transit_system_name);
    assert_eq!(
        transit_row.status_text,
        format!("In transit to {transit_target_name} • ETA 17")
    );

    let idle_row = snapshot
        .fleet_list_rows
        .iter()
        .find(|row| row.ship_id == idle_ship_id)
        .expect("idle row should exist");
    assert_eq!(idle_row.location_text, idle_system_name);
    assert_eq!(idle_row.status_text, format!("Idle in {idle_system_name}"));
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
fn markets_snapshot_contains_galaxy_dashboard_sections() {
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
    assert!(snapshot.markets.global_kpis.system_count > 0);
    assert!(snapshot.markets.global_kpis.station_count > 0);
    assert!(!snapshot.markets.commodity_rows.is_empty());
    assert!(snapshot.markets.commodity_rows[0]
        .forecast_next_avg
        .is_finite());
    assert!(!snapshot.markets.system_stress_rows.is_empty());
    assert!(snapshot.markets.station_detail.is_some());
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
    assert!(!galaxy_snapshot.markets.commodity_rows.is_empty());

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
    assert!(!system_snapshot.markets.commodity_rows.is_empty());
}

#[test]
fn markets_snapshot_uses_independent_detail_station() {
    let mut builder = SimulationScenarioBuilder::stage_a(42);
    let system_id = SystemId(0);
    let stations = builder.stations_in_system(system_id);
    if stations.len() < 2 {
        return;
    }
    let world_selected_station = stations[0];
    let detail_station = stations[1];
    builder.with_market_state_patch(
        world_selected_station,
        Commodity::Fuel,
        MarketStatePatch {
            stock: Some(9.0),
            ..MarketStatePatch::default()
        },
    );
    builder.with_market_state_patch(
        detail_station,
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
        Some(world_selected_station),
        Some(detail_station),
        Commodity::Fuel,
        None,
        None,
        None,
        ContractsFilterState::default(),
        &UiKpiTracker::default(),
    );
    let fuel_row = snapshot
        .markets
        .station_detail
        .as_ref()
        .expect("station detail should exist")
        .commodity_rows
        .iter()
        .find(|row| row.commodity == Commodity::Fuel)
        .expect("fuel row should exist");
    assert_eq!(snapshot.selected_station_id, Some(world_selected_station));
    assert_eq!(
        snapshot
            .markets
            .station_detail
            .as_ref()
            .map(|detail| detail.station_id),
        Some(detail_station)
    );
    assert!((fuel_row.local_stock - 77.0).abs() < 1e-9);
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
fn hud_snapshot_formats_calendar_time_from_tick_config() {
    let mut cfg = RuntimeConfig::default();
    cfg.time.day_ticks = 10;
    cfg.time.days_per_month = 2;
    cfg.time.months_per_year = 2;
    cfg.time.start_year = 3500;

    let mut sim = Simulation::new(cfg, 42);

    let snapshot = build_hud_snapshot(
        &sim,
        false,
        1,
        CameraMode::Galaxy,
        SystemId(0),
        None,
        ContractsFilterState::default(),
        &UiKpiTracker::default(),
    );
    assert_eq!(snapshot.time_label, "3500-01-01 00:00");

    for _ in 0..5 {
        sim.step_tick();
    }
    let snapshot = build_hud_snapshot(
        &sim,
        false,
        1,
        CameraMode::Galaxy,
        SystemId(0),
        None,
        ContractsFilterState::default(),
        &UiKpiTracker::default(),
    );
    assert_eq!(snapshot.time_label, "3500-01-01 12:00");

    for _ in 0..15 {
        sim.step_tick();
    }
    let snapshot = build_hud_snapshot(
        &sim,
        false,
        1,
        CameraMode::Galaxy,
        SystemId(0),
        None,
        ContractsFilterState::default(),
        &UiKpiTracker::default(),
    );
    assert_eq!(snapshot.time_label, "3500-02-01 00:00");

    for _ in 0..20 {
        sim.step_tick();
    }
    let snapshot = build_hud_snapshot(
        &sim,
        false,
        1,
        CameraMode::Galaxy,
        SystemId(0),
        None,
        ContractsFilterState::default(),
        &UiKpiTracker::default(),
    );
    assert_eq!(snapshot.time_label, "3501-01-01 00:00");
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
fn right_click_ship_opens_context_menu_state() {
    let mut ui = ShipUiState::default();
    assert!(!ui.context_menu_open);
    apply_ship_context_open(&mut ui, ShipId(9));
    assert!(ui.context_menu_open);
    assert_eq!(ui.context_ship_id, Some(ShipId(9)));
}

#[test]
fn opening_station_card_resets_tab_and_prefers_supplied_commodity() {
    let mut ui = StationUiState {
        card_tab: StationCardTab::Storage,
        trade_commodity: Commodity::Ore,
        ..StationUiState::default()
    };

    open_station_card(&mut ui, StationId(3), Some(Commodity::Electronics));

    assert!(ui.station_panel_open);
    assert_eq!(ui.card_station_id, Some(StationId(3)));
    assert_eq!(ui.card_tab, StationCardTab::Info);
    assert_eq!(ui.trade_commodity, Commodity::Electronics);

    ui.card_tab = StationCardTab::Trade;
    open_station_card(&mut ui, StationId(7), Some(Commodity::Fuel));
    assert_eq!(ui.card_station_id, Some(StationId(7)));
    assert_eq!(ui.card_tab, StationCardTab::Info);
    assert_eq!(ui.trade_commodity, Commodity::Fuel);
}

#[test]
fn opening_ship_card_resets_tab_and_binds_requested_ship() {
    let mut ui = ShipUiState {
        card_tab: ShipCardTab::Modules,
        ..ShipUiState::default()
    };

    open_ship_card(&mut ui, ShipId(3));

    assert!(ui.card_open);
    assert_eq!(ui.card_ship_id, Some(ShipId(3)));
    assert_eq!(ui.card_tab, ShipCardTab::Overview);

    ui.card_tab = ShipCardTab::Technical;
    open_ship_card(&mut ui, ShipId(7));
    assert_eq!(ui.card_ship_id, Some(ShipId(7)));
    assert_eq!(ui.card_tab, ShipCardTab::Overview);
}

#[test]
fn system_inspector_station_selection_opens_station_card_window() {
    let mut selected_station = SelectedStation::default();
    let mut panels = UiPanelState::default();
    let mut station_ui = StationUiState {
        trade_commodity: Commodity::Ore,
        ..StationUiState::default()
    };

    open_system_station_inspector_selection(
        &mut selected_station,
        &mut panels,
        &mut station_ui,
        StationId(7),
        Some(Commodity::Fuel),
    );

    assert_eq!(selected_station.station_id, Some(StationId(7)));
    assert!(panels.station_ops);
    assert!(station_ui.station_panel_open);
    assert_eq!(station_ui.card_station_id, Some(StationId(7)));
    assert_eq!(station_ui.trade_commodity, Commodity::Fuel);
}

#[test]
fn system_inspector_ship_selection_opens_ship_card_window() {
    let mut selected_ship = SelectedShip {
        ship_id: Some(ShipId(0)),
    };
    let mut ship_ui = ShipUiState::default();

    open_system_ship_inspector_selection(&mut selected_ship, &mut ship_ui, ShipId(9));

    assert_eq!(selected_ship.ship_id, Some(ShipId(0)));
    assert!(ship_ui.card_open);
    assert_eq!(ship_ui.card_ship_id, Some(ShipId(9)));
    assert_eq!(ship_ui.card_tab, ShipCardTab::Overview);
}

#[test]
fn inspector_selections_support_live_ship_and_station_cards_without_rebuilding_hud_snapshot() {
    let sim = Simulation::new(RuntimeConfig::default(), 42);
    let mut selected_ship = SelectedShip {
        ship_id: Some(ShipId(0)),
    };
    let mut ship_ui = ShipUiState::default();
    open_system_ship_inspector_selection(&mut selected_ship, &mut ship_ui, ShipId(1));
    let ship_card = build_ship_card_snapshot_for_ui(
        &sim,
        ship_ui.card_ship_id.expect("ship card should be targeted"),
    )
    .expect("live ship card should resolve");
    assert_eq!(ship_card.ship_id, ShipId(1));
    assert_eq!(selected_ship.ship_id, Some(ShipId(0)));

    let station_id = sim
        .camera_topology_view()
        .systems
        .iter()
        .find_map(|system| system.stations.first().map(|station| station.station_id))
        .expect("fixture should include a station");
    let mut selected_station = SelectedStation::default();
    let mut panels = UiPanelState::default();
    let mut station_ui = StationUiState::default();
    open_system_station_inspector_selection(
        &mut selected_station,
        &mut panels,
        &mut station_ui,
        station_id,
        Some(Commodity::Fuel),
    );
    let station_card = build_station_card_snapshot_for_ui(&sim, ShipId(0), station_id)
        .expect("live station card should resolve");
    assert_eq!(station_card.station_id, station_id);
}

#[test]
fn tracking_npc_ship_updates_focus_without_changing_selected_player_ship() {
    let sim = Simulation::new(RuntimeConfig::default(), 42);
    let tracked_id = ShipId(1);
    let expected_system = sim
        .ship_card_view(tracked_id)
        .expect("tracked ship card should exist")
        .location;
    let mut tracked = TrackedShip::default();
    let mut camera = CameraUiState::default();
    let selected_player_ship = ShipId(0);

    track_ship(&mut tracked, &mut camera, &sim, tracked_id)
        .expect("tracking seeded ship should succeed");

    assert_eq!(tracked.ship_id, Some(tracked_id));
    assert_eq!(camera.mode, CameraMode::System(expected_system));
    assert_eq!(selected_player_ship, ShipId(0));
}

#[test]
fn ship_card_snapshot_includes_owner_and_module_metadata() {
    let mut builder = SimulationScenarioBuilder::stage_a(42);
    let npc_id = builder.first_npc_ship_id().expect("npc ship should exist");
    builder.with_ship_patch(
        npc_id,
        ShipPatch {
            cargo: Some(Some(CargoLoad {
                commodity: Commodity::Parts,
                amount: 4.0,
                source: CargoSource::Spot,
            })),
            ..ShipPatch::default()
        },
    );
    let sim = builder.build();

    let snapshot = build_hud_snapshot_v2(
        &sim,
        true,
        1,
        CameraMode::System(SystemId(0)),
        SystemId(0),
        None,
        None,
        Commodity::Fuel,
        None,
        Some(npc_id),
        None,
        ContractsFilterState::default(),
        &UiKpiTracker::default(),
    );
    let card = snapshot
        .ship_card
        .as_ref()
        .expect("ship card snapshot should be present");

    assert_eq!(card.ship_id, npc_id);
    assert!(!card.owner_name.is_empty());
    assert!(!card.modules.is_empty());
    assert!(card.technical_state.hull > 0.0);
}

#[test]
fn station_card_snapshot_builds_generated_info_metadata() {
    let mut builder = SimulationScenarioBuilder::stage_a(42);
    let station_id = StationId(0);
    builder.with_ship_patch(
        ShipId(0),
        ShipPatch {
            location: Some(SystemId(0)),
            current_station: Some(Some(station_id)),
            eta_ticks_remaining: Some(0),
            segment_eta_remaining: Some(0),
            segment_progress_total: Some(0),
            movement_queue: Some(Vec::new()),
            active_contract: Some(None),
            cargo: Some(Some(CargoLoad {
                commodity: Commodity::Fuel,
                amount: 5.0,
                source: CargoSource::Spot,
            })),
            ..ShipPatch::default()
        },
    );
    let mut sim = builder.build();
    sim.player_unload_to_station_storage(ShipId(0), station_id, 3.0)
        .expect("station storage unload should pass");
    let snapshot = build_hud_snapshot(
        &sim,
        false,
        1,
        CameraMode::System(SystemId(0)),
        SystemId(0),
        Some(ShipId(0)),
        ContractsFilterState::default(),
        &UiKpiTracker::default(),
    );

    let card = snapshot
        .station_card
        .as_ref()
        .expect("station card snapshot should be present");

    assert_eq!(card.station_id, StationId(0));
    assert!(!card.station_name.is_empty());
    assert!(!card.system_name.is_empty());
    assert!(!card.host_body_name.is_empty());
    assert!(card.orbit_label.contains("orbit"));
    assert!(card.profile_summary.len() > 20);
    assert!(!card.imports.is_empty());
    assert!(!card.exports.is_empty());
    let fuel_row = card
        .storage
        .rows
        .iter()
        .find(|row| row.commodity == Commodity::Fuel)
        .expect("fuel storage row should be present");
    assert!((fuel_row.stored_amount - 3.0).abs() < 1e-9);
    assert!((fuel_row.player_cargo - 2.0).abs() < 1e-9);
}

#[test]
fn system_panel_snapshot_is_hidden_in_galaxy_mode() {
    let sim = Simulation::new(RuntimeConfig::default(), 42);

    let snapshot = build_hud_snapshot(
        &sim,
        false,
        1,
        CameraMode::Galaxy,
        SystemId(0),
        Some(ShipId(0)),
        ContractsFilterState::default(),
        &UiKpiTracker::default(),
    );

    assert!(snapshot.system_panel.is_none());
}

#[test]
fn system_panel_snapshot_exposes_owner_metrics_stations_and_local_ships() {
    let mut builder = SimulationScenarioBuilder::stage_a(58);
    let selected_system_id = (0..25)
        .map(SystemId)
        .find(|system_id| !builder.stations_in_system(*system_id).is_empty())
        .expect("fixture should contain a system with stations");
    let selected_station_id = builder.stations_in_system(selected_system_id)[0];
    builder.with_ship_patch(
        ShipId(0),
        ShipPatch {
            location: Some(selected_system_id),
            current_station: Some(Some(selected_station_id)),
            current_target: Some(None),
            ..ShipPatch::default()
        },
    );
    let sim = builder.build();

    let snapshot = build_hud_snapshot_v2(
        &sim,
        false,
        1,
        CameraMode::System(selected_system_id),
        selected_system_id,
        Some(selected_station_id),
        Some(selected_station_id),
        Commodity::Fuel,
        Some(selected_station_id),
        None,
        Some(ShipId(0)),
        ContractsFilterState::default(),
        &UiKpiTracker::default(),
    );
    let system_panel = snapshot
        .system_panel
        .as_ref()
        .expect("system panel snapshot should be present");

    assert_eq!(system_panel.system_id, selected_system_id);
    assert!(!system_panel.system_name.is_empty());
    assert!(!system_panel.owner_faction_name.is_empty());
    assert!(system_panel.station_count > 0);
    assert_eq!(system_panel.station_count, system_panel.stations.len());
    assert!(!system_panel.stations[0].station_name.is_empty());
    assert!(!system_panel.stations[0].orbit_label.is_empty());
    assert_eq!(system_panel.ship_count, system_panel.ships.len());
    assert!(!system_panel.ships.is_empty());
    assert!(system_panel
        .ships
        .iter()
        .all(|ship| ship.system_id == selected_system_id));
    assert!(!system_panel.ships[0].status_text.is_empty());
}

#[test]
fn systems_snapshot_lists_all_systems_sorted_by_stock_coverage() {
    let sim = Simulation::new(RuntimeConfig::default(), 42);
    let selected_station_id = sim
        .camera_topology_view()
        .systems
        .iter()
        .find(|system| system.system_id == SystemId(0))
        .and_then(|system| system.stations.first().map(|station| station.station_id));

    let snapshot = build_hud_snapshot_v2(
        &sim,
        false,
        1,
        CameraMode::Galaxy,
        SystemId(0),
        selected_station_id,
        selected_station_id,
        Commodity::Fuel,
        selected_station_id,
        None,
        Some(ShipId(0)),
        ContractsFilterState::default(),
        &UiKpiTracker::default(),
    );

    assert_eq!(
        snapshot.systems_list_rows.len(),
        sim.camera_topology_view().systems.len()
    );

    for row in &snapshot.systems_list_rows {
        let details = sim
            .system_details_view(row.system_id)
            .expect("systems list row should resolve system details");
        let topology_system = sim
            .camera_topology_view()
            .systems
            .into_iter()
            .find(|system| system.system_id == row.system_id)
            .expect("systems list row should exist in topology");

        assert_eq!(row.system_name, expected_system_name(row.system_id));
        assert_eq!(row.owner_faction_name, details.owner_faction_name);
        assert_eq!(row.owner_faction_color_rgb, details.faction_color_rgb);
        assert_eq!(row.station_count, topology_system.stations.len());
        assert_eq!(row.ship_count, details.ships.len());
        assert_eq!(row.outgoing_gate_count, details.outgoing_gate_count);
        assert_eq!(row.stock_coverage, details.stock_coverage);
    }

    let mut expected = snapshot.systems_list_rows.clone();
    expected.sort_by(|left, right| {
        right
            .stock_coverage
            .total_cmp(&left.stock_coverage)
            .then_with(|| left.system_name.cmp(&right.system_name))
            .then_with(|| left.system_id.0.cmp(&right.system_id.0))
    });
    assert_eq!(snapshot.systems_list_rows, expected);
}

#[test]
fn station_card_snapshot_does_not_override_markets_detail_selection() {
    let mut builder = SimulationScenarioBuilder::stage_a(44);
    let stations = builder.stations_in_system(SystemId(0));
    if stations.len() < 2 {
        return;
    }
    let market_station = stations[1];
    let card_station = stations[0];

    builder.with_market_state_patch(
        market_station,
        Commodity::Fuel,
        MarketStatePatch {
            price: Some(31.0),
            stock: Some(75.0),
            ..MarketStatePatch::default()
        },
    );
    builder.with_market_state_patch(
        card_station,
        Commodity::Fuel,
        MarketStatePatch {
            price: Some(9.0),
            stock: Some(18.0),
            ..MarketStatePatch::default()
        },
    );
    let sim = builder.build();

    let snapshot = build_hud_snapshot_v2(
        &sim,
        false,
        1,
        CameraMode::System(SystemId(0)),
        SystemId(0),
        Some(market_station),
        Some(market_station),
        Commodity::Fuel,
        Some(card_station),
        None,
        Some(ShipId(0)),
        ContractsFilterState::default(),
        &UiKpiTracker::default(),
    );

    let market_row = snapshot
        .markets
        .station_detail
        .as_ref()
        .expect("station detail should exist")
        .commodity_rows
        .iter()
        .find(|row| row.commodity == Commodity::Fuel)
        .expect("fuel market row should exist");
    assert_eq!(
        snapshot
            .markets
            .station_detail
            .as_ref()
            .map(|detail| detail.station_id),
        Some(market_station)
    );
    assert!((market_row.local_price - 31.0).abs() < 1e-9);
    assert_eq!(
        snapshot.station_card.as_ref().map(|card| card.station_id),
        Some(card_station)
    );
}

#[test]
fn markets_snapshot_hotspots_follow_focused_commodity() {
    let mut builder = SimulationScenarioBuilder::stage_a(46);
    let system0 = builder.stations_in_system(SystemId(0));
    let system1 = builder.stations_in_system(SystemId(1));
    if system0.len() < 2 || system1.len() < 2 {
        return;
    }
    let cheap_station = system0[0];
    let expensive_station = system1[1];

    builder.with_market_state_patch(
        cheap_station,
        Commodity::Fuel,
        MarketStatePatch {
            price: Some(7.0),
            ..MarketStatePatch::default()
        },
    );
    builder.with_market_state_patch(
        expensive_station,
        Commodity::Fuel,
        MarketStatePatch {
            price: Some(39.0),
            ..MarketStatePatch::default()
        },
    );
    let sim = builder.build();

    let snapshot = build_hud_snapshot_v2(
        &sim,
        true,
        1,
        CameraMode::Galaxy,
        SystemId(0),
        None,
        Some(cheap_station),
        Commodity::Fuel,
        None,
        None,
        None,
        ContractsFilterState::default(),
        &UiKpiTracker::default(),
    );

    assert_eq!(snapshot.markets.focused_commodity, Commodity::Fuel);
    assert_eq!(
        snapshot.markets.hotspots.cheapest_stations[0].station_id,
        cheap_station
    );
    assert_eq!(
        snapshot.markets.hotspots.priciest_stations[0].station_id,
        expensive_station
    );
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
