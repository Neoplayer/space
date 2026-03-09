use bevy_egui::egui;
use gatebound_domain::{ShipId, StationId};
use gatebound_sim::Simulation;

use crate::features::stations::{open_station_card, StationUiState};
use crate::input::camera::{CameraMode, CameraUiState};
use crate::runtime::save::{toggle_save_menu_with_storage, SaveMenuState, SaveStorage};
use crate::runtime::sim::{
    apply_panel_toggle, panel_button_specs, panel_is_open, preferred_trade_commodity,
    set_time_speed, toggle_pause, SelectedShip, SelectedStation, SimClock, UiKpiTracker,
    UiPanelState,
};

use super::messages::HudMessages;
use super::snapshot::HudSnapshot;

pub(crate) fn sync_station_panel_toggle(
    simulation: &Simulation,
    station_panel_open: bool,
    selected_station_id: Option<StationId>,
    selected_ship_id: Option<ShipId>,
    default_player_ship_id: Option<ShipId>,
    station_ui: &mut StationUiState,
) -> Option<StationId> {
    station_ui.station_panel_open = station_panel_open;
    if !station_panel_open {
        return None;
    }

    let station_id = selected_station_id.or(station_ui.card_station_id)?;
    let preferred = preferred_trade_commodity(
        simulation,
        selected_ship_id.or(default_player_ship_id),
        station_id,
        station_ui.trade_commodity,
    );
    open_station_card(station_ui, station_id, Some(preferred));
    Some(station_id)
}

pub(super) struct TopHudAccess<'a> {
    pub save_storage: &'a SaveStorage,
    pub save_menu: &'a mut SaveMenuState,
    pub clock: &'a mut SimClock,
    pub camera: &'a mut CameraUiState,
    pub kpi: &'a mut UiKpiTracker,
}

pub(super) fn render_top_bar(
    ctx: &egui::Context,
    snapshot: &HudSnapshot,
    current_tick: u64,
    access: TopHudAccess<'_>,
) {
    let TopHudAccess {
        save_storage,
        save_menu,
        clock,
        camera,
        kpi,
    } = access;

    let save_menu_open = save_menu.open;
    egui::TopBottomPanel::top("gatebound_top_panel").show(ctx, |ui| {
        ui.columns(2, |columns| {
            columns[0].horizontal_wrapped(|ui| {
                let mut menu_button = egui::Button::new("Menu (Esc)");
                if save_menu_open {
                    menu_button = menu_button.fill(ui.visuals().selection.bg_fill);
                }
                if ui.add(menu_button).clicked() {
                    toggle_save_menu_with_storage(save_menu, clock, save_storage);
                    kpi.record_manual_action(current_tick);
                }
                if matches!(camera.mode, CameraMode::System(_)) {
                    if ui
                        .add_enabled(!save_menu_open, egui::Button::new("Galaxy View"))
                        .clicked()
                    {
                        camera.mode = CameraMode::Galaxy;
                        kpi.record_manual_action(current_tick);
                    }
                    ui.separator();
                }
                ui.label(format!("View: {}", snapshot.camera_mode));
                ui.separator();
                ui.label(format!("Time: {}", snapshot.time_label));
                ui.separator();
                ui.add_enabled_ui(!save_menu_open, |ui| {
                    let pause_label = if snapshot.paused { "Resume" } else { "Pause" };
                    let mut pause_button = egui::Button::new(pause_label);
                    if snapshot.paused {
                        pause_button = pause_button.fill(ui.visuals().selection.bg_fill);
                    }
                    if ui.add(pause_button).clicked() {
                        toggle_pause(clock);
                        kpi.record_manual_action(current_tick);
                    }

                    for speed in [1_u32, 2, 4] {
                        let mut speed_button = egui::Button::new(format!("{speed}x"));
                        if snapshot.speed_multiplier == speed {
                            speed_button = speed_button.fill(ui.visuals().selection.bg_fill);
                        }
                        if ui.add(speed_button).clicked() {
                            set_time_speed(clock, speed);
                            kpi.record_manual_action(current_tick);
                        }
                    }
                });
            });
            columns[1].with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.strong(format!("Balance: {:.1}", snapshot.capital));
            });
        });
    });
}

pub(super) struct LeftHudAccess<'a> {
    pub simulation: &'a Simulation,
    pub selected_station: &'a SelectedStation,
    pub selected_ship: &'a SelectedShip,
    pub panels: &'a mut UiPanelState,
    pub station_ui: &'a mut StationUiState,
    pub kpi: &'a mut UiKpiTracker,
    pub messages: &'a HudMessages,
}

pub(super) fn render_left_sidebar(
    ctx: &egui::Context,
    snapshot: &HudSnapshot,
    save_menu_open: bool,
    access: LeftHudAccess<'_>,
) {
    let LeftHudAccess {
        simulation,
        selected_station,
        selected_ship,
        panels,
        station_ui,
        kpi,
        messages,
    } = access;

    egui::SidePanel::left("gatebound_left_hud")
        .resizable(true)
        .show(ctx, |ui| {
            ui.heading("Windows");
            ui.add_enabled_ui(!save_menu_open, |ui| {
                for button in panel_button_specs() {
                    let open = panel_is_open(panels, button.index);
                    let label = format!("{} ({})", button.label, button.hotkey);
                    if ui.selectable_label(open, label).clicked() {
                        apply_panel_toggle(panels, button.index);
                        if button.index == 6 {
                            sync_station_panel_toggle(
                                simulation,
                                panels.station_ops,
                                selected_station.station_id,
                                selected_ship.ship_id,
                                snapshot.default_player_ship_id,
                                station_ui,
                            );
                        }
                        kpi.record_manual_action(simulation.tick());
                    }
                }
            });

            if !messages.entries.is_empty() {
                ui.separator();
                ui.heading("Events");
                for message in messages.entries.iter().rev() {
                    ui.monospace(message);
                }
            }
        });
}
