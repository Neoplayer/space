use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use gatebound_domain::{CargoLoad, CargoSource, Commodity, PriorityMode, ShipId};
use gatebound_sim::{PopulationTrend, TradePriceTone};

use crate::runtime::save::{
    apply_loaded_simulation, format_save_timestamp, refresh_save_entries, toggle_save_menu,
    toggle_save_menu_with_storage, PendingSaveAction, SaveMenuState, SaveStorage,
};
use crate::runtime::sim::{
    open_active_mission, open_mission_offer, open_ship_card, open_station_card,
    open_system_ship_inspector_selection, open_system_station_inspector_selection,
    open_system_view, panel_button_specs, panel_is_open, preferred_trade_commodity,
    seed_markets_ui_state, set_time_speed, toggle_pause, track_ship, FinanceUiState,
    MarketsUiState, MissionsPanelState, SelectedShip, SelectedStation, SelectedSystem, ShipCardTab,
    ShipUiState, SimClock, SimResource, StationCardTab, StationUiState, TrackedShip, UiKpiTracker,
    UiPanelState,
};

use super::labels::{
    cargo_source_label, command_error_label, commodity_label, company_archetype_label,
    credit_error_label, milestone_label, mission_action_error_label, mission_offer_error_label,
    priority_mode_label, ship_class_label, ship_module_slot_label, ship_module_status_label,
    ship_role_label, station_profile_label, storage_transfer_error_label, trade_error_label,
};
use super::messages::HudMessages;
use super::snapshot::{
    build_hud_snapshot, build_ship_card_snapshot_for_ui, build_station_card_snapshot_for_ui,
    MarketsDashboardSnapshot, MarketsStationDetailSnapshot, MissionModalKind, ShipCardSnapshot,
    StationCardSnapshot, StationRefSnapshot, SystemPanelSnapshot, SystemRefSnapshot,
    SystemShipSnapshot, SystemStationSnapshot, SystemsListRowSnapshot,
};

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

    egui::TopBottomPanel::top("gatebound_top_panel").show(ctx, |ui| {
        ui.columns(2, |columns| {
            columns[0].horizontal_wrapped(|ui| {
                let mut menu_button = egui::Button::new("Menu (Esc)");
                if save_menu_open {
                    menu_button = menu_button.fill(ui.visuals().selection.bg_fill);
                }
                if ui.add(menu_button).clicked() {
                    toggle_save_menu_with_storage(&mut save_menu, &mut clock, &save_storage);
                    kpi.record_manual_action(sim.simulation.tick());
                }
                if matches!(camera.mode, crate::input::camera::CameraMode::System(_)) {
                    if ui
                        .add_enabled(!save_menu_open, egui::Button::new("Galaxy View"))
                        .clicked()
                    {
                        camera.mode = crate::input::camera::CameraMode::Galaxy;
                        kpi.record_manual_action(sim.simulation.tick());
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
                        toggle_pause(&mut clock);
                        kpi.record_manual_action(sim.simulation.tick());
                    }

                    for speed in [1_u32, 2, 4] {
                        let mut speed_button = egui::Button::new(format!("{speed}x"));
                        if snapshot.speed_multiplier == speed {
                            speed_button = speed_button.fill(ui.visuals().selection.bg_fill);
                        }
                        if ui.add(speed_button).clicked() {
                            set_time_speed(&mut clock, speed);
                            kpi.record_manual_action(sim.simulation.tick());
                        }
                    }
                });
            });
            columns[1].with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.strong(format!("Balance: {:.1}", snapshot.capital));
            });
        });
    });

    egui::SidePanel::left("gatebound_left_hud")
        .resizable(true)
        .show(ctx, |ui| {
            ui.heading("Windows");
            ui.add_enabled_ui(!save_menu_open, |ui| {
                for button in panel_button_specs() {
                    let open = panel_is_open(&panels, button.index);
                    let label = format!("{} ({})", button.label, button.hotkey);
                    if ui.selectable_label(open, label).clicked() {
                        crate::runtime::sim::apply_panel_toggle(&mut panels, button.index);
                        if button.index == 6 {
                            station_ui.station_panel_open = panels.station_ops;
                            if panels.station_ops {
                                let station_id =
                                    selected_station.station_id.or(station_ui.card_station_id);
                                if let Some(station_id) = station_id {
                                    let preferred = preferred_trade_commodity(
                                        &sim.simulation,
                                        selected_ship.ship_id.or(snapshot.default_player_ship_id),
                                        station_id,
                                        station_ui.trade_commodity,
                                    );
                                    open_station_card(&mut station_ui, station_id, Some(preferred));
                                }
                            }
                        }
                        kpi.record_manual_action(sim.simulation.tick());
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

    if !save_menu_open {
        if let Some(system_panel) = snapshot.system_panel.as_ref() {
            let current_station_id = selected_station.station_id;
            let current_ship_id = selected_ship.ship_id;
            let preferred_ship_id = selected_ship.ship_id.or(snapshot.default_player_ship_id);
            let current_tick = sim.simulation.tick();
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
                                &sim.simulation,
                                &mut selected_station,
                                &mut panels,
                                &mut station_ui,
                                &mut kpi,
                            );
                            ui.add_space(10.0);
                            render_system_ships(
                                ui,
                                system_panel,
                                current_ship_id,
                                current_tick,
                                &mut selected_ship,
                                &mut ship_ui,
                                &mut kpi,
                            );
                        });
                });
        }
    }

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

    if !save_menu_open && ship_ui.context_menu_open {
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
                    if let Some(system_id) =
                        track_ship(&mut tracked_ship, &mut camera, &sim.simulation, ship_id)
                    {
                        messages.push(format!(
                            "Tracking ship {} in system {}",
                            ship_id.0, system_id.0
                        ));
                        ship_ui.context_menu_open = false;
                    }
                }
                if ui.button("Open ship card").clicked() {
                    open_ship_card(&mut ship_ui, ship_id);
                    ship_ui.context_menu_open = false;
                }
            });
        ship_ui.context_menu_open = open && ship_ui.context_menu_open;
    }

    if !save_menu_open && station_ui.context_menu_open {
        let mut open = station_ui.context_menu_open;
        egui::Window::new("Station Context")
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .show(ctx, |ui| {
                if selected_ship.ship_id.is_none() {
                    selected_ship.ship_id = snapshot.default_player_ship_id;
                }
                let Some(station_id) = station_ui.context_station_id else {
                    ui.label("No station selected");
                    return;
                };
                ui.label(format!("Station: {}", station_id.0));
                let Some(ship_id) = selected_ship.ship_id else {
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
                    open_station_card(&mut station_ui, station_id, Some(preferred));
                    panels.station_ops = true;
                    station_ui.context_menu_open = false;
                }
            });
        station_ui.context_menu_open = open && station_ui.context_menu_open;
    }

    if !save_menu_open && panels.missions {
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
                                            open_active_mission(
                                                &mut missions_panel,
                                                row.mission_id,
                                            );
                                        }
                                    });
                                ui.add_space(6.0);
                            }
                        });
                }
            });
        panels.missions = open;
    }

    if !save_menu_open && missions_panel.modal_selection.is_some() {
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
                                    crate::runtime::sim::MissionModalSelection::Offer(offer_id) => {
                                        offer_id
                                    }
                                    crate::runtime::sim::MissionModalSelection::Active(_) => {
                                        unreachable!(
                                            "offer modal should always carry offer selection"
                                        )
                                    }
                                };
                                match sim.simulation.accept_mission_offer(offer_id) {
                                    Ok(mission_id) => {
                                        open_active_mission(&mut missions_panel, mission_id);
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
                                        crate::runtime::sim::MissionModalSelection::Active(
                                            mission_id,
                                        ) => mission_id,
                                        crate::runtime::sim::MissionModalSelection::Offer(_) => {
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
                                    crate::runtime::sim::MissionModalSelection::Active(
                                        mission_id,
                                    ) => mission_id,
                                    crate::runtime::sim::MissionModalSelection::Offer(_) => {
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

    if !save_menu_open && panels.fleet {
        let mut open = panels.fleet;
        egui::Window::new("Fleet Manager")
            .default_width(560.0)
            .default_height(420.0)
            .open(&mut open)
            .show(ctx, |ui| {
                ui.label(format!("Ships: {}", snapshot.fleet_list_rows.len()));
                ui.separator();

                if snapshot.fleet_list_rows.is_empty() {
                    ui.label("No player ships available");
                    return;
                }

                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for row in &snapshot.fleet_list_rows {
                            egui::Frame::group(ui.style())
                                .fill(egui::Color32::from_rgb(16, 21, 28))
                                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(56, 72, 88)))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.vertical(|ui| {
                                            ui.label(egui::RichText::new(&row.ship_name).strong());
                                            ui.label(&row.location_text);
                                            ui.label(
                                                egui::RichText::new(&row.status_text)
                                                    .color(egui::Color32::from_rgb(143, 185, 255)),
                                            );
                                        });
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                if ui.button("Open card").clicked() {
                                                    open_ship_card(&mut ship_ui, row.ship_id);
                                                }
                                            },
                                        );
                                    });
                                });
                            ui.add_space(6.0);
                        }
                    });
            });
        panels.fleet = open;
    }

    if !save_menu_open && panels.systems {
        let mut open = panels.systems;
        let current_mode = camera.mode;
        egui::Window::new("Systems")
            .default_width(520.0)
            .default_height(480.0)
            .open(&mut open)
            .show(ctx, |ui| {
                ui.label(format!("Systems: {}", snapshot.systems_list_rows.len()));
                ui.separator();

                if snapshot.systems_list_rows.is_empty() {
                    ui.label("No systems available");
                    return;
                }

                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for row in &snapshot.systems_list_rows {
                            render_systems_list_row(
                                ui,
                                row,
                                current_mode,
                                &mut camera,
                                &mut kpi,
                                sim.simulation.tick(),
                            );
                            ui.add_space(6.0);
                        }
                    });
            });
        panels.systems = open;
    }

    if !save_menu_open && panels.markets {
        let mut open = panels.markets;
        egui::Window::new("Markets")
            .default_width(1120.0)
            .default_height(760.0)
            .open(&mut open)
            .show(ctx, |ui| {
                render_markets_dashboard(
                    ui,
                    &snapshot.markets,
                    &mut markets_ui,
                    &mut kpi,
                    sim.simulation.tick(),
                );
            });
        panels.markets = open;
    }

    if !save_menu_open && ship_ui.card_open {
        let mut open = ship_ui.card_open;
        egui::Window::new("Ship Card")
            .open(&mut open)
            .default_width(720.0)
            .default_height(560.0)
            .show(ctx, |ui| {
                let Some(card) = live_ship_card.as_ref() else {
                    ui.label("No ship selected");
                    return;
                };
                ship_ui.card_ship_id = Some(card.ship_id);
                ship_ui.context_ship_id = Some(card.ship_id);

                render_ship_card_header(ui, card);
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui
                        .add(tab_button(
                            "Overview",
                            ship_ui.card_tab == ShipCardTab::Overview,
                        ))
                        .clicked()
                    {
                        ship_ui.card_tab = ShipCardTab::Overview;
                    }
                    if ui
                        .add(tab_button("Cargo", ship_ui.card_tab == ShipCardTab::Cargo))
                        .clicked()
                    {
                        ship_ui.card_tab = ShipCardTab::Cargo;
                    }
                    if ui
                        .add(tab_button(
                            "Modules",
                            ship_ui.card_tab == ShipCardTab::Modules,
                        ))
                        .clicked()
                    {
                        ship_ui.card_tab = ShipCardTab::Modules;
                    }
                    if ui
                        .add(tab_button(
                            "Technical",
                            ship_ui.card_tab == ShipCardTab::Technical,
                        ))
                        .clicked()
                    {
                        ship_ui.card_tab = ShipCardTab::Technical;
                    }
                });
                ui.separator();

                match ship_ui.card_tab {
                    ShipCardTab::Overview => render_ship_overview_tab(ui, card),
                    ShipCardTab::Cargo => render_ship_cargo_tab(ui, card),
                    ShipCardTab::Modules => render_ship_modules_tab(ui, card),
                    ShipCardTab::Technical => render_ship_technical_tab(ui, card),
                }
            });
        ship_ui.card_open = open;
    }

    if !save_menu_open && panels.station_ops && station_ui.station_panel_open {
        let mut open = panels.station_ops;
        egui::Window::new("Station Card")
            .open(&mut open)
            .default_width(760.0)
            .default_height(560.0)
            .show(ctx, |ui| {
                if selected_ship.ship_id.is_none() {
                    selected_ship.ship_id = snapshot.default_player_ship_id;
                }
                let Some(card) = live_station_card.as_ref() else {
                    ui.label("No station selected");
                    return;
                };
                station_ui.card_station_id = Some(card.station_id);
                station_ui.context_station_id = Some(card.station_id);
                if !card
                    .trade
                    .rows
                    .iter()
                    .any(|row| row.commodity == station_ui.trade_commodity)
                {
                    if let Some(row) = card.trade.rows.first() {
                        station_ui.trade_commodity = row.commodity;
                    }
                }
                if !card
                    .storage
                    .rows
                    .iter()
                    .any(|row| row.commodity == station_ui.storage_commodity)
                {
                    if let Some(row) = card.storage.rows.first() {
                        station_ui.storage_commodity = row.commodity;
                    }
                }

                render_station_card_header(ui, card, selected_ship.ship_id);
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    let info_selected = station_ui.card_tab == StationCardTab::Info;
                    let trade_selected = station_ui.card_tab == StationCardTab::Trade;
                    let storage_selected = station_ui.card_tab == StationCardTab::Storage;
                    let missions_selected = station_ui.card_tab == StationCardTab::Missions;
                    if ui.add(tab_button("Info", info_selected)).clicked() {
                        station_ui.card_tab = StationCardTab::Info;
                    }
                    if ui.add(tab_button("Trade", trade_selected)).clicked() {
                        station_ui.card_tab = StationCardTab::Trade;
                    }
                    if ui.add(tab_button("Storage", storage_selected)).clicked() {
                        station_ui.card_tab = StationCardTab::Storage;
                    }
                    if ui.add(tab_button("Missions", missions_selected)).clicked() {
                        station_ui.card_tab = StationCardTab::Missions;
                    }
                });
                ui.separator();

                match station_ui.card_tab {
                    StationCardTab::Info => render_station_info_tab(ui, card),
                    StationCardTab::Trade => {
                        let Some(ship_id) =
                            selected_ship.ship_id.or(snapshot.default_player_ship_id)
                        else {
                            ui.label("No player ship available");
                            return;
                        };
                        render_station_trade_tab(
                            ui,
                            &mut sim,
                            &mut kpi,
                            &mut messages,
                            &mut station_ui,
                            ship_id,
                            card,
                        );
                    }
                    StationCardTab::Storage => {
                        let Some(ship_id) =
                            selected_ship.ship_id.or(snapshot.default_player_ship_id)
                        else {
                            ui.label("No player ship available");
                            return;
                        };
                        render_station_storage_tab(
                            ui,
                            &mut sim,
                            &mut kpi,
                            &mut messages,
                            &mut station_ui,
                            ship_id,
                            card,
                        );
                    }
                    StationCardTab::Missions => {
                        let Some(ship_id) =
                            selected_ship.ship_id.or(snapshot.default_player_ship_id)
                        else {
                            ui.label("No player ship available");
                            return;
                        };
                        render_station_missions_tab(
                            ui,
                            &mut sim,
                            &mut kpi,
                            &mut messages,
                            &mut panels,
                            &mut missions_panel,
                            &mut station_ui,
                            ship_id,
                            card,
                        );
                    }
                }
            });
        panels.station_ops = open;
        station_ui.station_panel_open = open;
    }

    if !save_menu_open && panels.assets {
        let mut open = panels.assets;
        egui::Window::new("Finance")
            .open(&mut open)
            .show(ctx, |ui| {
                ui.heading("Player Finance");
                ui.label(format!("Capital: {:.1}", snapshot.capital));
                ui.label(format!("Debt: {:.1}", snapshot.debt));
                ui.label(format!("Rate: {:.2}%", snapshot.interest_rate * 100.0));
                ui.label(format!("Reputation: {:.2}", snapshot.reputation));
                ui.separator();

                if let Some(active_loan) = snapshot.active_loan {
                    finance_ui.pending_offer = None;
                    ui.heading("Active Loan");
                    ui.label(format!("Offer: {}", active_loan.offer_id.label()));
                    ui.label(format!("Principal: {:.1}", active_loan.principal_remaining));
                    ui.label(format!(
                        "Months remaining: {}",
                        active_loan.remaining_months
                    ));
                    ui.label(format!(
                        "Next monthly payment: {:.1}",
                        active_loan.next_payment
                    ));
                    ui.horizontal(|ui| {
                        ui.label("Repay amount");
                        ui.add(
                            egui::DragValue::new(&mut finance_ui.repayment_amount)
                                .speed(1.0)
                                .range(0.1..=10_000.0),
                        );
                    });
                    ui.horizontal(|ui| {
                        if ui.button("Repay Part").clicked() {
                            kpi.record_manual_action(sim.simulation.tick());
                            match sim.simulation.repay_credit(finance_ui.repayment_amount) {
                                Ok(()) => messages.push(format!(
                                    "Repaid {:.1} toward active loan",
                                    finance_ui
                                        .repayment_amount
                                        .min(active_loan.principal_remaining)
                                )),
                                Err(err) => messages
                                    .push(format!("Repay failed: {}", credit_error_label(err))),
                            }
                        }
                        if ui.button("Repay Full").clicked() {
                            kpi.record_manual_action(sim.simulation.tick());
                            match sim.simulation.repay_credit(active_loan.principal_remaining) {
                                Ok(()) => messages.push("Loan fully repaid".to_string()),
                                Err(err) => messages
                                    .push(format!("Repay failed: {}", credit_error_label(err))),
                            }
                        }
                    });
                    ui.separator();
                    ui.label("Credit offers unlock again after the current loan is closed.");
                } else {
                    ui.heading("Credit Offers");
                    for offer in &snapshot.loan_offers {
                        ui.group(|ui| {
                            ui.horizontal(|ui| {
                                ui.heading(offer.label);
                                ui.separator();
                                ui.label(format!("Amount: {:.1}", offer.principal));
                                ui.separator();
                                ui.label(format!(
                                    "Rate: {:.2}%/month",
                                    offer.monthly_interest_rate * 100.0
                                ));
                                ui.separator();
                                ui.label(format!("Term: {} mo", offer.term_months));
                                ui.separator();
                                ui.label(format!("Payment: {:.1}", offer.monthly_payment));
                            });
                            if finance_ui.pending_offer == Some(offer.id) {
                                ui.horizontal(|ui| {
                                    ui.label("Confirm taking this credit?");
                                    if ui.button("Confirm").clicked() {
                                        kpi.record_manual_action(sim.simulation.tick());
                                        match sim.simulation.take_credit(offer.id) {
                                            Ok(()) => {
                                                finance_ui.pending_offer = None;
                                                messages.push(format!(
                                                    "Credit approved: {} +{:.1}",
                                                    offer.label, offer.principal
                                                ));
                                            }
                                            Err(err) => messages.push(format!(
                                                "Credit failed: {}",
                                                credit_error_label(err)
                                            )),
                                        }
                                    }
                                    if ui.button("Cancel").clicked() {
                                        finance_ui.pending_offer = None;
                                    }
                                });
                            } else if ui.button(format!("Take {}", offer.label)).clicked() {
                                finance_ui.pending_offer = Some(offer.id);
                            }
                        });
                    }
                }
            });
        panels.assets = open;
    }

    if !save_menu_open && panels.policies {
        let mut open = panels.policies;
        egui::Window::new("Autopilot Policies")
            .open(&mut open)
            .show(ctx, |ui| {
                if selected_ship.ship_id.is_none() {
                    selected_ship.ship_id = snapshot.default_player_ship_id;
                }
                let Some(ship_id) = selected_ship.ship_id else {
                    ui.label("No player ship available");
                    return;
                };
                ui.label(format!("Selected ship: #{}", ship_id.0));
                let tick_now = sim.simulation.tick();
                if let Some(policy_view) = sim.simulation.ship_policy_view(ship_id) {
                    let mut policy = policy_view.policy;
                    let mut policy_changed = false;
                    ui.horizontal(|ui| {
                        ui.label("min_margin");
                        policy_changed |= ui
                            .add(egui::DragValue::new(&mut policy.min_margin).speed(0.1))
                            .changed();
                        ui.label("max_risk");
                        policy_changed |= ui
                            .add(egui::DragValue::new(&mut policy.max_risk_score).speed(0.1))
                            .changed();
                        ui.label("max_hops");
                        policy_changed |= ui
                            .add(egui::DragValue::new(&mut policy.max_hops).speed(1.0))
                            .changed();
                    });
                    egui::ComboBox::from_label("priority_mode")
                        .selected_text(priority_mode_label(policy.priority_mode))
                        .show_ui(ui, |ui| {
                            policy_changed |= ui
                                .selectable_value(
                                    &mut policy.priority_mode,
                                    PriorityMode::Profit,
                                    priority_mode_label(PriorityMode::Profit),
                                )
                                .changed();
                            policy_changed |= ui
                                .selectable_value(
                                    &mut policy.priority_mode,
                                    PriorityMode::Stability,
                                    priority_mode_label(PriorityMode::Stability),
                                )
                                .changed();
                            policy_changed |= ui
                                .selectable_value(
                                    &mut policy.priority_mode,
                                    PriorityMode::Hybrid,
                                    priority_mode_label(PriorityMode::Hybrid),
                                )
                                .changed();
                        });
                    if policy_changed
                        && sim
                            .simulation
                            .update_ship_policy(ship_id, policy.clone())
                            .is_ok()
                    {
                        kpi.record_manual_action(tick_now);
                        kpi.record_policy_edit(tick_now);
                    }
                    ui.label(format!(
                        "waypoints={}",
                        policy
                            .waypoints
                            .iter()
                            .map(|system_id| system_id.0.to_string())
                            .collect::<Vec<_>>()
                            .join(" -> ")
                    ));
                } else {
                    ui.label("Selected ship not found");
                }

                ui.separator();
                ui.heading("Manual vs Policy KPI");
                ui.monospace(format!(
                    "manual/min={:.1} policy_edits/min={:.1} avg_route_hops={:.2}",
                    snapshot.manual_actions_per_min,
                    snapshot.policy_edits_per_min,
                    snapshot.avg_route_hops_player
                ));
                ui.separator();
                ui.heading("Milestones");
                for milestone in &snapshot.milestones {
                    ui.monospace(format!(
                        "{} current={:.2} target={:.2} completed={} cycle={}",
                        milestone_label(milestone),
                        milestone.current,
                        milestone.target,
                        milestone.completed,
                        milestone
                            .completed_cycle
                            .map(|cycle| cycle.to_string())
                            .unwrap_or_else(|| "-".to_string())
                    ));
                }
            });
        panels.policies = open;
    }

    if !save_menu_open && panels.corporations {
        let mut open = panels.corporations;
        egui::Window::new("NPC Corporations")
            .open(&mut open)
            .show(ctx, |ui| {
                ui.label(format!(
                    "Tracked corporations: {}",
                    snapshot.corporation_rows.len()
                ));
                ui.separator();
                egui::Grid::new("corporation_panel_grid")
                    .num_columns(8)
                    .striped(true)
                    .show(ui, |ui| {
                        ui.strong("Corp");
                        ui.strong("Type");
                        ui.strong("Balance");
                        ui.strong("Last P&L");
                        ui.strong("Idle");
                        ui.strong("Transit");
                        ui.strong("Orders");
                        ui.strong("Next Tick");
                        ui.end_row();

                        for row in &snapshot.corporation_rows {
                            ui.label(&row.name);
                            ui.monospace(company_archetype_label(row.archetype));
                            ui.monospace(format!("{:.1}", row.balance));
                            ui.monospace(format!("{:.1}", row.last_realized_profit));
                            ui.monospace(format!("{}", row.idle_ships));
                            ui.monospace(format!("{}", row.in_transit_ships));
                            ui.monospace(format!("{}", row.active_orders));
                            ui.monospace(format!("{}", row.next_plan_tick));
                            ui.end_row();
                        }
                    });
            });
        panels.corporations = open;
    }

    render_save_menu(
        ctx,
        &save_storage,
        &mut save_menu,
        &mut sim,
        &mut clock,
        &mut camera,
        &mut selected_system,
        &mut selected_station,
        &mut selected_ship,
        &mut panels,
        &mut missions_panel,
        &mut tracked_ship,
        &mut ship_ui,
        &mut station_ui,
        &mut markets_ui,
        &mut finance_ui,
        &mut kpi,
        &mut messages,
    );

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn render_save_menu(
    ctx: &egui::Context,
    save_storage: &SaveStorage,
    save_menu: &mut SaveMenuState,
    sim: &mut SimResource,
    clock: &mut SimClock,
    camera: &mut crate::input::camera::CameraUiState,
    selected_system: &mut SelectedSystem,
    selected_station: &mut SelectedStation,
    selected_ship: &mut SelectedShip,
    panels: &mut UiPanelState,
    missions_panel: &mut MissionsPanelState,
    tracked_ship: &mut TrackedShip,
    ship_ui: &mut ShipUiState,
    station_ui: &mut StationUiState,
    markets_ui: &mut MarketsUiState,
    finance_ui: &mut FinanceUiState,
    kpi: &mut UiKpiTracker,
    messages: &mut HudMessages,
) {
    if !save_menu.open {
        return;
    }

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
                    save_menu
                        .selected_entry_id
                        .as_ref()
                        .and_then(|selected_id| {
                            entries
                                .iter()
                                .find(|entry| &entry.id == selected_id)
                                .cloned()
                        });
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

fn render_station_card_header(
    ui: &mut egui::Ui,
    card: &StationCardSnapshot,
    selected_ship_id: Option<ShipId>,
) {
    egui::Frame::group(ui.style())
        .fill(egui::Color32::from_rgb(15, 22, 28))
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(66, 90, 108)))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.heading(&card.station_name);
                    ui.label(
                        egui::RichText::new(format!(
                            "{} station in {}",
                            station_profile_label(card.profile),
                            card.system_name
                        ))
                        .color(egui::Color32::from_rgb(143, 185, 255)),
                    );
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let dock_text = if card.docked { "Docked" } else { "In approach" };
                    let dock_color = if card.docked {
                        egui::Color32::from_rgb(120, 220, 150)
                    } else {
                        egui::Color32::from_rgb(240, 180, 90)
                    };
                    ui.label(egui::RichText::new(dock_text).strong().color(dock_color));
                    if let Some(ship_id) = selected_ship_id {
                        ui.separator();
                        ui.monospace(format!("Ship #{}", ship_id.0));
                    }
                });
            });
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                ui.monospace(format!("Station #{:03}", card.station_id.0));
                ui.separator();
                ui.monospace(format!("System {}", card.system_id.0));
                ui.separator();
                ui.monospace(format!("Host {}", card.host_body_name));
                ui.separator();
                ui.monospace(&card.orbit_label);
                ui.separator();
                ui.monospace(format!(
                    "coords {:.1}, {:.1}",
                    card.station_x, card.station_y
                ));
            });
        });
}

