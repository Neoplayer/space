use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use gatebound_domain::{CargoLoad, CargoSource, Commodity, PriorityMode};
use gatebound_sim::TradePriceTone;

use crate::features::finance::FinanceUiState;
use crate::features::markets::{seed_markets_ui_state, MarketsUiState};
use crate::features::missions::MissionsPanelState;
use crate::features::ships::{open_ship_card, ShipUiState};
use crate::features::stations::{open_station_card, StationUiState};
use crate::runtime::save::{
    apply_loaded_simulation, format_save_timestamp, refresh_save_entries, toggle_save_menu,
    toggle_save_menu_with_storage, PendingSaveAction, SaveMenuState, SaveStorage,
};
use crate::runtime::sim::{
    open_system_view, panel_button_specs, panel_is_open, preferred_trade_commodity, set_time_speed,
    toggle_pause, track_ship, SelectedShip, SelectedStation, SelectedSystem, SimClock, SimResource,
    TrackedShip, UiKpiTracker, UiPanelState,
};

use super::finance::render_finance_window;
use super::labels::{
    cargo_source_label, command_error_label, commodity_label, company_archetype_label,
    milestone_label, priority_mode_label, ship_class_label, ship_role_label,
};
use super::markets::render_markets_dashboard;
use super::messages::HudMessages;
use super::missions::{render_missions_windows, MissionHudAccess};
use super::ships::{render_ship_window, render_system_ships};
use super::snapshot::{
    build_hud_snapshot, build_ship_card_snapshot_for_ui, build_station_card_snapshot_for_ui,
    SystemPanelSnapshot, SystemsListRowSnapshot,
};
use super::stations::{render_station_window, render_system_stations, StationHudAccess};

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

    if !save_menu_open && panels.assets {
        let mut open = panels.assets;
        render_finance_window(
            ctx,
            &mut open,
            &snapshot,
            &mut finance_ui,
            &mut sim,
            &mut kpi,
            &mut messages,
        );
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

pub(super) fn tab_button(label: &'static str, selected: bool) -> egui::Button<'static> {
    let mut button = egui::Button::new(label);
    if selected {
        button = button.fill(egui::Color32::from_rgb(51, 86, 117));
    }
    button
}

pub(super) fn price_tone_color(tone: TradePriceTone) -> egui::Color32 {
    match tone {
        TradePriceTone::Favorable => egui::Color32::from_rgb(112, 214, 147),
        TradePriceTone::Neutral => egui::Color32::from_rgb(198, 202, 208),
        TradePriceTone::Unfavorable => egui::Color32::from_rgb(232, 112, 112),
    }
}

pub(super) fn sorted_cargo_lots(cargo_lots: &[CargoLoad]) -> Vec<CargoLoad> {
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

pub(super) fn cargo_summary_line(cargo_lots: &[CargoLoad]) -> String {
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

pub(super) fn buy_disabled_reason(
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

pub(super) fn sell_disabled_reason(
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

pub(super) fn storage_load_disabled_reason(
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

pub(super) fn storage_unload_disabled_reason(
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
