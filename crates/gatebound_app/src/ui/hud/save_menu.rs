use bevy_egui::egui;

use crate::features::finance::FinanceUiState;
use crate::features::markets::MarketsUiState;
use crate::features::missions::MissionsPanelState;
use crate::features::ships::ShipUiState;
use crate::features::stations::StationUiState;
use crate::input::camera::CameraUiState;
use crate::runtime::save::{
    apply_loaded_simulation, format_save_timestamp, refresh_save_entries, toggle_save_menu,
    GameSaveSummary, PendingSaveAction, SaveMenuState, SaveStorage,
};
use crate::runtime::sim::{
    SelectedShip, SelectedStation, SelectedSystem, SimClock, SimResource, TrackedShip,
    UiKpiTracker, UiPanelState,
};

use super::messages::HudMessages;

pub(super) struct SaveMenuHudAccess<'a> {
    pub sim: &'a mut SimResource,
    pub clock: &'a mut SimClock,
    pub camera: &'a mut CameraUiState,
    pub selected_system: &'a mut SelectedSystem,
    pub selected_station: &'a mut SelectedStation,
    pub selected_ship: &'a mut SelectedShip,
    pub panels: &'a mut UiPanelState,
    pub missions_panel: &'a mut MissionsPanelState,
    pub tracked_ship: &'a mut TrackedShip,
    pub ship_ui: &'a mut ShipUiState,
    pub station_ui: &'a mut StationUiState,
    pub markets_ui: &'a mut MarketsUiState,
    pub finance_ui: &'a mut FinanceUiState,
    pub kpi: &'a mut UiKpiTracker,
    pub messages: &'a mut HudMessages,
}

