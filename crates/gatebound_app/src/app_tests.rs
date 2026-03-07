use gatebound_domain::{Commodity, MissionKind, MissionStatus, ShipId, SystemId};
use gatebound_sim::test_support::{MarketStatePatch, SimulationScenarioBuilder};

use crate::input::camera::CameraMode;
use crate::runtime::sim::{
    apply_panel_toggle, panel_button_specs, MissionsPanelState, UiKpiTracker, UiPanelState,
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
fn hud_snapshot_surfaces_mission_offers_and_active_missions() {
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
        &UiKpiTracker::default(),
    );
    assert!(
        before.mission_offers.iter().any(|offer| {
            offer.offer.origin_station == source_station
                && offer.offer.destination_station == destination_station
                && offer.offer.commodity == Commodity::Fuel
        }),
        "hud snapshot should expose generated mission offers"
    );

    let mission_offer_id = before
        .mission_offers
        .iter()
        .find(|offer| {
            offer.offer.origin_station == source_station
                && offer.offer.destination_station == destination_station
                && offer.offer.commodity == Commodity::Fuel
        })
        .map(|offer| offer.offer.id)
        .expect("mission offer should exist");
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
        &UiKpiTracker::default(),
    );
    assert!(
        after
            .mission_details
            .iter()
            .any(|detail| detail.mission.id == mission_id),
        "accepted mission should appear in active mission details"
    );
    assert_eq!(after.active_missions, 1);
}

#[test]
fn station_card_snapshot_includes_mission_tab_payload() {
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

    let offer_id = sim
        .missions_board_view()
        .offers
        .iter()
        .find(|offer| {
            offer.offer.origin_station == source_station
                && offer.offer.destination_station == destination_station
        })
        .map(|offer| offer.offer.id)
        .expect("mission offer should exist");
    let mission_id = sim
        .accept_mission_offer(offer_id)
        .expect("accepting mission should succeed");

    let card = build_station_card_snapshot_for_ui(&sim, ship_id, source_station)
        .expect("station card snapshot should exist");

    assert!(card.docked);
    assert!(
        card.missions
            .mission_rows
            .iter()
            .any(|row| row.mission.id == mission_id && row.can_load),
        "station mission tab should expose accepted mission rows with load availability"
    );
}

#[test]
fn ship_card_snapshot_lists_loaded_mission_cargo() {
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
    let mission_id = sim
        .accept_mission_offer(offer.offer.id)
        .expect("accepting mission should succeed");
    sim.player_load_mission_cargo(ship_id, mission_id, 6.0)
        .expect("mission load should succeed");

    let ship_card =
        build_ship_card_snapshot_for_ui(&sim, ship_id).expect("ship card snapshot should exist");
    let mission_detail = ship_card
        .mission_cargo
        .iter()
        .find(|detail| detail.mission.id == mission_id)
        .expect("mission cargo detail should exist");

    assert_eq!(mission_detail.mission.kind, MissionKind::Transport);
    assert_eq!(mission_detail.mission.status, MissionStatus::InProgress);
    assert!((mission_detail.in_transit_amount - 6.0).abs() < 1e-9);
    assert!(
        ship_card
            .cargo_lots
            .iter()
            .any(|cargo| cargo.commodity == Commodity::Fuel),
        "ship cargo lots should include loaded mission cargo"
    );
}

#[test]
fn missions_panel_state_defaults_to_no_selection() {
    let state = MissionsPanelState::default();
    assert!(state.selected_mission_id.is_none());
}