fn render_station_info_tab(ui: &mut egui::Ui, card: &StationCardSnapshot) {
    ui.columns(2, |columns| {
        columns[0].vertical(|ui| {
            egui::Frame::group(ui.style())
                .fill(egui::Color32::from_rgb(18, 26, 34))
                .show(ui, |ui| {
                    ui.heading("Station Brief");
                    ui.label(&card.profile_summary);
                    ui.add_space(6.0);
                    ui.monospace(format!("Primary system: {}", card.system_name));
                    ui.monospace(format!("Host body: {}", card.host_body_name));
                    ui.monospace(format!("Orbit band: {}", card.orbit_label));
                    ui.monospace(format!(
                        "Population: {}",
                        format_population(card.population)
                    ));
                    ui.monospace(format!(
                        "Baseline load: {:.0}%",
                        card.population_ratio * 100.0
                    ));
                    ui.monospace(format!(
                        "Trend: {}",
                        population_trend_label(card.population_trend)
                    ));
                    ui.monospace(format!(
                        "Dock status: {}",
                        if card.docked {
                            "ready for cargo handling"
                        } else {
                            "remote / not docked"
                        }
                    ));
                });
        });
        columns[1].vertical(|ui| {
            egui::Frame::group(ui.style())
                .fill(egui::Color32::from_rgb(24, 24, 29))
                .show(ui, |ui| {
                    ui.heading("Trade Posture");
                    ui.label("Priority imports");
                    for note in &card.imports {
                        ui.monospace(format!("+ {note}"));
                    }
                    ui.add_space(8.0);
                    ui.label("Likely exports");
                    for note in &card.exports {
                        ui.monospace(format!("- {note}"));
                    }
                });
        });
    });

    ui.add_space(10.0);
    egui::Frame::group(ui.style())
        .fill(egui::Color32::from_rgb(20, 20, 24))
        .show(ui, |ui| {
            ui.heading("Operational Read");
            ui.label(match card.profile {
                gatebound_domain::StationProfile::Civilian => {
                    "Civilian concourses keep local demand steady: expect dependable retail pull, moderate fuel burn, and softer bulk margins."
                }
                gatebound_domain::StationProfile::Industrial => {
                    "Industrial yards reward timing around raw-material shortages and part surpluses; docking windows matter when fabrication queues spike."
                }
                gatebound_domain::StationProfile::Research => {
                    "Research arrays swing harder on precision goods: electronics and specialist inputs can flip from surplus to shortage in a single cycle."
                }
            });
        });
}

