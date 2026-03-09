use bevy_egui::egui;
use gatebound_domain::ShipId;

use crate::features::ships::{open_system_ship_inspector_selection, ShipCardTab, ShipUiState};
use crate::runtime::sim::UiKpiTracker;

use super::labels::{
    cargo_source_label, commodity_label, company_archetype_label, priority_mode_label,
    ship_class_label, ship_module_slot_label, ship_module_status_label, ship_role_label,
};
use super::shared::{cargo_summary_line, sorted_cargo_lots, tab_button};
use super::snapshot::{ShipCardSnapshot, SystemPanelSnapshot, SystemShipSnapshot};

pub(super) fn render_ship_window(
    ctx: &egui::Context,
    save_menu_open: bool,
    ship_ui: &mut ShipUiState,
    live_ship_card: Option<&ShipCardSnapshot>,
) {
    if save_menu_open || !ship_ui.card_open {
        return;
    }

    let mut open = ship_ui.card_open;
    egui::Window::new("Ship Card")
        .open(&mut open)
        .default_width(720.0)
        .default_height(560.0)
        .show(ctx, |ui| {
            let Some(card) = live_ship_card else {
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

pub(super) fn render_system_ships(
    ui: &mut egui::Ui,
    panel: &SystemPanelSnapshot,
    current_ship_id: Option<ShipId>,
    tick: u64,
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
                    open_system_ship_inspector_selection(ship_ui, ship.ship_id);
                    kpi.record_manual_action(tick);
                }
                ui.add_space(4.0);
            }
        });
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
