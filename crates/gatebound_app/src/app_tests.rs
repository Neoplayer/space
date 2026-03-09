use bevy::prelude::*;
use gatebound_domain::{
    CargoLoad, CargoManifest, CargoSource, Commodity, RuntimeConfig, ShipId, SystemId,
};
use gatebound_sim::test_support::{MarketStatePatch, ShipPatch, SimulationScenarioBuilder};
use gatebound_sim::PopulationTrend;

use crate::app_shell::GateboundAppShellPlugin;
use crate::features::finance::FinanceUiState;
use crate::features::markets::{seed_markets_ui_state, MarketsUiState};
use crate::features::missions::{
    open_active_mission, open_mission_offer, MissionModalSelection, MissionsPanelState,
};
use crate::features::ships::{apply_ship_context_open, open_ship_card, ShipCardTab, ShipUiState};
use crate::features::stations::{
    apply_station_context_open, open_station_card, StationCardTab, StationUiState,
};
use crate::input::camera::CameraMode;
use crate::input::camera::CameraUiState;
use crate::render::world::ShipMotionCache;
use crate::runtime::save::{SaveMenuState, SaveStorage};
use crate::runtime::sim::{
    apply_panel_toggle, panel_button_specs, SelectedShip, SelectedStation, SelectedSystem,
    SimClock, SimResource, TrackedShip, UiKpiTracker, UiPanelState,
};
use crate::ui::hud::{
    build_hud_snapshot, build_ship_card_snapshot_for_ui, build_station_card_snapshot_for_ui,
    HudMessages,
};

fn first_station_in_system(
    builder: &SimulationScenarioBuilder,
    system_id: SystemId,
) -> gatebound_domain::StationId {
    builder
        .first_station_in_system(system_id)
        .expect("station should exist")
}

fn player_ship_id(builder: &SimulationScenarioBuilder) -> ShipId {
    builder.player_ship_id().expect("player ship should exist")
}

#[test]
fn app_shell_plugin_registers_core_resources() {
    let mut app = App::new();
    app.add_plugins(GateboundAppShellPlugin::new(RuntimeConfig::default(), 77));

    assert!(app.world().contains_resource::<SimResource>());
    assert!(app.world().contains_resource::<SimClock>());
    assert!(app.world().contains_resource::<CameraUiState>());
    assert!(app.world().contains_resource::<ShipMotionCache>());
    assert!(app.world().contains_resource::<UiPanelState>());
    assert!(app.world().contains_resource::<MissionsPanelState>());
    assert!(app.world().contains_resource::<SelectedShip>());
    assert!(app.world().contains_resource::<SelectedSystem>());
    assert!(app.world().contains_resource::<SelectedStation>());
    assert!(app.world().contains_resource::<FinanceUiState>());
    assert!(app.world().contains_resource::<MarketsUiState>());
    assert!(app.world().contains_resource::<TrackedShip>());
    assert!(app.world().contains_resource::<ShipUiState>());
    assert!(app.world().contains_resource::<StationUiState>());
    assert!(app.world().contains_resource::<UiKpiTracker>());
    assert!(app.world().contains_resource::<HudMessages>());
    assert!(app.world().contains_resource::<SaveStorage>());
    assert!(app.world().contains_resource::<SaveMenuState>());
    assert_eq!(app.world().resource::<SimClock>().speed_multiplier, 1);
    assert_eq!(
        app.world().resource::<CameraUiState>().mode,
        CameraMode::Galaxy
    );
    assert!(!app.world().resource::<UiPanelState>().missions);
}

#[test]
fn mission_feature_actions_update_state() {
    let mut state = MissionsPanelState::default();

    open_mission_offer(&mut state, 42);
    assert_eq!(
        state.modal_selection,
        Some(MissionModalSelection::Offer(42))
    );

    open_active_mission(&mut state, gatebound_domain::MissionId(7));
    assert_eq!(
        state.selected_mission_id,
        Some(gatebound_domain::MissionId(7))
    );
    assert_eq!(
        state.modal_selection,
        Some(MissionModalSelection::Active(gatebound_domain::MissionId(
            7
        )))
    );
}