pub(super) fn render_save_menu(
    ctx: &egui::Context,
    save_storage: &SaveStorage,
    save_menu: &mut SaveMenuState,
    access: SaveMenuHudAccess<'_>,
) {
    if !save_menu.open {
        return;
    }

    let SaveMenuHudAccess {
        sim,
        clock,
        camera,
        selected_system,
        selected_station,
        selected_ship,
        panels,
        missions_panel,
        tracked_ship,
        ship_ui,
        station_ui,
        markets_ui,
        finance_ui,
        kpi,
        messages,
    } = access;

    let mut open = save_menu.open;
    let entries = save_menu.entries.clone();
    egui::Window::new("Save / Load")
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_width(720.0)
        .default_height(460.0)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            ui.label("Create a new save, overwrite the selected slot, or load an existing one.");
            if let Some(error) = save_menu.last_error.as_ref() {
                ui.colored_label(egui::Color32::from_rgb(220, 96, 96), error);
            }

            ui.add_space(8.0);
            ui.columns(2, |columns| {
                columns[0].heading("Saves");
                columns[0].separator();
                if entries.is_empty() {
                    columns[0].label("No saves yet");
                } else {
                    egui::ScrollArea::vertical()
                        .id_salt("save_menu_entries")
                        .show(&mut columns[0], |ui| {
                            for entry in &entries {
                                let selected =
                                    save_menu.selected_entry_id.as_ref() == Some(&entry.id);
                                ui.group(|ui| {
                                    if ui.selectable_label(selected, &entry.display_name).clicked()
                                    {
                                        save_menu.selected_entry_id = Some(entry.id.clone());
                                        save_menu.pending_action = None;
                                    }
                                    ui.small(format!(
                                        "Saved: {}",
                                        format_save_timestamp(entry.saved_at_unix)
                                    ));
                                    ui.small(format!("World: {}", entry.world_time_label));
                                });
                                ui.add_space(4.0);
                            }
                        });
                }

                columns[1].heading("Details");
                columns[1].separator();
                let selected_summary =
                    selected_save_summary(&entries, save_menu.selected_entry_id.as_ref());
                if let Some(summary) = selected_summary.clone() {
                    columns[1].label(format!("Name: {}", summary.display_name));
                    columns[1].label(format!(
                        "Saved at: {}",
                        format_save_timestamp(summary.saved_at_unix)
                    ));
                    columns[1].label(format!("World time: {}", summary.world_time_label));
                    columns[1].label(format!("Capital: {:.1}", summary.capital));
                    columns[1].label(format!("Debt: {:.1}", summary.debt));
                    columns[1].label(format!("Reputation: {:.2}", summary.reputation));
                } else {
                    columns[1].label("Select a save slot to inspect it.");
                }

                columns[1].add_space(12.0);
                columns[1].horizontal_wrapped(|ui| {
                    if ui.button("Create New").clicked() {
                        match save_storage.create_new_save(&sim.simulation) {
                            Ok(summary) => {
                                refresh_save_entries(save_menu, save_storage);
                                save_menu.selected_entry_id = Some(summary.id.clone());
                                save_menu.pending_action = None;
                                save_menu.last_error = None;
                                messages.push(format!("Created save {}", summary.display_name));
                                kpi.record_manual_action(sim.simulation.tick());
                            }
                            Err(error) => save_menu.last_error = Some(error.to_string()),
                        }
                    }

                    let has_selection = selected_summary.is_some();
                    if ui
                        .add_enabled(has_selection, egui::Button::new("Overwrite"))
                        .clicked()
                    {
                        if let Some(summary) = selected_summary.as_ref() {
                            save_menu.pending_action =
                                Some(PendingSaveAction::Overwrite(summary.id.clone()));
                        }
                    }
                    if ui
                        .add_enabled(has_selection, egui::Button::new("Load"))
                        .clicked()
                    {
                        if let Some(summary) = selected_summary.as_ref() {
                            save_menu.pending_action =
                                Some(PendingSaveAction::Load(summary.id.clone()));
                        }
                    }
                    if ui.button("Cancel").clicked() {
                        toggle_save_menu(save_menu, clock);
                        kpi.record_manual_action(sim.simulation.tick());
                    }
                });

                if let Some(pending_action) = save_menu.pending_action.clone() {
                    let target_id = match &pending_action {
                        PendingSaveAction::Load(id) | PendingSaveAction::Overwrite(id) => id,
                    };
                    let target_summary =
                        entries.iter().find(|entry| &entry.id == target_id).cloned();
                    columns[1].add_space(12.0);
                    columns[1].group(|ui| {
                        let action_label = match pending_action {
                            PendingSaveAction::Load(_) => "Load",
                            PendingSaveAction::Overwrite(_) => "Overwrite",
                        };
                        ui.strong(format!(
                            "{action_label} {}?",
                            target_summary
                                .as_ref()
                                .map(|summary| summary.display_name.as_str())
                                .unwrap_or("selected save")
                        ));
                        ui.label("This action requires confirmation.");
                        ui.horizontal(|ui| {
                            if ui.button(action_label).clicked() {
                                match pending_action {
                                    PendingSaveAction::Overwrite(save_id) => {
                                        match save_storage.overwrite_save(&save_id, &sim.simulation)
                                        {
                                            Ok(summary) => {
                                                refresh_save_entries(save_menu, save_storage);
                                                save_menu.selected_entry_id =
                                                    Some(summary.id.clone());
                                                save_menu.pending_action = None;
                                                save_menu.last_error = None;
                                                messages.push(format!(
                                                    "Overwrote save {}",
                                                    summary.display_name
                                                ));
                                                kpi.record_manual_action(sim.simulation.tick());
                                            }
                                            Err(error) => {
                                                save_menu.last_error = Some(error.to_string())
                                            }
                                        }
                                    }
                                    PendingSaveAction::Load(save_id) => {
                                        let config = sim.simulation.config().clone();
                                        match save_storage.load_save(&save_id).and_then(
                                            |envelope| {
                                                let save_name =
                                                    envelope.summary.display_name.clone();
                                                envelope
                                                    .into_simulation(config)
                                                    .map(|loaded| (save_name, loaded))
                                            },
                                        ) {
                                            Ok((save_name, loaded)) => apply_loaded_simulation(
                                                loaded,
                                                &save_name,
                                                sim,
                                                clock,
                                                camera,
                                                selected_system,
                                                selected_station,
                                                selected_ship,
                                                panels,
                                                missions_panel,
                                                tracked_ship,
                                                ship_ui,
                                                station_ui,
                                                markets_ui,
                                                finance_ui,
                                                kpi,
                                                messages,
                                                save_menu,
                                            ),
                                            Err(error) => {
                                                save_menu.last_error = Some(error.to_string())
                                            }
                                        }
                                    }
                                }
                            }
                            if ui.button("Back").clicked() {
                                save_menu.pending_action = None;
                            }
                        });
                    });
                }
            });
        });

    if !open && save_menu.open {
        toggle_save_menu(save_menu, clock);
    }
}

fn selected_save_summary(
    entries: &[GameSaveSummary],
    selected_entry_id: Option<&String>,
) -> Option<GameSaveSummary> {
    selected_entry_id.and_then(|selected_id| {
        entries
            .iter()
            .find(|entry| &entry.id == selected_id)
            .cloned()
    })
}
