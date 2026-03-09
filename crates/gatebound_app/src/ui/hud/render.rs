use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy_egui::EguiContexts;

use crate::features::finance::FinanceUiState;
use crate::features::markets::{seed_markets_ui_state, MarketsUiState};
use crate::features::missions::MissionsPanelState;
use crate::features::ships::ShipUiState;
use crate::features::stations::StationUiState;
use crate::runtime::save::{SaveMenuState, SaveStorage};
use crate::runtime::sim::{
    SelectedShip, SelectedStation, SelectedSystem, SimClock, SimResource, TrackedShip,
    UiKpiTracker, UiPanelState,
};

use super::chrome::{render_left_sidebar, render_top_bar, LeftHudAccess, TopHudAccess};
use super::context::{render_context_windows, ContextHudAccess};
use super::corporations::render_corporations_window;
use super::finance::{render_finance_panel, FinancePanelAccess};
use super::fleet::render_fleet_window;
use super::markets::render_markets_window;
use super::messages::HudMessages;
use super::missions::{render_missions_windows, MissionHudAccess};
use super::policies::render_policies_window;
use super::save_menu::{render_save_menu, SaveMenuHudAccess};
use super::ships::render_ship_window;
use super::snapshot::{
    build_hud_snapshot, build_ship_card_snapshot_for_ui, build_station_card_snapshot_for_ui,
};
use super::stations::{render_station_window, StationHudAccess};
use super::systems::{render_system_side_panel, render_systems_window, SystemPanelHudAccess};

#[derive(SystemParam)]
pub struct HudUiState<'w> {
    selected_system: ResMut<'w, SelectedSystem>,
    selected_station: ResMut<'w, SelectedStation>,
    selected_ship: ResMut<'w, SelectedShip>,
    missions_panel: ResMut<'w, MissionsPanelState>,
    panels: ResMut<'w, UiPanelState>,
    kpi: ResMut<'w, UiKpiTracker>,
    messages: ResMut<'w, HudMessages>,
    tracked_ship: ResMut<'w, TrackedShip>,
    ship_ui: ResMut<'w, ShipUiState>,
    station_ui: ResMut<'w, StationUiState>,
    markets_ui: ResMut<'w, MarketsUiState>,
    finance_ui: ResMut<'w, FinanceUiState>,
    save_storage: Res<'w, SaveStorage>,
    save_menu: ResMut<'w, SaveMenuState>,
}