fn render_station_trade_tab(
    ui: &mut egui::Ui,
    sim: &mut ResMut<SimResource>,
    kpi: &mut ResMut<UiKpiTracker>,
    messages: &mut ResMut<HudMessages>,
    station_ui: &mut ResMut<StationUiState>,
    ship_id: ShipId,
    card: &StationCardSnapshot,
) {
    let trade = &card.trade;
    egui::Frame::group(ui.style())
        .fill(egui::Color32::from_rgb(14, 19, 24))
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                let cargo_line = cargo_summary_line(&trade.cargo_lots);
                ui.monospace(format!("Cargo: {cargo_line}"));
                ui.separator();
                ui.monospace(format!(
                    "Usage: {:.1} / {:.1}",
                    trade.cargo_total_amount, trade.cargo_capacity
                ));
                ui.separator();
                ui.monospace(format!("Fee: {:.1}%", trade.market_fee_rate * 100.0));
                ui.separator();
                ui.monospace(format!(
                    "Docked: {}",
                    if trade.docked { "yes" } else { "no" }
                ));
            });
        });

    ui.add_space(8.0);
    egui::ScrollArea::vertical()
        .max_height(240.0)
        .show(ui, |ui| {
            egui::Grid::new("station_trade_grid")
                .striped(true)
                .spacing([14.0, 6.0])
                .show(ui, |ui| {
                    ui.strong("Stock");
                    ui.strong("Buy @");
                    ui.strong("Commodity");
                    ui.strong("Sell @");
                    ui.strong("Cargo");
                    ui.end_row();

                    for row in &trade.rows {
                        ui.monospace(format!("{:>6.1}", row.station_stock));
                        ui.colored_label(
                            price_tone_color(row.buy_tone),
                            format!("{:>6.2}", row.effective_buy_price),
                        );
                        if ui
                            .selectable_label(
                                station_ui.trade_commodity == row.commodity,
                                commodity_label(row.commodity),
                            )
                            .clicked()
                        {
                            station_ui.trade_commodity = row.commodity;
                        }
                        ui.colored_label(
                            price_tone_color(row.sell_tone),
                            format!("{:>6.2}", row.effective_sell_price),
                        );
                        ui.monospace(format!("{:>6.1}", row.player_cargo));
                        ui.end_row();
                    }
                });
        });

    let selected_row = trade
        .rows
        .iter()
        .find(|row| row.commodity == station_ui.trade_commodity)
        .or_else(|| trade.rows.first());
    let Some(selected_row) = selected_row else {
        ui.label("No market rows available for this station");
        return;
    };

    ui.add_space(8.0);
    egui::Frame::group(ui.style())
        .fill(egui::Color32::from_rgb(20, 24, 30))
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.heading(format!(
                    "{} market row",
                    commodity_label(selected_row.commodity)
                ));
                ui.separator();
                ui.monospace(format!("galaxy avg {:.2}", selected_row.market_avg_price));
                ui.separator();
                ui.monospace(format!("buy cap {:.1}", selected_row.buy_cap));
                ui.separator();
                ui.monospace(format!("sell cap {:.1}", selected_row.sell_cap));
            });

            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label("Quantity");
                ui.add(
                    egui::DragValue::new(&mut station_ui.trade_quantity)
                        .speed(0.5)
                        .range(0.1..=10_000.0),
                );
                let preset_cap = selected_row.buy_cap.max(selected_row.sell_cap).max(0.1);
                if ui.button("25%").clicked() {
                    station_ui.trade_quantity = (preset_cap * 0.25).max(0.1);
                }
                if ui.button("50%").clicked() {
                    station_ui.trade_quantity = (preset_cap * 0.50).max(0.1);
                }
                if ui.button("100%").clicked() {
                    station_ui.trade_quantity = preset_cap.max(0.1);
                }
            });

            if let Some(reason) = buy_disabled_reason(trade.docked, &trade.cargo_lots, selected_row)
            {
                ui.colored_label(
                    egui::Color32::from_rgb(232, 194, 88),
                    format!("Buy unavailable: {reason}"),
                );
            }
            if let Some(reason) =
                sell_disabled_reason(trade.docked, &trade.cargo_lots, selected_row)
            {
                ui.colored_label(
                    egui::Color32::from_rgb(232, 194, 88),
                    format!("Sell unavailable: {reason}"),
                );
            }

            ui.horizontal(|ui| {
                if ui
                    .add_enabled(selected_row.can_buy, egui::Button::new("Buy"))
                    .clicked()
                {
                    kpi.record_manual_action(sim.simulation.tick());
                    match sim.simulation.player_buy(
                        ship_id,
                        card.station_id,
                        selected_row.commodity,
                        station_ui.trade_quantity.min(selected_row.buy_cap.max(0.0)),
                    ) {
                        Ok(receipt) => messages.push(format!(
                            "Bought {:.1} {} @ {:.2} fee={:.2} cash_delta={:.2}",
                            receipt.quantity,
                            commodity_label(receipt.commodity),
                            receipt.unit_price,
                            receipt.fee,
                            receipt.net_cash_delta
                        )),
                        Err(err) => {
                            messages.push(format!("Buy failed: {}", trade_error_label(err)))
                        }
                    }
                }
                if ui
                    .add_enabled(selected_row.can_sell, egui::Button::new("Sell"))
                    .clicked()
                {
                    kpi.record_manual_action(sim.simulation.tick());
                    match sim.simulation.player_sell(
                        ship_id,
                        card.station_id,
                        selected_row.commodity,
                        station_ui
                            .trade_quantity
                            .min(selected_row.sell_cap.max(0.0)),
                    ) {
                        Ok(receipt) => messages.push(format!(
                            "Sold {:.1} {} @ {:.2} fee={:.2} cash_delta={:.2}",
                            receipt.quantity,
                            commodity_label(receipt.commodity),
                            receipt.unit_price,
                            receipt.fee,
                            receipt.net_cash_delta
                        )),
                        Err(err) => {
                            messages.push(format!("Sell failed: {}", trade_error_label(err)))
                        }
                    }
                }
            });
        });
}

