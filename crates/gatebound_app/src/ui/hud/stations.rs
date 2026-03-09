use bevy_egui::egui;
use gatebound_domain::{Commodity, ShipId, StationId, StationProfile};
use gatebound_sim::Simulation;

use crate::features::missions::MissionsPanelState;
use crate::features::stations::{open_station_card, StationCardTab, StationUiState};
use crate::runtime::sim::{
    preferred_trade_commodity, SelectedShip, SelectedStation, SimResource, UiKpiTracker,
    UiPanelState,
};

use super::labels::{
    commodity_label, format_population, population_trend_label, station_profile_label,
    storage_transfer_error_label, trade_error_label,
};
use super::messages::HudMessages;
use super::missions::render_station_missions_tab;
use super::shared::{
    buy_disabled_reason, cargo_summary_line, price_tone_color, sell_disabled_reason,
    storage_load_disabled_reason, storage_unload_disabled_reason, tab_button,
};
use super::snapshot::{
    HudSnapshot, StationCardSnapshot, SystemPanelSnapshot, SystemStationSnapshot,
};

pub(crate) fn open_system_station_panel(
    selected_station: &mut SelectedStation,
    panels: &mut UiPanelState,
    station_ui: &mut StationUiState,
    station_id: StationId,
    preferred_commodity: Option<Commodity>,
) {
    selected_station.station_id = Some(station_id);
    panels.station_ops = true;
    open_station_card(station_ui, station_id, preferred_commodity);
}

#[allow(clippy::too_many_arguments)]
pub(super) fn render_system_stations(
    ui: &mut egui::Ui,
    panel: &SystemPanelSnapshot,
    preferred_ship_id: Option<ShipId>,
    current_station_id: Option<StationId>,
    simulation: &Simulation,
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
                    open_system_station_panel(
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

pub(super) struct StationHudAccess<'a> {
    pub sim: &'a mut SimResource,
    pub selected_ship: &'a mut SelectedShip,
    pub panels: &'a mut UiPanelState,
    pub station_ui: &'a mut StationUiState,
    pub missions_panel: &'a mut MissionsPanelState,
    pub kpi: &'a mut UiKpiTracker,
    pub messages: &'a mut HudMessages,
}

pub(super) fn render_station_window(
    ctx: &egui::Context,
    snapshot: &HudSnapshot,
    save_menu_open: bool,
    live_station_card: Option<&StationCardSnapshot>,
    access: StationHudAccess<'_>,
) {
    if save_menu_open {
        return;
    }

    let StationHudAccess {
        sim,
        selected_ship,
        panels,
        station_ui,
        missions_panel,
        kpi,
        messages,
    } = access;

    if !panels.station_ops || !station_ui.station_panel_open {
        return;
    }

    let mut open = panels.station_ops;
    egui::Window::new("Station Card")
        .open(&mut open)
        .default_width(760.0)
        .default_height(560.0)
        .show(ctx, |ui| {
            if selected_ship.ship_id.is_none() {
                selected_ship.ship_id = snapshot.default_player_ship_id;
            }
            let Some(card) = live_station_card else {
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
                    let Some(ship_id) = selected_ship.ship_id.or(snapshot.default_player_ship_id)
                    else {
                        ui.label("No player ship available");
                        return;
                    };
                    render_station_trade_tab(ui, sim, kpi, messages, station_ui, ship_id, card);
                }
                StationCardTab::Storage => {
                    let Some(ship_id) = selected_ship.ship_id.or(snapshot.default_player_ship_id)
                    else {
                        ui.label("No player ship available");
                        return;
                    };
                    render_station_storage_tab(ui, sim, kpi, messages, station_ui, ship_id, card);
                }
                StationCardTab::Missions => {
                    if selected_ship
                        .ship_id
                        .or(snapshot.default_player_ship_id)
                        .is_none()
                    {
                        ui.label("No player ship available");
                        return;
                    }
                    render_station_missions_tab(ui, missions_panel, card);
                }
            }
        });

    panels.station_ops = open;
    station_ui.station_panel_open = open;
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
                StationProfile::Civilian => {
                    "Civilian concourses keep local demand steady: expect dependable retail pull, moderate fuel burn, and softer bulk margins."
                }
                StationProfile::Industrial => {
                    "Industrial yards reward timing around raw-material shortages and part surpluses; docking windows matter when fabrication queues spike."
                }
                StationProfile::Research => {
                    "Research arrays swing harder on precision goods: electronics and specialist inputs can flip from surplus to shortage in a single cycle."
                }
            });
        });
}

fn render_station_trade_tab(
    ui: &mut egui::Ui,
    sim: &mut SimResource,
    kpi: &mut UiKpiTracker,
    messages: &mut HudMessages,
    station_ui: &mut StationUiState,
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
    sim: &mut SimResource,
    kpi: &mut UiKpiTracker,
    messages: &mut HudMessages,
    station_ui: &mut StationUiState,
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
        "Pop {} ({:.0}% baseline) • {}",
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