#[test]
fn station_feature_open_station_card_updates_station_ui() {
    let mut state = StationUiState::default();

    open_station_card(
        &mut state,
        gatebound_domain::StationId(9),
        Some(Commodity::Ore),
    );

    assert!(state.station_panel_open);
    assert_eq!(state.card_station_id, Some(gatebound_domain::StationId(9)));
    assert_eq!(
        state.context_station_id,
        Some(gatebound_domain::StationId(9))
    );
    assert_eq!(state.card_tab, StationCardTab::Info);
    assert_eq!(state.trade_commodity, Commodity::Ore);
    assert_eq!(state.storage_commodity, Commodity::Ore);
}

#[test]
fn station_feature_context_open_sets_context_menu() {
    let mut state = StationUiState::default();

    apply_station_context_open(&mut state, gatebound_domain::StationId(12));

    assert_eq!(
        state.context_station_id,
        Some(gatebound_domain::StationId(12))
    );
    assert!(state.context_menu_open);
}

#[test]
fn station_hud_system_selection_updates_runtime_and_station_ui() {
    let mut selected_station = SelectedStation::default();
    let mut panels = UiPanelState::default();
    let mut station_ui = StationUiState::default();

    crate::ui::hud::open_system_station_panel(
        &mut selected_station,
        &mut panels,
        &mut station_ui,
        gatebound_domain::StationId(14),
        Some(Commodity::Electronics),
    );

    assert_eq!(
        selected_station.station_id,
        Some(gatebound_domain::StationId(14))
    );
    assert!(panels.station_ops);
    assert!(station_ui.station_panel_open);
    assert_eq!(
        station_ui.card_station_id,
        Some(gatebound_domain::StationId(14))
    );
    assert_eq!(station_ui.card_tab, StationCardTab::Info);
    assert_eq!(station_ui.trade_commodity, Commodity::Electronics);
    assert_eq!(station_ui.storage_commodity, Commodity::Electronics);
}

#[test]
fn station_sidebar_toggle_opens_selected_station_card() {
    let builder = SimulationScenarioBuilder::stage_a(713);
    let station_id = first_station_in_system(&builder, SystemId(0));
    let ship_id = player_ship_id(&builder);
    let sim = builder.build();
    let mut station_ui = StationUiState::default();

    assert_eq!(
        crate::ui::hud::sync_station_panel_toggle(
            &sim,
            true,
            Some(station_id),
            None,
            Some(ship_id),
            &mut station_ui,
        ),
        Some(station_id)
    );
    assert!(station_ui.station_panel_open);
    assert_eq!(station_ui.card_station_id, Some(station_id));
}

#[test]
fn station_sidebar_toggle_closes_station_panel_without_resetting_selection() {
    let builder = SimulationScenarioBuilder::stage_a(714);
    let station_id = first_station_in_system(&builder, SystemId(0));
    let sim = builder.build();
    let mut station_ui = StationUiState {
        station_panel_open: true,
        card_station_id: Some(station_id),
        ..StationUiState::default()
    };

    assert_eq!(
        crate::ui::hud::sync_station_panel_toggle(
            &sim,
            false,
            Some(station_id),
            None,
            None,
            &mut station_ui,
        ),
        None
    );
    assert!(!station_ui.station_panel_open);
    assert_eq!(station_ui.card_station_id, Some(station_id));
}

#[test]
fn ship_feature_open_ship_card_updates_ship_ui() {
    let mut state = ShipUiState::default();

    open_ship_card(&mut state, gatebound_domain::ShipId(5));

    assert!(state.card_open);
    assert_eq!(state.card_ship_id, Some(gatebound_domain::ShipId(5)));
    assert_eq!(state.context_ship_id, Some(gatebound_domain::ShipId(5)));
    assert_eq!(state.card_tab, ShipCardTab::Overview);
}