fn render_station_storage_tab(
    ui: &mut egui::Ui,
    sim: &mut ResMut<SimResource>,
    kpi: &mut ResMut<UiKpiTracker>,
    messages: &mut ResMut<HudMessages>,
    station_ui: &mut ResMut<StationUiState>,
    ship_id: ShipId,
    card: &StationCardSnapshot,
) {
    let storage = &card.storage;
    let total_stored = storage
        .rows
        .iter()
        .map(|row| row.stored_amount)
        .sum::<f64>();
    egui::Frame::group(ui.style())
        .fill(egui::Color32::from_rgb(14, 19, 24))
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                let cargo_line = cargo_summary_line(&storage.cargo_lots);
                ui.monospace(format!("Ship cargo: {cargo_line}"));
                ui.separator();
                ui.monospace(format!(
                    "Usage: {:.1} / {:.1}",
                    storage.cargo_total_amount, storage.cargo_capacity
                ));
                ui.separator();
                ui.monospace(format!("Stored total: {:.1}", total_stored));
                ui.separator();
                ui.monospace(format!(
                    "Docked: {}",
                    if storage.docked { "yes" } else { "no" }
                ));
            });
        });

    ui.add_space(8.0);
    egui::ScrollArea::vertical()
        .max_height(240.0)
        .show(ui, |ui| {
            egui::Grid::new("station_storage_grid")
                .striped(true)
                .spacing([14.0, 6.0])
                .show(ui, |ui| {
                    ui.strong("Stored");
                    ui.strong("Commodity");
                    ui.strong("Ship");
                    ui.strong("Load");
                    ui.strong("Unload");
                    ui.end_row();

                    for row in &storage.rows {
                        ui.monospace(format!("{:>6.1}", row.stored_amount));
                        if ui
                            .selectable_label(
                                station_ui.storage_commodity == row.commodity,
                                commodity_label(row.commodity),
                            )
                            .clicked()
                        {
                            station_ui.storage_commodity = row.commodity;
                        }
                        ui.monospace(format!("{:>6.1}", row.player_cargo));
                        ui.monospace(format!("{:>6.1}", row.load_cap));
                        ui.monospace(format!("{:>6.1}", row.unload_cap));
                        ui.end_row();
                    }
                });
        });

    let selected_row = storage
        .rows
        .iter()
        .find(|row| row.commodity == station_ui.storage_commodity)
        .or_else(|| storage.rows.first());
    let Some(selected_row) = selected_row else {
        ui.label("No stored cargo at this station yet.");
        return;
    };

    ui.add_space(8.0);
    egui::Frame::group(ui.style())
        .fill(egui::Color32::from_rgb(20, 24, 30))
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.heading(format!(
                    "{} storage row",
                    commodity_label(selected_row.commodity)
                ));
                ui.separator();
                ui.monospace(format!("stored {:.1}", selected_row.stored_amount));
                ui.separator();
                ui.monospace(format!("load cap {:.1}", selected_row.load_cap));
                ui.separator();
                ui.monospace(format!("unload cap {:.1}", selected_row.unload_cap));
            });

            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label("Quantity");
                ui.add(
                    egui::DragValue::new(&mut station_ui.storage_quantity)
                        .speed(0.5)
                        .range(0.1..=10_000.0),
                );
                let preset_cap = selected_row.load_cap.max(selected_row.unload_cap).max(0.1);
                if ui.button("25%").clicked() {
                    station_ui.storage_quantity = (preset_cap * 0.25).max(0.1);
                }
                if ui.button("50%").clicked() {
                    station_ui.storage_quantity = (preset_cap * 0.50).max(0.1);
                }
                if ui.button("100%").clicked() {
                    station_ui.storage_quantity = preset_cap.max(0.1);
                }
            });

            if let Some(reason) =
                storage_load_disabled_reason(storage.docked, &storage.cargo_lots, selected_row)
            {
                ui.colored_label(
                    egui::Color32::from_rgb(232, 194, 88),
                    format!("Load unavailable: {reason}"),
                );
            }
            if let Some(reason) =
                storage_unload_disabled_reason(storage.docked, &storage.cargo_lots, selected_row)
            {
                ui.colored_label(
                    egui::Color32::from_rgb(232, 194, 88),
                    format!("Unload unavailable: {reason}"),
                );
            }

            ui.horizontal(|ui| {
                if ui
                    .add_enabled(
                        selected_row.can_load,
                        egui::Button::new("Load from Storage"),
                    )
                    .clicked()
                {
                    kpi.record_manual_action(sim.simulation.tick());
                    match sim.simulation.player_load_from_station_storage(
                        ship_id,
                        card.station_id,
                        selected_row.commodity,
                        station_ui
                            .storage_quantity
                            .min(selected_row.load_cap.max(0.0)),
                    ) {
                        Ok(()) => messages.push(format!(
                            "Loaded {:.1} {} from station storage",
                            station_ui
                                .storage_quantity
                                .min(selected_row.load_cap.max(0.0)),
                            commodity_label(selected_row.commodity)
                        )),
                        Err(err) => messages.push(format!(
                            "Storage load failed: {}",
                            storage_transfer_error_label(err)
                        )),
                    }
                }
                if ui
                    .add_enabled(
                        selected_row.can_unload,
                        egui::Button::new("Unload to Storage"),
                    )
                    .clicked()
                {
                    kpi.record_manual_action(sim.simulation.tick());
                    match sim.simulation.player_unload_to_station_storage(
                        ship_id,
                        card.station_id,
                        selected_row.commodity,
                        station_ui
                            .storage_quantity
                            .min(selected_row.unload_cap.max(0.0)),
                    ) {
                        Ok(()) => messages.push(format!(
                            "Unloaded {:.1} {} to station storage",
                            station_ui
                                .storage_quantity
                                .min(selected_row.unload_cap.max(0.0)),
                            commodity_label(selected_row.commodity)
                        )),
                        Err(err) => messages.push(format!(
                            "Storage unload failed: {}",
                            storage_transfer_error_label(err)
                        )),
                    }
                }
            });
        });
}

