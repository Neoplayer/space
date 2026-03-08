use bevy_egui::egui;

use crate::features::missions::{
    open_active_mission, open_mission_offer, MissionModalSelection, MissionsPanelState,
};
use crate::runtime::sim::{SelectedShip, SimResource, UiKpiTracker, UiPanelState};

use super::labels::{commodity_label, mission_action_error_label, mission_offer_error_label};
use super::messages::HudMessages;
use super::missions_snapshot::MissionModalKind;
use super::snapshot::{HudSnapshot, StationCardSnapshot};

pub(super) struct MissionHudAccess<'a> {
    pub sim: &'a mut SimResource,
    pub selected_ship: &'a SelectedShip,
    pub panels: &'a mut UiPanelState,
    pub missions_panel: &'a mut MissionsPanelState,
    pub kpi: &'a mut UiKpiTracker,
    pub messages: &'a mut HudMessages,
}

pub(super) fn render_missions_windows(
    ctx: &egui::Context,
    snapshot: &HudSnapshot,
    save_menu_open: bool,
    access: MissionHudAccess<'_>,
) {
    if save_menu_open {
        return;
    }

    let MissionHudAccess {
        sim,
        selected_ship,
        panels,
        missions_panel,
        kpi,
        messages,
    } = access;

    if panels.missions {
        let mut open = panels.missions;
        egui::Window::new("Missions")
            .anchor(egui::Align2::LEFT_TOP, egui::vec2(12.0, 56.0))
            .default_width(520.0)
            .default_height(560.0)
            .open(&mut open)
            .show(ctx, |ui| {
                if missions_panel.selected_mission_id.is_none() {
                    missions_panel.selected_mission_id = snapshot
                        .active_mission_rows
                        .first()
                        .map(|row| row.mission_id);
                }

                ui.horizontal_wrapped(|ui| {
                    ui.label(format!("Active: {}", snapshot.active_mission_rows.len()));
                    ui.separator();
                    ui.label(format!(
                        "Selected ship: {}",
                        selected_ship
                            .ship_id
                            .map(|ship_id| format!("#{}", ship_id.0))
                            .unwrap_or_else(|| "none".to_string())
                    ));
                });
                ui.separator();

                ui.heading("Active Missions");
                ui.add_space(6.0);
                if snapshot.active_mission_rows.is_empty() {
                    ui.small("No active missions");
                } else {
                    egui::ScrollArea::vertical()
                        .id_salt("missions_board_active")
                        .max_height(420.0)
                        .show(ui, |ui| {
                            for row in &snapshot.active_mission_rows {
                                let selected =
                                    missions_panel.selected_mission_id == Some(row.mission_id);
                                egui::Frame::group(ui.style())
                                    .fill(if selected {
                                        egui::Color32::from_rgb(28, 38, 46)
                                    } else {
                                        egui::Color32::from_rgb(16, 21, 28)
                                    })
                                    .show(ui, |ui| {
                                        if ui
                                            .selectable_label(selected, &row.summary.summary_line)
                                            .clicked()
                                        {
                                            missions_panel.selected_mission_id =
                                                Some(row.mission_id);
                                        }
                                        ui.small(format!(
                                            "{} • {} {:.1} • penalty {:.1}",
                                            row.status_label,
                                            commodity_label(row.commodity),
                                            row.quantity,
                                            row.penalty
                                        ));
                                        if ui.button("Открыть").clicked() {
                                            open_active_mission(missions_panel, row.mission_id);
                                        }
                                    });
                                ui.add_space(6.0);
                            }
                        });
                }
            });
        panels.missions = open;
    }

    if missions_panel.modal_selection.is_some() {
        let mut open = true;
        if let Some(modal) = snapshot.mission_modal.as_ref() {
            egui::Window::new("Mission")
                .default_width(420.0)
                .resizable(false)
                .collapsible(false)
                .open(&mut open)
                .show(ctx, |ui| {
                    ui.heading(&modal.title);
                    ui.add_space(4.0);
                    ui.monospace(&modal.summary.summary_line);
                    ui.small(format!(
                        "{} {:.1} • reward {:.1} • penalty {:.1}",
                        commodity_label(modal.commodity),
                        modal.quantity,
                        modal.reward,
                        modal.penalty
                    ));
                    if let Some(status_label) = modal.status_label.as_ref() {
                        ui.small(format!("Status: {status_label}"));
                    }
                    if let (Some(stored), Some(required)) =
                        (modal.destination_storage_amount, modal.required_quantity)
                    {
                        ui.small(format!(
                            "Destination storage: {:.1} / {:.1}",
                            stored, required
                        ));
                    }
                    if let Some(reason) = modal.complete_disabled_reason.as_ref() {
                        if !modal.can_complete {
                            ui.colored_label(egui::Color32::from_rgb(232, 194, 88), reason);
                        }
                    }

                    ui.add_space(10.0);
                    match modal.kind {
                        MissionModalKind::Offer => {
                            if ui.button("Принять миссию").clicked() {
                                kpi.record_manual_action(sim.simulation.tick());
                                let offer_id = match modal.selection {
                                    MissionModalSelection::Offer(offer_id) => offer_id,
                                    MissionModalSelection::Active(_) => {
                                        unreachable!(
                                            "offer modal should always carry offer selection"
                                        )
                                    }
                                };
                                match sim.simulation.accept_mission_offer(offer_id) {
                                    Ok(mission_id) => {
                                        open_active_mission(missions_panel, mission_id);
                                        panels.missions = true;
                                        messages.push(format!(
                                            "Accepted mission offer {} as mission {}",
                                            offer_id, mission_id.0
                                        ));
                                    }
                                    Err(err) => messages.push(format!(
                                        "Mission accept failed: {}",
                                        mission_offer_error_label(err)
                                    )),
                                }
                            }
                        }
                        MissionModalKind::Active => {
                            let action_ship_id =
                                selected_ship.ship_id.or(snapshot.default_player_ship_id);
                            if ui
                                .add_enabled(
                                    modal.can_complete,
                                    egui::Button::new("Завершить миссию"),
                                )
                                .clicked()
                            {
                                if let Some(ship_id) = action_ship_id {
                                    let mission_id = match modal.selection {
                                        MissionModalSelection::Active(mission_id) => mission_id,
                                        MissionModalSelection::Offer(_) => {
                                            unreachable!(
                                                "active modal should carry mission selection"
                                            )
                                        }
                                    };
                                    kpi.record_manual_action(sim.simulation.tick());
                                    match sim.simulation.complete_mission(ship_id, mission_id) {
                                        Ok(()) => {
                                            messages.push(format!(
                                                "Completed mission {}",
                                                mission_id.0
                                            ));
                                            missions_panel.modal_selection = None;
                                        }
                                        Err(err) => messages.push(format!(
                                            "Mission completion failed: {}",
                                            mission_action_error_label(err)
                                        )),
                                    }
                                }
                            }
                            if ui.button("Отменить миссию").clicked() {
                                let mission_id = match modal.selection {
                                    MissionModalSelection::Active(mission_id) => mission_id,
                                    MissionModalSelection::Offer(_) => {
                                        unreachable!("active modal should carry mission selection")
                                    }
                                };
                                kpi.record_manual_action(sim.simulation.tick());
                                match sim.simulation.cancel_mission(mission_id) {
                                    Ok(()) => {
                                        messages
                                            .push(format!("Cancelled mission {}", mission_id.0));
                                        missions_panel.modal_selection = None;
                                    }
                                    Err(err) => messages.push(format!(
                                        "Mission cancel failed: {}",
                                        mission_action_error_label(err)
                                    )),
                                }
                            }
                        }
                    }
                });
        }
        if !open {
            missions_panel.modal_selection = None;
        }
    }
}