#[allow(clippy::too_many_arguments)]
pub fn draw_hud_panel(
    mut egui_contexts: EguiContexts,
    mut sim: ResMut<SimResource>,
    mut clock: ResMut<SimClock>,
    mut camera: ResMut<crate::input::camera::CameraUiState>,
    hud: HudUiState,
) -> Result {
    let HudUiState {
        mut selected_system,
        mut selected_station,
        mut selected_ship,
        mut missions_panel,
        mut panels,
        mut kpi,
        mut messages,
        mut tracked_ship,
        mut ship_ui,
        mut station_ui,
        mut markets_ui,
        mut finance_ui,
        save_storage,
        mut save_menu,
    } = hud;

    let selected_system_id = selected_system.system_id;
    if panels.markets {
        seed_markets_ui_state(
            &mut markets_ui,
            &sim.simulation,
            selected_system_id,
            selected_station.station_id,
        );
    }
    let snapshot = build_hud_snapshot(
        &sim.simulation,
        clock.paused,
        clock.speed_multiplier,
        camera.mode,
        selected_system_id,
        selected_station.station_id,
        markets_ui.detail_station_id,
        markets_ui.focused_commodity,
        station_ui
            .card_station_id
            .filter(|_| station_ui.station_panel_open)
            .or(selected_station.station_id),
        ship_ui.card_ship_id.filter(|_| ship_ui.card_open),
        selected_ship.ship_id,
        missions_panel.modal_selection,
        &kpi,
    );

    let ctx = egui_contexts.ctx_mut()?;
    let save_menu_open = save_menu.open;
    let current_tick = sim.simulation.tick();

    render_top_bar(
        ctx,
        &snapshot,
        current_tick,
        TopHudAccess {
            save_storage: &save_storage,
            save_menu: &mut save_menu,
            clock: &mut clock,
            camera: &mut camera,
            kpi: &mut kpi,
        },
    );

    render_left_sidebar(
        ctx,
        &snapshot,
        save_menu_open,
        LeftHudAccess {
            simulation: &sim.simulation,
            selected_station: &selected_station,
            selected_ship: &selected_ship,
            panels: &mut panels,
            station_ui: &mut station_ui,
            kpi: &mut kpi,
            messages: &messages,
        },
    );

    let current_station_id = selected_station.station_id;
    let current_ship_id = selected_ship.ship_id;
    let preferred_ship_id = selected_ship.ship_id.or(snapshot.default_player_ship_id);
    render_system_side_panel(
        ctx,
        save_menu_open,
        snapshot.system_panel.as_ref(),
        SystemPanelHudAccess {
            simulation: &sim.simulation,
            current_station_id,
            current_ship_id,
            preferred_ship_id,
            selected_station: &mut selected_station,
            panels: &mut panels,
            station_ui: &mut station_ui,
            ship_ui: &mut ship_ui,
            kpi: &mut kpi,
        },
    );

    let live_ship_card = ship_ui
        .card_ship_id
        .filter(|_| ship_ui.card_open)
        .and_then(|ship_id| build_ship_card_snapshot_for_ui(&sim.simulation, ship_id));
    let live_station_card = station_ui
        .card_station_id
        .filter(|_| panels.station_ops && station_ui.station_panel_open)
        .or(selected_station.station_id)
        .and_then(|station_id| {
            selected_ship
                .ship_id
                .or(snapshot.default_player_ship_id)
                .map(|ship_id| (ship_id, station_id))
        })
        .and_then(|(ship_id, station_id)| {
            build_station_card_snapshot_for_ui(&sim.simulation, ship_id, station_id)
        });

    render_context_windows(
        ctx,
        &snapshot,
        save_menu_open,
        ContextHudAccess {
            sim: &mut sim,
            camera: &mut camera,
            selected_ship: &selected_ship,
            tracked_ship: &mut tracked_ship,
            panels: &mut panels,
            ship_ui: &mut ship_ui,
            station_ui: &mut station_ui,
            kpi: &mut kpi,
            messages: &mut messages,
        },
    );

    render_missions_windows(
        ctx,
        &snapshot,
        save_menu_open,
        MissionHudAccess {
            sim: &mut sim,
            selected_ship: &selected_ship,
            panels: &mut panels,
            missions_panel: &mut missions_panel,
            kpi: &mut kpi,
            messages: &mut messages,
        },
    );

    render_fleet_window(
        ctx,
        save_menu_open,
        &mut panels.fleet,
        &snapshot.fleet_list_rows,
        &mut ship_ui,
    );

    render_systems_window(
        ctx,
        save_menu_open,
        &mut panels.systems,
        &snapshot.systems_list_rows,
        &mut camera,
        &mut kpi,
        current_tick,
    );

    render_markets_window(
        ctx,
        save_menu_open,
        &mut panels.markets,
        &snapshot.markets,
        &mut markets_ui,
        &mut kpi,
        current_tick,
    );

    render_ship_window(ctx, save_menu_open, &mut ship_ui, live_ship_card.as_ref());

    render_station_window(
        ctx,
        &snapshot,
        save_menu_open,
        live_station_card.as_ref(),
        StationHudAccess {
            sim: &mut sim,
            selected_ship: &mut selected_ship,
            panels: &mut panels,
            station_ui: &mut station_ui,
            missions_panel: &mut missions_panel,
            kpi: &mut kpi,
            messages: &mut messages,
        },
    );

    render_finance_panel(
        ctx,
        save_menu_open,
        &mut panels.assets,
        &snapshot,
        FinancePanelAccess {
            finance_ui: &mut finance_ui,
            sim: &mut sim,
            kpi: &mut kpi,
            messages: &mut messages,
        },
    );

    render_policies_window(
        ctx,
        &snapshot,
        save_menu_open,
        &mut panels,
        &selected_ship,
        &mut sim,
        &mut kpi,
    );

    render_corporations_window(
        ctx,
        save_menu_open,
        &mut panels.corporations,
        &snapshot.corporation_rows,
    );

    render_save_menu(
        ctx,
        &save_storage,
        &mut save_menu,
        SaveMenuHudAccess {
            sim: &mut sim,
            clock: &mut clock,
            camera: &mut camera,
            selected_system: &mut selected_system,
            selected_station: &mut selected_station,
            selected_ship: &mut selected_ship,
            panels: &mut panels,
            missions_panel: &mut missions_panel,
            tracked_ship: &mut tracked_ship,
            ship_ui: &mut ship_ui,
            station_ui: &mut station_ui,
            markets_ui: &mut markets_ui,
            finance_ui: &mut finance_ui,
            kpi: &mut kpi,
            messages: &mut messages,
        },
    );

    Ok(())
}