#[test]
fn ship_feature_context_open_sets_context_menu() {
    let mut state = ShipUiState::default();

    apply_ship_context_open(&mut state, gatebound_domain::ShipId(17));

    assert_eq!(state.context_ship_id, Some(gatebound_domain::ShipId(17)));
    assert!(state.context_menu_open);
}

#[test]
fn ship_feature_system_selection_opens_card() {
    let mut ship_ui = ShipUiState::default();

    crate::features::ships::open_system_ship_inspector_selection(
        &mut ship_ui,
        gatebound_domain::ShipId(23),
    );

    assert!(ship_ui.card_open);
    assert_eq!(ship_ui.card_ship_id, Some(gatebound_domain::ShipId(23)));
}

#[test]
fn markets_feature_seed_prefers_selected_station() {
    let builder = SimulationScenarioBuilder::stage_a(411);
    let station_id = first_station_in_system(&builder, SystemId(0));
    let sim = builder.build();
    let mut state = MarketsUiState::default();

    seed_markets_ui_state(&mut state, &sim, SystemId(0), Some(station_id));

    assert_eq!(state.detail_station_id, Some(station_id));
    assert!(state.seeded_from_world_selection);
    assert_eq!(state.focused_commodity, Commodity::Fuel);
}

#[test]
fn markets_feature_seed_preserves_existing_seeded_selection() {
    let builder = SimulationScenarioBuilder::stage_a(412);
    let current_station = first_station_in_system(&builder, SystemId(1));
    let incoming_station = first_station_in_system(&builder, SystemId(0));
    let sim = builder.build();
    let mut state = MarketsUiState {
        detail_station_id: Some(current_station),
        focused_commodity: Commodity::Ore,
        seeded_from_world_selection: true,
    };

    seed_markets_ui_state(&mut state, &sim, SystemId(0), Some(incoming_station));

    assert_eq!(state.detail_station_id, Some(current_station));
    assert_eq!(state.focused_commodity, Commodity::Ore);
    assert!(state.seeded_from_world_selection);
}

#[test]
fn finance_feature_defaults_remain_stable() {
    let state = FinanceUiState::default();

    assert_eq!(state.pending_offer, None);
    assert_eq!(state.repayment_amount, 25.0);
}

#[test]
fn policies_hud_resolves_selected_ship_before_default() {
    assert_eq!(
        crate::ui::hud::resolve_policy_ship_id(
            Some(gatebound_domain::ShipId(5)),
            Some(gatebound_domain::ShipId(9)),
        ),
        Some(gatebound_domain::ShipId(5))
    );
}

#[test]
fn policies_hud_falls_back_to_default_ship() {
    assert_eq!(
        crate::ui::hud::resolve_policy_ship_id(None, Some(gatebound_domain::ShipId(9))),
        Some(gatebound_domain::ShipId(9))
    );
    assert_eq!(crate::ui::hud::resolve_policy_ship_id(None, None), None);
}

#[test]
fn station_context_hud_resolves_selected_ship_before_default() {
    assert_eq!(
        crate::ui::hud::resolve_station_context_ship_id(
            Some(gatebound_domain::ShipId(7)),
            Some(gatebound_domain::ShipId(11)),
        ),
        Some(gatebound_domain::ShipId(7))
    );
}

#[test]
fn station_context_hud_falls_back_to_default_ship() {
    assert_eq!(
        crate::ui::hud::resolve_station_context_ship_id(None, Some(gatebound_domain::ShipId(11))),
        Some(gatebound_domain::ShipId(11))
    );
    assert_eq!(
        crate::ui::hud::resolve_station_context_ship_id(None, None),
        None
    );
}