fn render_station_missions_tab(
    ui: &mut egui::Ui,
    _sim: &mut ResMut<SimResource>,
    _kpi: &mut ResMut<UiKpiTracker>,
    _messages: &mut ResMut<HudMessages>,
    _panels: &mut ResMut<UiPanelState>,
    mut missions_panel: &mut ResMut<MissionsPanelState>,
    _station_ui: &mut ResMut<StationUiState>,
    _ship_id: ShipId,
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
                                open_mission_offer(&mut missions_panel, row.offer_id);
                            }
                        });
                    ui.add_space(6.0);
                }
            });
    }
}

fn render_ship_card_header(ui: &mut egui::Ui, card: &ShipCardSnapshot) {
    egui::Frame::group(ui.style())
        .fill(egui::Color32::from_rgb(15, 22, 28))
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(66, 90, 108)))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.heading(&card.ship_name);
                    ui.label(
                        egui::RichText::new(format!(
                            "{} ship in {}",
                            ship_class_label(card.ship_class),
                            card.system_name
                        ))
                        .color(egui::Color32::from_rgb(143, 185, 255)),
                    );
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.monospace(format!("Ship #{}", card.ship_id.0));
                    ui.separator();
                    ui.label(
                        egui::RichText::new(format!(
                            "{} ({})",
                            card.owner_name,
                            company_archetype_label(card.owner_archetype)
                        ))
                        .strong(),
                    );
                });
            });
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                ui.monospace(format!("Role {}", ship_role_label(card.role)));
                ui.separator();
                ui.monospace(format!("System {}", card.system_id.0));
                if let Some(station_name) = &card.current_station_name {
                    ui.separator();
                    ui.monospace(format!("Station {station_name}"));
                }
                if let Some(target_name) = &card.target_system_name {
                    ui.separator();
                    ui.monospace(format!("Target {target_name}"));
                }
                ui.separator();
                ui.monospace(format!("ETA {}", card.eta_ticks_remaining));
                ui.separator();
                ui.monospace(format!("Risk {:.2}", card.last_risk_score));
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
    current_mode: crate::input::camera::CameraMode,
    camera: &mut crate::input::camera::CameraUiState,
    kpi: &mut UiKpiTracker,
    current_tick: u64,
) {
    let is_open = matches!(
        current_mode,
        crate::input::camera::CameraMode::System(system_id) if system_id == row.system_id
    );
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

#[allow(clippy::too_many_arguments)]
fn render_system_stations(
    ui: &mut egui::Ui,
    panel: &SystemPanelSnapshot,
    preferred_ship_id: Option<ShipId>,
    current_station_id: Option<gatebound_domain::StationId>,
    simulation: &gatebound_sim::Simulation,
    selected_station: &mut SelectedStation,
    panels: &mut UiPanelState,
    station_ui: &mut StationUiState,
    kpi: &mut UiKpiTracker,
) {
    egui::Frame::group(ui.style())
        .fill(egui::Color32::from_rgb(17, 23, 29))
        .show(ui, |ui| {
            ui.heading("Stations");
            if panel.stations.is_empty() {
                ui.small("No stations in this system");
                return;
            }

            for station in &panel.stations {
                let response = ui.add_sized(
                    [ui.available_width(), 72.0],
                    egui::Button::new(system_station_button_text(station))
                        .selected(current_station_id == Some(station.station_id)),
                );
                if response.clicked() {
                    let preferred = preferred_trade_commodity(
                        simulation,
                        preferred_ship_id,
                        station.station_id,
                        station_ui.trade_commodity,
                    );
                    open_system_station_inspector_selection(
                        selected_station,
                        panels,
                        station_ui,
                        station.station_id,
                        Some(preferred),
                    );
                    kpi.record_manual_action(simulation.tick());
                }
                ui.add_space(4.0);
            }
        });
}

fn render_system_ships(
    ui: &mut egui::Ui,
    panel: &SystemPanelSnapshot,
    current_ship_id: Option<ShipId>,
    tick: u64,
    selected_ship: &mut SelectedShip,
    ship_ui: &mut ShipUiState,
    kpi: &mut UiKpiTracker,
) {
    egui::Frame::group(ui.style())
        .fill(egui::Color32::from_rgb(17, 22, 28))
        .show(ui, |ui| {
            ui.heading("Ships");
            if panel.ships.is_empty() {
                ui.small("No ships currently in this system");
                return;
            }

            for ship in &panel.ships {
                let response = ui.add_sized(
                    [ui.available_width(), 54.0],
                    egui::Button::new(system_ship_button_text(ship))
                        .selected(current_ship_id == Some(ship.ship_id)),
                );
                if response.clicked() {
                    open_system_ship_inspector_selection(selected_ship, ship_ui, ship.ship_id);
                    kpi.record_manual_action(tick);
                }
                ui.add_space(4.0);
            }
        });
}

fn system_station_button_text(station: &SystemStationSnapshot) -> String {
    let trading = format!(
        "PI {:.2}  Cov {:.2}",
        station.price_index, station.stock_coverage
    );
    let commodity_pair = format!(
        "Buy {}  Sell {}",
        station
            .best_buy_commodity
            .map(commodity_label)
            .unwrap_or("-"),
        station
            .best_sell_commodity
            .map(commodity_label)
            .unwrap_or("-"),
    );
    let imbalances = format!(
        "Short {}  Surplus {}",
        station
            .strongest_shortage_commodity
            .map(commodity_label)
            .unwrap_or("-"),
        station
            .strongest_surplus_commodity
            .map(commodity_label)
            .unwrap_or("-"),
    );
    let population = format!(
        "Pop {}  {:.0}%  {}",
        format_population(station.population),
        station.population_ratio * 100.0,
        population_trend_label(station.population_trend)
    );
    format!(
        "{} ({})\n{}\n{} • {} • {} • {} • {}",
        station.station_name,
        station_profile_label(station.profile),
        population,
        station.host_body_name,
        station.orbit_label,
        trading,
        imbalances,
        commodity_pair
    )
}

fn system_ship_button_text(ship: &SystemShipSnapshot) -> String {
    format!(
        "{} ({})\n{} • {} • {} • Risk {:.2} • Reroutes {}",
        ship.ship_name,
        ship_class_label(ship.ship_class),
        ship.owner_name,
        ship_role_label(ship.role),
        ship.status_text,
        ship.last_risk_score,
        ship.reroutes
    )
}

fn render_ship_overview_tab(ui: &mut egui::Ui, card: &ShipCardSnapshot) {
    ui.columns(2, |columns| {
        columns[0].vertical(|ui| {
            egui::Frame::group(ui.style())
                .fill(egui::Color32::from_rgb(18, 26, 34))
                .show(ui, |ui| {
                    ui.heading("Ship Brief");
                    ui.label(&card.description);
                    ui.add_space(6.0);
                    ui.monospace(format!("Owner: {}", card.owner_name));
                    ui.monospace(format!(
                        "Owner type: {}",
                        company_archetype_label(card.owner_archetype)
                    ));
                    ui.monospace(format!("Role: {}", ship_role_label(card.role)));
                    ui.monospace(
                        card.current_station_name
                            .as_ref()
                            .map(|name| format!("Dock: {name}"))
                            .unwrap_or_else(|| "Dock: in transit".to_string()),
                    );
                    ui.monospace(
                        card.target_system_name
                            .as_ref()
                            .map(|name| format!("Target: {name}"))
                            .unwrap_or_else(|| "Target: -".to_string()),
                    );
                    ui.monospace(
                        card.current_segment_kind
                            .map(|kind| format!("Segment: {kind:?}"))
                            .unwrap_or_else(|| "Segment: idle".to_string()),
                    );
                });
        });
        columns[1].vertical(|ui| {
            egui::Frame::group(ui.style())
                .fill(egui::Color32::from_rgb(24, 24, 29))
                .show(ui, |ui| {
                    ui.heading("Autopilot Policy");
                    ui.monospace(format!("min_margin={:.1}", card.policy.min_margin));
                    ui.monospace(format!("max_risk={:.1}", card.policy.max_risk_score));
                    ui.monospace(format!("max_hops={}", card.policy.max_hops));
                    ui.monospace(format!(
                        "priority={}",
                        priority_mode_label(card.policy.priority_mode)
                    ));
                    ui.monospace(format!("route_len={}", card.route_len));
                    ui.monospace(format!("reroutes={}", card.reroutes));
                });
        });
    });
}

fn render_ship_cargo_tab(ui: &mut egui::Ui, card: &ShipCardSnapshot) {
    egui::Frame::group(ui.style())
        .fill(egui::Color32::from_rgb(14, 19, 24))
        .show(ui, |ui| {
            ui.heading("Cargo Hold");
            ui.monospace(format!(
                "Usage: {:.1} / {:.1}",
                card.cargo_total_amount, card.cargo_capacity
            ));
            ui.monospace(format!("Summary: {}", cargo_summary_line(&card.cargo_lots)));
            if card.cargo_lots.is_empty() {
                ui.monospace("Load: empty");
            } else {
                ui.add_space(6.0);
                egui::Grid::new("ship_cargo_grid")
                    .num_columns(3)
                    .striped(true)
                    .show(ui, |ui| {
                        ui.strong("Commodity");
                        ui.strong("Amount");
                        ui.strong("Source");
                        ui.end_row();

                        for cargo in sorted_cargo_lots(&card.cargo_lots) {
                            ui.monospace(commodity_label(cargo.commodity));
                            ui.monospace(format!("{:.1}", cargo.amount));
                            ui.monospace(cargo_source_label(cargo.source));
                            ui.end_row();
                        }
                    });
            }
        });
}

fn render_ship_modules_tab(ui: &mut egui::Ui, card: &ShipCardSnapshot) {
    egui::Frame::group(ui.style())
        .fill(egui::Color32::from_rgb(18, 23, 28))
        .show(ui, |ui| {
            ui.heading("Installed Modules");
            egui::Grid::new("ship_modules_grid")
                .num_columns(3)
                .striped(true)
                .show(ui, |ui| {
                    ui.strong("Slot");
                    ui.strong("Module");
                    ui.strong("Status");
                    ui.end_row();
                    for module in &card.modules {
                        ui.monospace(ship_module_slot_label(module.slot));
                        ui.label(&module.name);
                        ui.monospace(ship_module_status_label(module.status));
                        ui.end_row();
                        ui.label("");
                        ui.small(&module.details);
                        ui.label("");
                        ui.end_row();
                    }
                });
        });
}

fn render_ship_technical_tab(ui: &mut egui::Ui, card: &ShipCardSnapshot) {
    egui::Frame::group(ui.style())
        .fill(egui::Color32::from_rgb(20, 20, 24))
        .show(ui, |ui| {
            ui.heading("Technical State");
            render_ship_condition_bar(ui, "Hull", card.technical_state.hull);
            render_ship_condition_bar(ui, "Drive", card.technical_state.drive);
            render_ship_condition_bar(ui, "Reactor", card.technical_state.reactor);
            render_ship_condition_bar(ui, "Sensors", card.technical_state.sensors);
            render_ship_condition_bar(ui, "Cargo bay", card.technical_state.cargo_bay);
            ui.add_space(8.0);
            ui.heading("Maintenance");
            ui.label(&card.technical_state.maintenance_note);
        });
}

fn render_ship_condition_bar(ui: &mut egui::Ui, label: &str, value: f64) {
    ui.horizontal(|ui| {
        ui.monospace(format!("{label:<9}"));
        ui.add(
            egui::ProgressBar::new((value / 100.0).clamp(0.0, 1.0) as f32)
                .desired_width(220.0)
                .text(format!("{value:.0}%")),
        );
    });
}

fn render_markets_dashboard(
    ui: &mut egui::Ui,
    markets: &MarketsDashboardSnapshot,
    markets_ui: &mut MarketsUiState,
    kpi: &mut UiKpiTracker,
    current_tick: u64,
) {
    ui.horizontal_wrapped(|ui| {
        render_market_metric_card(
            ui,
            "Avg Index",
            format!("{:.2}", markets.global_kpis.avg_price_index),
            format!(
                "{} systems / {} stations",
                markets.global_kpis.system_count, markets.global_kpis.station_count
            ),
        );
        render_market_metric_card(
            ui,
            "Coverage",
            format!(
                "{:.0}%",
                markets.global_kpis.aggregate_stock_coverage * 100.0
            ),
            format!(
                "{:.0}/{:.0} stock",
                markets.global_kpis.aggregate_stock, markets.global_kpis.aggregate_target_stock
            ),
        );
        render_market_metric_card(
            ui,
            "Net Flow",
            format!("{:+.1}", markets.global_kpis.aggregate_net_flow),
            "inflow - outflow".to_string(),
        );
        render_market_metric_card(
            ui,
            "Market Share",
            format!("{:.0}%", markets.global_kpis.player_market_share * 100.0),
            format!(
                "{} window flow",
                markets.global_kpis.rolling_window_total_flow
            ),
        );
        render_market_metric_card(
            ui,
            "Market Fee",
            format!("{:.1}%", markets.global_kpis.market_fee_rate * 100.0),
            "read-only analytics".to_string(),
        );
    });

    ui.horizontal_wrapped(|ui| {
        ui.label("Pressure:");
        let mut any_pressure = false;
        for (label, active) in [
            (
                "Gate congestion",
                markets.global_kpis.gate_congestion_active,
            ),
            (
                "Dock congestion",
                markets.global_kpis.dock_congestion_active,
            ),
            ("Fuel shock", markets.global_kpis.fuel_shock_active),
        ] {
            let color = if active {
                any_pressure = true;
                egui::Color32::from_rgb(224, 136, 92)
            } else {
                egui::Color32::from_rgb(112, 168, 135)
            };
            ui.colored_label(color, label);
        }
        if !any_pressure {
            ui.colored_label(egui::Color32::from_rgb(112, 168, 135), "Stable");
        }
    });

    ui.separator();
    ui.columns(2, |columns| {
        columns[0].vertical(|ui| {
            ui.heading("Galaxy Commodity Matrix");
            egui::ScrollArea::both()
                .id_salt("markets_commodity_matrix")
                .max_height(300.0)
                .show(ui, |ui| {
                    egui::Grid::new("markets_commodity_grid")
                        .striped(true)
                        .show(ui, |ui| {
                            ui.strong("Commodity");
                            ui.strong("Avg");
                            ui.strong("Min");
                            ui.strong("Max");
                            ui.strong("Spread");
                            ui.strong("Sys low");
                            ui.strong("Sys high");
                            ui.strong("Stock");
                            ui.strong("Cov");
                            ui.strong("Net");
                            ui.strong("Trend");
                            ui.strong("Next");
                            ui.strong("Base");
                            ui.strong("<");
                            ui.strong(">");
                            ui.end_row();

                            for row in &markets.commodity_rows {
                                let selected = markets_ui.focused_commodity == row.commodity;
                                if ui
                                    .selectable_label(selected, commodity_label(row.commodity))
                                    .clicked()
                                {
                                    markets_ui.focused_commodity = row.commodity;
                                    kpi.record_manual_action(current_tick);
                                }
                                ui.monospace(format!("{:.2}", row.galaxy_avg_price));
                                ui.label(station_ref_label(row.min_price_station.as_ref()));
                                ui.label(station_ref_label(row.max_price_station.as_ref()));
                                ui.monospace(format!(
                                    "{:.2} / {:.0}%",
                                    row.spread_abs,
                                    row.spread_pct * 100.0
                                ));
                                ui.label(system_ref_label(row.cheapest_system.as_ref()));
                                ui.label(system_ref_label(row.priciest_system.as_ref()));
                                ui.monospace(format!("{:.0}", row.total_stock));
                                ui.monospace(format!("{:.0}%", row.stock_coverage * 100.0));
                                ui.monospace(format!("{:+.1}", row.net_flow));
                                ui.monospace(format!("{:+.2}", row.trend_delta));
                                ui.monospace(format!("{:.2}", row.forecast_next_avg));
                                ui.monospace(format!("{:.0}%", row.price_vs_base * 100.0));
                                ui.monospace(row.stations_below_target.to_string());
                                ui.monospace(row.stations_above_target.to_string());
                                ui.end_row();
                            }
                        });
                });

            ui.separator();
            ui.horizontal(|ui| {
                ui.heading("Station Drilldown");
                let selected_station_text = markets
                    .station_detail
                    .as_ref()
                    .map(|detail| format!("{} / {}", detail.station_name, detail.system_name))
                    .unwrap_or_else(|| "No station".to_string());
                egui::ComboBox::from_id_salt("markets_detail_station")
                    .selected_text(selected_station_text)
                    .show_ui(ui, |ui| {
                        for row in &markets.station_anomaly_rows {
                            let label = format!("{} / {}", row.station_name, row.system_name);
                            if ui
                                .selectable_label(
                                    markets_ui.detail_station_id == Some(row.station_id),
                                    label,
                                )
                                .clicked()
                            {
                                markets_ui.detail_station_id = Some(row.station_id);
                                markets_ui.seeded_from_world_selection = true;
                                kpi.record_manual_action(current_tick);
                            }
                        }
                    });
            });

            if let Some(detail) = markets.station_detail.as_ref() {
                render_markets_station_detail(ui, detail);
            } else {
                ui.label("No station detail available");
            }
        });

        columns[1].vertical(|ui| {
            ui.heading(format!(
                "Focused Commodity: {}",
                commodity_label(markets.focused_commodity)
            ));

            ui.group(|ui| {
                ui.heading("System Stress");
                egui::Grid::new("markets_system_stress_grid")
                    .striped(true)
                    .show(ui, |ui| {
                        ui.strong("System");
                        ui.strong("Stress");
                        ui.strong("Index");
                        ui.strong("Cov");
                        ui.strong("Cong");
                        ui.strong("Fuel");
                        ui.end_row();
                        for row in markets.system_stress_rows.iter().take(6) {
                            ui.label(&row.system_name);
                            ui.monospace(format!("{:.2}", row.stress_score));
                            ui.monospace(format!("{:.2}", row.avg_price_index));
                            ui.monospace(format!("{:.0}%", row.stock_coverage * 100.0));
                            ui.monospace(format!("{:.2}", row.congestion));
                            ui.monospace(format!("{:.2}", row.fuel_stress));
                            ui.end_row();
                        }
                    });
            });

            ui.add_space(8.0);
            ui.group(|ui| {
                ui.heading("Hotspots");
                ui.label("Cheapest stations");
                for row in &markets.hotspots.cheapest_stations {
                    ui.monospace(format!(
                        "{} / {}  {:.2}  cov={:.0}%  net={:+.1}",
                        row.station_name,
                        row.system_name,
                        row.price,
                        row.stock_coverage * 100.0,
                        row.net_flow
                    ));
                }
                ui.separator();
                ui.label("Priciest stations");
                for row in &markets.hotspots.priciest_stations {
                    ui.monospace(format!(
                        "{} / {}  {:.2}  cov={:.0}%  net={:+.1}",
                        row.station_name,
                        row.system_name,
                        row.price,
                        row.stock_coverage * 100.0,
                        row.net_flow
                    ));
                }
                ui.separator();
                ui.label("Cheapest systems");
                for row in &markets.hotspots.cheapest_systems {
                    ui.monospace(format!(
                        "{}  {:.2}  cov={:.0}%  net={:+.1}",
                        row.system_name,
                        row.avg_price,
                        row.stock_coverage * 100.0,
                        row.net_flow
                    ));
                }
                ui.separator();
                ui.label("Priciest systems");
                for row in &markets.hotspots.priciest_systems {
                    ui.monospace(format!(
                        "{}  {:.2}  cov={:.0}%  net={:+.1}",
                        row.system_name,
                        row.avg_price,
                        row.stock_coverage * 100.0,
                        row.net_flow
                    ));
                }
            });

            ui.add_space(8.0);
            ui.group(|ui| {
                ui.heading("Station Anomalies");
                egui::ScrollArea::vertical()
                    .id_salt("markets_anomaly_scroll")
                    .max_height(240.0)
                    .show(ui, |ui| {
                        egui::Grid::new("markets_anomaly_grid")
                            .striped(true)
                            .show(ui, |ui| {
                                ui.strong("Station");
                                ui.strong("Pop");
                                ui.strong("Trend");
                                ui.strong("Score");
                                ui.strong("Index");
                                ui.strong("Cov");
                                ui.strong("Dev");
                                ui.end_row();
                                for row in markets.station_anomaly_rows.iter().take(10) {
                                    ui.label(format!("{} / {}", row.station_name, row.system_name));
                                    ui.monospace(format_population(row.population));
                                    ui.monospace(population_trend_label(row.population_trend));
                                    ui.monospace(format!("{:.2}", row.anomaly_score));
                                    ui.monospace(format!("{:.2}", row.price_index));
                                    ui.monospace(format!("{:.0}%", row.stock_coverage * 100.0));
                                    ui.monospace(format!(
                                        "{:.0}%",
                                        row.avg_price_deviation * 100.0
                                    ));
                                    ui.end_row();
                                }
                            });
                    });
            });
        });
    });
}

fn render_markets_station_detail(ui: &mut egui::Ui, detail: &MarketsStationDetailSnapshot) {
    ui.group(|ui| {
        ui.heading(format!("{} / {}", detail.station_name, detail.system_name));
        ui.label(format!(
            "Profile: {}",
            station_profile_label(detail.profile)
        ));
        ui.horizontal_wrapped(|ui| {
            ui.monospace(format!("pop {}", format_population(detail.population)));
            ui.monospace(format!("{:.0}% baseline", detail.population_ratio * 100.0));
            ui.monospace(population_trend_label(detail.population_trend));
            ui.monospace(format!("index {:.2}", detail.price_index));
            ui.monospace(format!("coverage {:.0}%", detail.stock_coverage * 100.0));
            ui.monospace(format!("net {:+.1}", detail.net_flow));
            ui.monospace(format!(
                "deviation {:.0}%",
                detail.avg_price_deviation * 100.0
            ));
        });
        ui.horizontal_wrapped(|ui| {
            ui.label(format!(
                "Best buy: {}",
                detail
                    .best_buy_commodity
                    .map(commodity_label)
                    .unwrap_or("-")
            ));
            ui.separator();
            ui.label(format!(
                "Best sell: {}",
                detail
                    .best_sell_commodity
                    .map(commodity_label)
                    .unwrap_or("-")
            ));
            ui.separator();
            ui.label(format!(
                "Shortage: {}",
                detail
                    .strongest_shortage_commodity
                    .map(commodity_label)
                    .unwrap_or("-")
            ));
            ui.separator();
            ui.label(format!(
                "Surplus: {}",
                detail
                    .strongest_surplus_commodity
                    .map(commodity_label)
                    .unwrap_or("-")
            ));
        });
        ui.separator();
        egui::ScrollArea::vertical()
            .id_salt("markets_station_detail_scroll")
            .max_height(250.0)
            .show(ui, |ui| {
                egui::Grid::new("markets_station_detail_grid")
                    .striped(true)
                    .show(ui, |ui| {
                        ui.strong("Commodity");
                        ui.strong("Local");
                        ui.strong("Galaxy");
                        ui.strong("Delta");
                        ui.strong("Stock");
                        ui.strong("Cov");
                        ui.strong("Net");
                        ui.strong("Trend");
                        ui.strong("Next");
                        ui.end_row();
                        for row in &detail.commodity_rows {
                            ui.label(commodity_label(row.commodity));
                            ui.monospace(format!("{:.2}", row.local_price));
                            ui.monospace(format!("{:.2}", row.galaxy_avg_price));
                            ui.monospace(format!("{:+.2}", row.price_delta));
                            ui.monospace(format!("{:.0}", row.local_stock));
                            ui.monospace(format!("{:.0}%", row.stock_coverage * 100.0));
                            ui.monospace(format!("{:+.1}", row.net_flow));
                            ui.monospace(format!("{:+.2}", row.trend_delta));
                            ui.monospace(format!("{:.2}", row.forecast_next));
                            ui.end_row();
                        }
                    });
            });
    });
}

fn render_market_metric_card(ui: &mut egui::Ui, title: &str, value: String, detail: String) {
    ui.group(|ui| {
        ui.set_min_width(150.0);
        ui.small(title);
        ui.heading(value);
        ui.small(detail);
    });
}

fn station_ref_label(reference: Option<&StationRefSnapshot>) -> String {
    reference
        .map(|station| format!("{} / {}", station.station_name, station.system_name))
        .unwrap_or_else(|| "-".to_string())
}

fn system_ref_label(reference: Option<&SystemRefSnapshot>) -> String {
    reference
        .map(|system| system.system_name.clone())
        .unwrap_or_else(|| "-".to_string())
}

fn tab_button(label: &'static str, selected: bool) -> egui::Button<'static> {
    let mut button = egui::Button::new(label);
    if selected {
        button = button.fill(egui::Color32::from_rgb(51, 86, 117));
    }
    button
}

fn price_tone_color(tone: TradePriceTone) -> egui::Color32 {
    match tone {
        TradePriceTone::Favorable => egui::Color32::from_rgb(112, 214, 147),
        TradePriceTone::Neutral => egui::Color32::from_rgb(198, 202, 208),
        TradePriceTone::Unfavorable => egui::Color32::from_rgb(232, 112, 112),
    }
}

fn sorted_cargo_lots(cargo_lots: &[CargoLoad]) -> Vec<CargoLoad> {
    let mut lots = cargo_lots.to_vec();
    lots.sort_by(|left, right| {
        right
            .amount
            .total_cmp(&left.amount)
            .then_with(|| left.commodity.cmp(&right.commodity))
            .then_with(|| left.source.cmp(&right.source))
    });
    lots
}

fn cargo_summary_line(cargo_lots: &[CargoLoad]) -> String {
    if cargo_lots.is_empty() {
        return "-".to_string();
    }

    let lots = sorted_cargo_lots(cargo_lots);
    let mut parts = lots
        .iter()
        .take(3)
        .map(|cargo| {
            format!(
                "{} {:.1} ({})",
                commodity_label(cargo.commodity),
                cargo.amount,
                cargo_source_label(cargo.source)
            )
        })
        .collect::<Vec<_>>();
    if lots.len() > 3 {
        parts.push(format!("+{} more", lots.len() - 3));
    }
    parts.join(", ")
}

fn has_matching_spot_cargo(cargo_lots: &[CargoLoad], commodity: Commodity) -> bool {
    cargo_lots.iter().any(|cargo| {
        cargo.source == CargoSource::Spot && cargo.commodity == commodity && cargo.amount > 0.0
    })
}

fn buy_disabled_reason(
    docked: bool,
    _cargo_lots: &[CargoLoad],
    row: &gatebound_sim::StationTradeRowView,
) -> Option<&'static str> {
    if !docked {
        return Some("ship must be docked at the station before spot trading is available");
    }
    if row.can_buy {
        return None;
    }
    if row.station_stock + 1e-9 < 0.1 {
        return Some("station stock is below the minimum tradable lot");
    }
    if row.insufficient_capital {
        return Some("insufficient capital for the minimum trade lot");
    }
    Some("the hold has no remaining capacity for this commodity")
}

fn sell_disabled_reason(
    docked: bool,
    cargo_lots: &[CargoLoad],
    row: &gatebound_sim::StationTradeRowView,
) -> Option<&'static str> {
    if !docked {
        return Some("ship must be docked at the station before spot trading is available");
    }
    if row.can_sell {
        return None;
    }
    if has_matching_spot_cargo(cargo_lots, row.commodity) {
        return Some("matching spot cargo is below the minimum trade lot");
    }
    Some("no matching spot cargo is loaded for this row")
}

