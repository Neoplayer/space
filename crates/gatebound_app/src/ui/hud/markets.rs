use bevy_egui::egui;

use crate::features::markets::MarketsUiState;
use crate::runtime::sim::UiKpiTracker;

use super::labels::{
    commodity_label, format_population, population_trend_label, station_profile_label,
};
use super::snapshot::{
    MarketsDashboardSnapshot, MarketsStationDetailSnapshot, StationRefSnapshot, SystemRefSnapshot,
};

pub(super) fn render_markets_dashboard(
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

pub(super) fn render_markets_window(
    ctx: &egui::Context,
    save_menu_open: bool,
    open: &mut bool,
    markets: &MarketsDashboardSnapshot,
    markets_ui: &mut MarketsUiState,
    kpi: &mut UiKpiTracker,
    current_tick: u64,
) {
    if save_menu_open || !*open {
        return;
    }

    egui::Window::new("Markets")
        .default_width(1120.0)
        .default_height(760.0)
        .open(open)
        .show(ctx, |ui| {
            render_markets_dashboard(ui, markets, markets_ui, kpi, current_tick);
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