#[test]
fn missions_panel_uses_f1_slot() {
    let specs = panel_button_specs();
    assert_eq!(specs[0].label, "Missions");
    assert_eq!(specs[0].hotkey, "F1");

    let mut panels = UiPanelState::default();
    assert!(!panels.missions);
    apply_panel_toggle(&mut panels, 1);
    assert!(panels.missions);
    apply_panel_toggle(&mut panels, 1);
    assert!(!panels.missions);
}

#[test]
fn hud_snapshot_separates_station_offers_from_active_missions() {
    let mut builder = SimulationScenarioBuilder::stage_a(901);
    let ship_id = player_ship_id(&builder);
    let source_station = first_station_in_system(&builder, SystemId(0));
    let destination_station = first_station_in_system(&builder, SystemId(1));
    builder
        .with_market_state_patch(
            source_station,
            Commodity::Fuel,
            MarketStatePatch {
                stock: Some(180.0),
                ..MarketStatePatch::default()
            },
        )
        .with_market_state_patch(
            destination_station,
            Commodity::Fuel,
            MarketStatePatch {
                stock: Some(20.0),
                ..MarketStatePatch::default()
            },
        );
    let mut sim = builder.build();
    sim.refresh_mission_offers();

    let before = build_hud_snapshot(
        &sim,
        false,
        1,
        CameraMode::Galaxy,
        SystemId(0),
        Some(source_station),
        Some(source_station),
        Commodity::Fuel,
        Some(source_station),
        Some(ship_id),
        Some(ship_id),
        None,
        &UiKpiTracker::default(),
    );
    assert!(
        before.active_mission_rows.is_empty(),
        "before accepting, there should be no active mission rows"
    );
    let station_card = before
        .station_card
        .as_ref()
        .expect("station card snapshot should exist");
    let offer_row = station_card
        .missions
        .offers
        .iter()
        .find(|offer| {
            offer.summary.origin.station_id == source_station
                && offer.summary.destination.station_id == destination_station
                && offer.commodity == Commodity::Fuel
        })
        .expect("station offers should expose generated mission offers");
    assert!(offer_row.summary.summary_line.contains("->"));
    assert!(offer_row.summary.gate_jumps > 0);

    let mission_offer_id = offer_row.offer_id;
    let mission_id = sim
        .accept_mission_offer(mission_offer_id)
        .expect("mission offer should be accepted");

    let after = build_hud_snapshot(
        &sim,
        false,
        1,
        CameraMode::Galaxy,
        SystemId(0),
        Some(source_station),
        Some(source_station),
        Commodity::Fuel,
        Some(source_station),
        Some(ship_id),
        Some(ship_id),
        None,
        &UiKpiTracker::default(),
    );
    assert!(
        after
            .active_mission_rows
            .iter()
            .any(|detail| detail.mission_id == mission_id),
        "accepted mission should appear only in active mission rows"
    );
    assert_eq!(after.active_missions, 1);
}

#[test]
fn station_card_snapshot_only_lists_offer_rows() {
    let mut builder = SimulationScenarioBuilder::stage_a(902);
    let ship_id = player_ship_id(&builder);
    let source_station = first_station_in_system(&builder, SystemId(0));
    let destination_station = first_station_in_system(&builder, SystemId(1));
    builder
        .with_market_state_patch(
            source_station,
            Commodity::Fuel,
            MarketStatePatch {
                stock: Some(180.0),
                ..MarketStatePatch::default()
            },
        )
        .with_market_state_patch(
            destination_station,
            Commodity::Fuel,
            MarketStatePatch {
                stock: Some(20.0),
                ..MarketStatePatch::default()
            },
        )
        .dock_ship_at(ship_id, source_station);
    let mut sim = builder.build();
    sim.refresh_mission_offers();

    let card = build_station_card_snapshot_for_ui(&sim, ship_id, source_station)
        .expect("station card snapshot should exist");

    assert!(card.docked);
    assert!(
        card.missions.offers.iter().any(|row| {
            row.summary.origin.station_id == source_station
                && row.summary.destination.station_id == destination_station
                && row.summary.summary_line.contains("->")
        }),
        "station mission tab should expose only readable offer rows"
    );
}

