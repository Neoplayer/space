use bevy_egui::egui;
use gatebound_domain::ShipId;

use crate::features::ships::{open_ship_card, ShipUiState};
use crate::features::stations::{open_station_card, StationUiState};
use crate::input::camera::CameraUiState;
use crate::runtime::sim::{
    preferred_trade_commodity, track_ship, SelectedShip, SimResource, TrackedShip, UiKpiTracker,
    UiPanelState,
};

use super::labels::{
    command_error_label, company_archetype_label, ship_class_label, ship_role_label,
};
use super::messages::HudMessages;
use super::snapshot::HudSnapshot;

pub(crate) fn resolve_station_context_ship_id(
    selected_ship_id: Option<ShipId>,
    default_player_ship_id: Option<ShipId>,
) -> Option<ShipId> {
    selected_ship_id.or(default_player_ship_id)
}

pub(super) struct ContextHudAccess<'a> {
    pub sim: &'a mut SimResource,
    pub camera: &'a mut CameraUiState,
    pub selected_ship: &'a SelectedShip,
    pub tracked_ship: &'a mut TrackedShip,
    pub panels: &'a mut UiPanelState,
    pub ship_ui: &'a mut ShipUiState,
    pub station_ui: &'a mut StationUiState,
    pub kpi: &'a mut UiKpiTracker,
    pub messages: &'a mut HudMessages,
}

struct StationContextWindowAccess<'a> {
    sim: &'a mut SimResource,
    selected_ship: &'a SelectedShip,
    panels: &'a mut UiPanelState,
    station_ui: &'a mut StationUiState,
    kpi: &'a mut UiKpiTracker,
    messages: &'a mut HudMessages,
}

pub(super) fn render_context_windows(
    ctx: &egui::Context,
    snapshot: &HudSnapshot,
    save_menu_open: bool,
    access: ContextHudAccess<'_>,
) {
    if save_menu_open {
        return;
    }

    let ContextHudAccess {
        sim,
        camera,
        selected_ship,
        tracked_ship,
        panels,
        ship_ui,
        station_ui,
        kpi,
        messages,
    } = access;

    render_ship_context_window(ctx, sim, camera, tracked_ship, ship_ui, kpi, messages);
    render_station_context_window(
        ctx,
        snapshot,
        StationContextWindowAccess {
            sim,
            selected_ship,
            panels,
            station_ui,
            kpi,
            messages,
        },
    );
}

fn render_ship_context_window(
    ctx: &egui::Context,
    sim: &mut SimResource,
    camera: &mut CameraUiState,
    tracked_ship: &mut TrackedShip,
    ship_ui: &mut ShipUiState,
    kpi: &mut UiKpiTracker,
    messages: &mut HudMessages,
) {
    if !ship_ui.context_menu_open {
        return;
    }

    let mut open = ship_ui.context_menu_open;
    egui::Window::new("Ship Context")
        .open(&mut open)
        .resizable(false)
        .collapsible(false)
        .show(ctx, |ui| {
            let Some(ship_id) = ship_ui.context_ship_id else {
                ui.label("No ship selected");
                return;
            };
            let Some(ship) = sim.simulation.ship_card_view(ship_id) else {
                ui.label("Ship details unavailable");
                return;
            };

            ui.label(format!("Ship: {}", ship.ship_name));
            ui.label(format!("Class: {}", ship_class_label(ship.ship_class)));
            ui.label(format!(
                "Owner: {} ({})",
                ship.owner_name,
                company_archetype_label(ship.owner_archetype)
            ));
            ui.label(format!("Role: {}", ship_role_label(ship.role)));
            ui.label(format!("System: {}", ship.location.0));
            if ui.button("Track ship").clicked() {
                kpi.record_manual_action(sim.simulation.tick());
                if let Some(system_id) = track_ship(tracked_ship, camera, &sim.simulation, ship_id)
                {
                    messages.push(format!(
                        "Tracking ship {} in system {}",
                        ship_id.0, system_id.0
                    ));
                    ship_ui.context_menu_open = false;
                }
            }
            if ui.button("Open ship card").clicked() {
                open_ship_card(ship_ui, ship_id);
                ship_ui.context_menu_open = false;
            }
        });
    ship_ui.context_menu_open = open && ship_ui.context_menu_open;
}

fn render_station_context_window(
    ctx: &egui::Context,
    snapshot: &HudSnapshot,
    access: StationContextWindowAccess<'_>,
) {
    let StationContextWindowAccess {
        sim,
        selected_ship,
        panels,
        station_ui,
        kpi,
        messages,
    } = access;

    if !station_ui.context_menu_open {
        return;
    }

    let mut open = station_ui.context_menu_open;
    egui::Window::new("Station Context")
        .open(&mut open)
        .resizable(false)
        .collapsible(false)
        .show(ctx, |ui| {
            let Some(station_id) = station_ui.context_station_id else {
                ui.label("No station selected");
                return;
            };
            ui.label(format!("Station: {}", station_id.0));

            let Some(ship_id) = resolve_station_context_ship_id(
                selected_ship.ship_id,
                snapshot.default_player_ship_id,
            ) else {
                ui.label("No player ship available");
                return;
            };

            let docked = sim
                .simulation
                .station_ops_view(ship_id, station_id)
                .map(|view| view.docked)
                .unwrap_or(false);
            ui.label(format!("Ship #{} docked={}", ship_id.0, docked));
            if ui.button("Fly to station").clicked() {
                kpi.record_manual_action(sim.simulation.tick());
                match sim.simulation.command_fly_to_station(ship_id, station_id) {
                    Ok(()) => {
                        messages.push(format!(
                            "Command: ship {} fly to station {}",
                            ship_id.0, station_id.0
                        ));
                        station_ui.context_menu_open = false;
                    }
                    Err(err) => messages.push(format!(
                        "Fly command failed for ship {}: {}",
                        ship_id.0,
                        command_error_label(err)
                    )),
                }
            }
            if ui.button("Open station card").clicked() {
                let preferred = preferred_trade_commodity(
                    &sim.simulation,
                    Some(ship_id),
                    station_id,
                    station_ui.trade_commodity,
                );
                open_station_card(station_ui, station_id, Some(preferred));
                panels.station_ops = true;
                station_ui.context_menu_open = false;
            }
        });
    station_ui.context_menu_open = open && station_ui.context_menu_open;
}
