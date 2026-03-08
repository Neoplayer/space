use gatebound_domain::{CargoLoad, CargoManifest, CargoSource, Commodity, ShipId, SystemId};
use gatebound_sim::test_support::{MarketStatePatch, ShipPatch, SimulationScenarioBuilder};
use gatebound_sim::PopulationTrend;

use crate::input::camera::CameraMode;
use crate::runtime::sim::{
    apply_panel_toggle, panel_button_specs, MissionModalSelection, MissionsPanelState,
    UiKpiTracker, UiPanelState,
};
use crate::ui::hud::{
    build_hud_snapshot, build_ship_card_snapshot_for_ui, build_station_card_snapshot_for_ui,
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