#[test]
fn mission_modal_snapshot_switches_between_offer_and_active_actions() {
    let mut builder = SimulationScenarioBuilder::stage_a(903);
    let ship_id = player_ship_id(&builder);
    let source_station = first_station_in_system(&builder, SystemId(0));
    let destination_station = first_station_in_system(&builder, SystemId(1));
    builder
        .with_market_state_patch(
            source_station,
            Commodity::Fuel,
            MarketStatePatch {
                stock: Some(180.0),
                ..MarketStatePatch::default()
            },
        )
        .with_market_state_patch(
            destination_station,
            Commodity::Fuel,
            MarketStatePatch {
                stock: Some(20.0),
                ..MarketStatePatch::default()
            },
        )
        .dock_ship_at(ship_id, destination_station)
        .with_ship_patch(
            ship_id,
            ShipPatch {
                cargo_capacity: Some(1_000.0),
                cargo: Some(CargoManifest::from(CargoLoad {
                    commodity: Commodity::Fuel,
                    amount: 1_000.0,
                    source: CargoSource::Spot,
                })),
                ..ShipPatch::default()
            },
        );
    let mut sim = builder.build();
    sim.refresh_mission_offers();

    let offer = sim
        .missions_board_view()
        .offers
        .into_iter()
        .find(|offer| {
            offer.offer.origin_station == source_station
                && offer.offer.destination_station == destination_station
                && offer.offer.commodity == Commodity::Fuel
        })
        .expect("mission offer should exist");
    let offer_snapshot = build_hud_snapshot(
        &sim,
        false,
        1,
        CameraMode::Galaxy,
        SystemId(0),
        Some(source_station),
        Some(source_station),
        Commodity::Fuel,
        Some(source_station),
        Some(ship_id),
        Some(ship_id),
        Some(MissionModalSelection::Offer(offer.offer.id)),
        &UiKpiTracker::default(),
    );
    let offer_modal = offer_snapshot
        .mission_modal
        .as_ref()
        .expect("offer modal should exist");
    assert_eq!(
        offer_modal.selection,
        MissionModalSelection::Offer(offer.offer.id)
    );
    assert!(offer_modal.can_accept);
    assert!(!offer_modal.can_complete);
    assert!(!offer_modal.can_cancel);

    let mission_id = sim
        .accept_mission_offer(offer.offer.id)
        .expect("accepting mission should succeed");
    sim.player_unload_to_station_storage(
        ship_id,
        destination_station,
        Commodity::Fuel,
        offer.offer.quantity,
    )
    .expect("ordinary cargo should unload into destination storage");

    let active_snapshot = build_hud_snapshot(
        &sim,
        false,
        1,
        CameraMode::Galaxy,
        SystemId(1),
        Some(destination_station),
        Some(destination_station),
        Commodity::Fuel,
        Some(destination_station),
        Some(ship_id),
        Some(ship_id),
        Some(MissionModalSelection::Active(mission_id)),
        &UiKpiTracker::default(),
    );
    let active_modal = active_snapshot
        .mission_modal
        .as_ref()
        .expect("active mission modal should exist");
    assert_eq!(
        active_modal.selection,
        MissionModalSelection::Active(mission_id)
    );
    assert!(!active_modal.can_accept);
    assert!(active_modal.can_complete);
    assert!(active_modal.can_cancel);
    assert!(
        (active_modal.destination_storage_amount.unwrap_or_default() - offer.offer.quantity).abs()
            < 1e-9
    );
}