pub(super) fn render_station_missions_tab(
    ui: &mut egui::Ui,
    missions_panel: &mut MissionsPanelState,
    card: &StationCardSnapshot,
) {
    let missions = &card.missions;

    ui.horizontal_wrapped(|ui| {
        ui.monospace(format!(
            "Docked: {}",
            if missions.docked { "yes" } else { "no" }
        ));
        ui.separator();
        ui.monospace(format!("Offers: {}", missions.offers.len()));
    });

    ui.add_space(8.0);
    ui.heading("Available Offers");
    if missions.offers.is_empty() {
        ui.small("No mission offers for this station right now.");
    } else {
        egui::ScrollArea::vertical()
            .id_salt("station_mission_offers")
            .max_height(320.0)
            .show(ui, |ui| {
                for row in &missions.offers {
                    egui::Frame::group(ui.style())
                        .fill(egui::Color32::from_rgb(16, 21, 28))
                        .show(ui, |ui| {
                            ui.monospace(&row.summary.summary_line);
                            ui.small(format!(
                                "{} {:.1} • penalty {:.1}",
                                commodity_label(row.commodity),
                                row.quantity,
                                row.penalty
                            ));
                            if ui.button("Открыть").clicked() {
                                open_mission_offer(missions_panel, row.offer_id);
                            }
                        });
                    ui.add_space(6.0);
                }
            });
    }
}