fn storage_load_disabled_reason(
    docked: bool,
    _cargo_lots: &[CargoLoad],
    row: &gatebound_sim::StationStorageRowView,
) -> Option<&'static str> {
    if !docked {
        return Some("ship must be docked at the station before storage transfer is available");
    }
    if row.can_load {
        return None;
    }
    if row.stored_amount + 1e-9 < 0.1 {
        return Some("this station storage row is below the minimum transferable lot");
    }
    Some("the hold has no remaining capacity for this commodity")
}

fn storage_unload_disabled_reason(
    docked: bool,
    cargo_lots: &[CargoLoad],
    row: &gatebound_sim::StationStorageRowView,
) -> Option<&'static str> {
    if !docked {
        return Some("ship must be docked at the station before storage transfer is available");
    }
    if row.can_unload {
        return None;
    }
    if cargo_lots.is_empty() {
        return Some("ship has no cargo available for storage");
    }
    if has_matching_spot_cargo(cargo_lots, row.commodity) {
        return Some("matching spot cargo is below the minimum transferable lot");
    }
    Some("selected storage row does not match any spot cargo loaded")
}

fn format_population(population: f64) -> String {
    format!("{:.0}", population)
}

fn population_trend_label(trend: PopulationTrend) -> &'static str {
    match trend {
        PopulationTrend::Growing => "Rising",
        PopulationTrend::Stable => "Stable",
        PopulationTrend::Shrinking => "Falling",
    }
}