#[test]
fn hud_snapshot_exposes_population_across_station_system_and_market_views() {
    let mut builder = SimulationScenarioBuilder::stage_a(904);
    let ship_id = player_ship_id(&builder);
    let station_id = first_station_in_system(&builder, SystemId(0));
    builder.dock_ship_at(ship_id, station_id);
    let mut sim = builder.build();

    sim.step_cycle();

    let snapshot = build_hud_snapshot(
        &sim,
        false,
        1,
        CameraMode::System(SystemId(0)),
        SystemId(0),
        Some(station_id),
        Some(station_id),
        Commodity::Gas,
        Some(station_id),
        Some(ship_id),
        Some(ship_id),
        None,
        &UiKpiTracker::default(),
    );

    let card = snapshot
        .station_card
        .as_ref()
        .expect("station card snapshot should exist");
    assert!(card.population > 0.0);
    assert!(card.population_ratio > 1.0);
    assert_eq!(card.population_trend, PopulationTrend::Growing);

    let system_station = snapshot
        .system_panel
        .as_ref()
        .expect("system panel should exist")
        .stations
        .iter()
        .find(|station| station.station_id == station_id)
        .expect("system station snapshot should exist");
    assert!(system_station.population > 0.0);
    assert!(system_station.population_ratio > 1.0);
    assert_eq!(system_station.population_trend, PopulationTrend::Growing);

    let anomaly = snapshot
        .markets
        .station_anomaly_rows
        .iter()
        .find(|row| row.station_id == station_id)
        .expect("station anomaly snapshot should exist");
    assert!(anomaly.population > 0.0);
    assert!(anomaly.population_ratio > 1.0);
    assert_eq!(anomaly.population_trend, PopulationTrend::Growing);

    let detail = snapshot
        .markets
        .station_detail
        .as_ref()
        .expect("station detail snapshot should exist");
    assert_eq!(detail.station_id, station_id);
    assert!(detail.population > 0.0);
    assert!(detail.population_ratio > 1.0);
    assert_eq!(detail.population_trend, PopulationTrend::Growing);
}

#[test]
fn ship_card_snapshot_keeps_regular_cargo_only() {
    let mut builder = SimulationScenarioBuilder::stage_a(904);
    let ship_id = player_ship_id(&builder);
    let source_station = first_station_in_system(&builder, SystemId(0));
    let destination_station = first_station_in_system(&builder, SystemId(1));
    builder
        .with_market_state_patch(
            source_station,
            Commodity::Fuel,
            MarketStatePatch {
                stock: Some(180.0),
                ..MarketStatePatch::default()
            },
        )
        .with_market_state_patch(
            destination_station,
            Commodity::Fuel,
            MarketStatePatch {
                stock: Some(20.0),
                ..MarketStatePatch::default()
            },
        )
        .dock_ship_at(ship_id, source_station);
    let mut sim = builder.build();
    sim.refresh_mission_offers();

    let offer = sim
        .missions_board_view()
        .offers
        .into_iter()
        .find(|offer| {
            offer.offer.origin_station == source_station
                && offer.offer.destination_station == destination_station
                && offer.offer.commodity == Commodity::Fuel
        })
        .expect("mission offer should exist");
    sim.accept_mission_offer(offer.offer.id)
        .expect("accepting mission should succeed");
    sim.player_load_from_station_storage(ship_id, source_station, Commodity::Fuel, 6.0)
        .expect("storage load should succeed");

    let ship_card =
        build_ship_card_snapshot_for_ui(&sim, ship_id).expect("ship card snapshot should exist");
    assert!(
        ship_card
            .cargo_lots
            .iter()
            .any(|cargo| cargo.commodity == Commodity::Fuel),
        "ship cargo should stay visible as ordinary cargo"
    );
}

#[test]
fn missions_panel_state_defaults_to_no_selection() {
    let state = MissionsPanelState::default();
    assert!(state.selected_mission_id.is_none());
    assert!(state.modal_selection.is_none());
}
