use bevy_egui::egui;
use gatebound_domain::{ShipId, StationId};
use gatebound_sim::Simulation;

use crate::features::ships::ShipUiState;
use crate::features::stations::StationUiState;
use crate::input::camera::{CameraMode, CameraUiState};
use crate::runtime::sim::{open_system_view, SelectedStation, UiKpiTracker, UiPanelState};

use super::ships::render_system_ships;
use super::snapshot::{SystemPanelSnapshot, SystemsListRowSnapshot};
use super::stations::render_system_stations;

pub(super) struct SystemPanelHudAccess<'a> {
    pub simulation: &'a Simulation,
    pub current_station_id: Option<StationId>,
    pub current_ship_id: Option<ShipId>,
    pub preferred_ship_id: Option<ShipId>,
    pub selected_station: &'a mut SelectedStation,
    pub panels: &'a mut UiPanelState,
    pub station_ui: &'a mut StationUiState,
    pub ship_ui: &'a mut ShipUiState,
    pub kpi: &'a mut UiKpiTracker,
}

pub(super) fn render_system_side_panel(
    ctx: &egui::Context,
    save_menu_open: bool,
    system_panel: Option<&SystemPanelSnapshot>,
    access: SystemPanelHudAccess<'_>,
) {
    if save_menu_open {
        return;
    }

    let Some(system_panel) = system_panel else {
        return;
    };

    let SystemPanelHudAccess {
        simulation,
        current_station_id,
        current_ship_id,
        preferred_ship_id,
        selected_station,
        panels,
        station_ui,
        ship_ui,
        kpi,
    } = access;

    let current_tick = simulation.tick();
    egui::SidePanel::right("gatebound_system_hud")
        .resizable(false)
        .default_width(360.0)
        .show(ctx, |ui| {
            render_system_panel_header(ui, system_panel);
            ui.separator();
            egui::ScrollArea::vertical()
                .id_salt("system_panel_scroll")
                .show(ui, |ui| {
                    render_system_overview(ui, system_panel);
                    ui.add_space(10.0);
                    render_system_stations(
                        ui,
                        system_panel,
                        preferred_ship_id,
                        current_station_id,
                        simulation,
                        selected_station,
                        panels,
                        station_ui,
                        kpi,
                    );
                    ui.add_space(10.0);
                    render_system_ships(
                        ui,
                        system_panel,
                        current_ship_id,
                        current_tick,
                        ship_ui,
                        kpi,
                    );
                });
        });
}

pub(super) fn render_systems_window(
    ctx: &egui::Context,
    save_menu_open: bool,
    open: &mut bool,
    systems_list_rows: &[SystemsListRowSnapshot],
    camera: &mut CameraUiState,
    kpi: &mut UiKpiTracker,
    current_tick: u64,
) {
    if save_menu_open || !*open {
        return;
    }

    let current_mode = camera.mode;
    egui::Window::new("Systems")
        .default_width(520.0)
        .default_height(480.0)
        .open(open)
        .show(ctx, |ui| {
            ui.label(format!("Systems: {}", systems_list_rows.len()));
            ui.separator();

            if systems_list_rows.is_empty() {
                ui.label("No systems available");
                return;
            }

            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for row in systems_list_rows {
                        render_systems_list_row(ui, row, current_mode, camera, kpi, current_tick);
                        ui.add_space(6.0);
                    }
                });
        });
}

fn render_system_panel_header(ui: &mut egui::Ui, panel: &SystemPanelSnapshot) {
    egui::Frame::group(ui.style())
        .fill(egui::Color32::from_rgb(14, 20, 27))
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(58, 78, 94)))
        .show(ui, |ui| {
            ui.heading(&panel.system_name);
            ui.horizontal_wrapped(|ui| {
                ui.monospace(format!("System #{}", panel.system_id.0));
                ui.separator();
                ui.colored_label(
                    egui::Color32::from_rgb(
                        panel.owner_faction_color_rgb[0],
                        panel.owner_faction_color_rgb[1],
                        panel.owner_faction_color_rgb[2],
                    ),
                    &panel.owner_faction_name,
                );
            });
        });
}

fn render_systems_list_row(
    ui: &mut egui::Ui,
    row: &SystemsListRowSnapshot,
    current_mode: CameraMode,
    camera: &mut CameraUiState,
    kpi: &mut UiKpiTracker,
    current_tick: u64,
) {
    let is_open =
        matches!(current_mode, CameraMode::System(system_id) if system_id == row.system_id);
    let fill = if is_open {
        egui::Color32::from_rgb(24, 35, 44)
    } else {
        egui::Color32::from_rgb(16, 21, 28)
    };

    egui::Frame::group(ui.style())
        .fill(fill)
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(56, 72, 88)))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new(&row.system_name).strong());
                    ui.colored_label(
                        egui::Color32::from_rgb(
                            row.owner_faction_color_rgb[0],
                            row.owner_faction_color_rgb[1],
                            row.owner_faction_color_rgb[2],
                        ),
                        &row.owner_faction_name,
                    );
                    ui.label(format!(
                        "Stations {}  Ships {}  Gates {}",
                        row.station_count, row.ship_count, row.outgoing_gate_count
                    ));
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Open system").clicked() {
                        open_system_view(&mut camera.mode, row.system_id);
                        kpi.record_manual_action(current_tick);
                    }
                });
            });
        });
}

fn render_system_overview(ui: &mut egui::Ui, panel: &SystemPanelSnapshot) {
    egui::Frame::group(ui.style())
        .fill(egui::Color32::from_rgb(18, 25, 32))
        .show(ui, |ui| {
            ui.heading("Overview");
            egui::Grid::new("system_overview_grid")
                .num_columns(2)
                .spacing([16.0, 6.0])
                .show(ui, |ui| {
                    ui.monospace("Stations");
                    ui.monospace(panel.station_count.to_string());
                    ui.end_row();
                    ui.monospace("Ships");
                    ui.monospace(panel.ship_count.to_string());
                    ui.end_row();
                    ui.monospace("Gates");
                    ui.monospace(panel.outgoing_gate_count.to_string());
                    ui.end_row();
                    ui.monospace("Dock cap");
                    ui.monospace(format!("{:.1}", panel.dock_capacity));
                    ui.end_row();
                    ui.monospace("Price idx");
                    ui.monospace(format!("{:.2}", panel.avg_price_index));
                    ui.end_row();
                    ui.monospace("Stock cov");
                    ui.monospace(format!("{:.2}", panel.stock_coverage));
                    ui.end_row();
                    ui.monospace("Net flow");
                    ui.monospace(format!("{:.1}", panel.net_flow));
                    ui.end_row();
                    ui.monospace("Congestion");
                    ui.monospace(format!("{:.2}", panel.congestion));
                    ui.end_row();
                    ui.monospace("Fuel stress");
                    ui.monospace(format!("{:.2}", panel.fuel_stress));
                    ui.end_row();
                    ui.monospace("Stress");
                    ui.monospace(format!("{:.2}", panel.stress_score));
                    ui.end_row();
                });
        });
}
