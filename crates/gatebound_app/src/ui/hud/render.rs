use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use gatebound_domain::{CargoSource, Commodity, OfferProblemTag, PriorityMode};

use crate::runtime::sim::{
    ContractsFilterState, FinanceUiState, OfferSortMode, SelectedShip, SelectedStation,
    SelectedSystem, SimClock, SimResource, StationUiState, UiKpiTracker, UiPanelState,
};

use super::labels::{
    cargo_source_label, command_error_label, commodity_label, contract_action_error_label,
    contract_progress_label, credit_error_label, job_kind_label, milestone_label,
    offer_error_label, priority_mode_label, problem_label, ship_role_label, sort_mode_label,
    station_profile_label, trade_error_label, warning_label,
};
use super::messages::HudMessages;
use super::snapshot::build_hud_snapshot;
#[allow(clippy::too_many_arguments)]
pub fn draw_hud_panel(
    mut egui_contexts: EguiContexts,
    mut sim: ResMut<SimResource>,
    clock: Res<SimClock>,
    camera: Res<crate::input::camera::CameraUiState>,
    selected_system: Res<SelectedSystem>,
    selected_station: Res<SelectedStation>,
    mut selected_ship: ResMut<SelectedShip>,
    mut filters: ResMut<ContractsFilterState>,
    mut panels: ResMut<UiPanelState>,
    mut kpi: ResMut<UiKpiTracker>,
    mut messages: ResMut<HudMessages>,
    mut station_ui: ResMut<StationUiState>,
    mut finance_ui: ResMut<FinanceUiState>,
) -> Result {
    let selected_system_id = selected_system.system_id;
    let snapshot = build_hud_snapshot(
        &sim.simulation,
        clock.paused,
        clock.speed_multiplier,
        camera.mode,
        selected_system_id,
        selected_station.station_id,
        selected_ship.ship_id,
        *filters,
        &kpi,
    );

    let ctx = egui_contexts.ctx_mut()?;

    egui::TopBottomPanel::top("gatebound_top_panel").show(ctx, |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.label(format!("View: {}", snapshot.camera_mode));
            ui.separator();
            ui.label(format!("Tick: {}", snapshot.tick));
            ui.separator();
            ui.label(format!("Cycle: {}", snapshot.cycle));
            ui.separator();
            ui.label(format!(
                "Time: {} @ {}x",
                if snapshot.paused { "paused" } else { "running" },
                snapshot.speed_multiplier
            ));
            ui.separator();
            ui.label(format!("Capital: {:.1}", snapshot.capital));
            ui.separator();
            ui.label(format!("Debt: {:.1}", snapshot.debt));
            ui.separator();
            ui.label(format!("Rate: {:.2}%", snapshot.interest_rate * 100.0));
            ui.separator();
            ui.label(format!("Rep: {:.2}", snapshot.reputation));
            ui.separator();
            ui.label(format!("SLA: {:.2}", snapshot.sla_success_rate));
            ui.separator();
            ui.label(format!("Reroutes: {}", snapshot.reroutes));
            ui.separator();
            ui.label(format!("PriceIdx: {:.2}", snapshot.avg_price_index));
            ui.separator();
            ui.label(format!("MShare: {:.2}", snapshot.market_share));
            ui.separator();
            ui.label(format!(
                "Manual/min: {:.1}",
                snapshot.manual_actions_per_min
            ));
            ui.separator();
            ui.label(format!(
                "Ship: {}",
                snapshot
                    .selected_ship_id
                    .map(|ship_id| ship_id.0.to_string())
                    .unwrap_or_else(|| "-".to_string())
            ));
        });
    });

    egui::SidePanel::left("gatebound_left_hud")
        .resizable(true)
        .show(ctx, |ui| {
            ui.heading("Controls");
            ui.label("Space: pause/resume");
            ui.label("1/2/4: sim speed");
            ui.label("Mouse wheel / +/-: zoom");
            ui.label("Double-click system: enter System view");
            ui.label("Esc: back to Galaxy view");
            ui.label("F1..F6: toggle panels");
            ui.label("[ / ]: switch selected player ship");
            ui.label("Right-click station: context menu (Fly / Open station UI)");
            ui.label("Finance panel (F4): take credit, repay partially or fully");
            ui.label("G / D / F: trigger Stage A risk events");
            ui.separator();
            ui.heading("Map Legend");
            ui.label("Edge glow: gate load intensity");
            ui.label("Orange ring: dock congestion");
            ui.label("Red ring: fuel stress");
            ui.label("Sun/orbits/stations shown in System view");

            if !messages.entries.is_empty() {
                ui.separator();
                ui.heading("Events");
                for message in messages.entries.iter().rev() {
                    ui.monospace(message);
                }
            }
        });

    if station_ui.context_menu_open {
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
                if ui.button("Open station UI").clicked() {
                    station_ui.station_panel_open = true;
                    panels.station_ops = true;
                    station_ui.context_menu_open = false;
                }
            });
        station_ui.context_menu_open = open && station_ui.context_menu_open;
    }

    if panels.contracts {
        let mut open = panels.contracts;
        egui::Window::new("Contracts Board")
            .open(&mut open)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Min margin");
                    ui.add(egui::DragValue::new(&mut filters.min_margin).speed(0.5));
                    ui.label("Max risk");
                    ui.add(egui::DragValue::new(&mut filters.max_risk).speed(0.05));
                    ui.label("Max ETA");
                    ui.add(egui::DragValue::new(&mut filters.max_eta).speed(1.0));
                });
                egui::ComboBox::from_label("Commodity")
                    .selected_text(
                        filters
                            .commodity
                            .map(commodity_label)
                            .unwrap_or("Any"),
                    )
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut filters.commodity, None, "Any");
                        for commodity in Commodity::ALL {
                            ui.selectable_value(
                                &mut filters.commodity,
                                Some(commodity),
                                commodity_label(commodity),
                            );
                        }
                    });
                egui::ComboBox::from_label("Route gate")
                    .selected_text(
                        filters
                            .route_gate
                            .map(|gate_id| format!("Gate {}", gate_id.0))
                            .unwrap_or_else(|| "Any".to_string()),
                    )
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut filters.route_gate, None, "Any");
                        for gate_id in &snapshot.route_gate_options {
                            ui.selectable_value(
                                &mut filters.route_gate,
                                Some(*gate_id),
                                format!("Gate {}", gate_id.0),
                            );
                        }
                    });
                egui::ComboBox::from_label("Problem")
                    .selected_text(
                        filters
                            .problem
                            .map(problem_label)
                            .unwrap_or("Any"),
                    )
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut filters.problem, None, "Any");
                        ui.selectable_value(
                            &mut filters.problem,
                            Some(OfferProblemTag::HighRisk),
                            problem_label(OfferProblemTag::HighRisk),
                        );
                        ui.selectable_value(
                            &mut filters.problem,
                            Some(OfferProblemTag::CongestedRoute),
                            problem_label(OfferProblemTag::CongestedRoute),
                        );
                        ui.selectable_value(
                            &mut filters.problem,
                            Some(OfferProblemTag::LowMargin),
                            problem_label(OfferProblemTag::LowMargin),
                        );
                        ui.selectable_value(
                            &mut filters.problem,
                            Some(OfferProblemTag::FuelVolatility),
                            problem_label(OfferProblemTag::FuelVolatility),
                        );
                    });
                ui.checkbox(&mut filters.premium_only, "Premium only");
                egui::ComboBox::from_label("Sort")
                    .selected_text(sort_mode_label(filters.sort_mode))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut filters.sort_mode,
                            OfferSortMode::MarginDesc,
                            sort_mode_label(OfferSortMode::MarginDesc),
                        );
                        ui.selectable_value(
                            &mut filters.sort_mode,
                            OfferSortMode::RiskAsc,
                            sort_mode_label(OfferSortMode::RiskAsc),
                        );
                        ui.selectable_value(
                            &mut filters.sort_mode,
                            OfferSortMode::EtaAsc,
                            sort_mode_label(OfferSortMode::EtaAsc),
                        );
                    });

                ui.separator();
                ui.label(format!("Offers: {}", snapshot.offers.len()));
                for offer in snapshot.offers.iter().take(16) {
                    let gates = if offer.offer.route_gate_ids.is_empty() {
                        "-".to_string()
                    } else {
                        offer
                            .offer
                            .route_gate_ids
                            .iter()
                            .map(|gate_id| gate_id.0.to_string())
                            .collect::<Vec<_>>()
                            .join(">")
                    };
                    let intel = offer
                        .destination_intel
                        .map(|info| format!("s={} c={:.2}", info.staleness_ticks, info.confidence))
                        .unwrap_or_else(|| "s=0 c=1.00".to_string());
                    ui.horizontal(|ui| {
                        ui.monospace(format!(
                            "#{:03} {:?} {} S{}:A{}->S{}:A{} qty={:.1} eta={} risk={:.2} margin={:.1} ppt={:.2} problem={} gates={} intel={}{}",
                            offer.offer.id,
                            offer.offer.kind,
                            commodity_label(offer.offer.commodity),
                            offer.offer.origin.0,
                            offer.offer.origin_station.0,
                            offer.offer.destination.0,
                            offer.offer.destination_station.0,
                            offer.offer.quantity,
                            offer.offer.eta_ticks,
                            offer.offer.risk_score,
                            offer.offer.margin_estimate,
                            offer.offer.profit_per_ton,
                            problem_label(offer.offer.problem_tag),
                            gates,
                            intel,
                            if offer.offer.premium { " premium" } else { "" }
                        ));
                        if let Some(ship_id) = selected_ship.ship_id {
                            if ui.button("Accept").clicked() {
                                kpi.record_manual_action(sim.simulation.tick());
                                match sim
                                    .simulation
                                    .accept_contract_offer(offer.offer.id, ship_id)
                                {
                                    Ok(contract_id) => messages.push(format!(
                                        "Accepted offer {} as contract {} for ship {}",
                                        offer.offer.id, contract_id.0, ship_id.0
                                    )),
                                    Err(err) => messages.push(format!(
                                        "Accept offer {} failed: {}",
                                        offer.offer.id,
                                        offer_error_label(err)
                                    )),
                                }
                            }
                        }
                    });
                }
            });
        panels.contracts = open;
    }

    if panels.fleet {
        let mut open = panels.fleet;
        egui::Window::new("Fleet Manager")
            .open(&mut open)
            .show(ctx, |ui| {
                ui.label(format!("Ships: {}", snapshot.fleet_rows.len()));
                for row in &snapshot.fleet_rows {
                    ui.collapsing(format!("Ship #{}", row.ship_id.0), |ui| {
                        let active_segment = row
                            .job_queue
                            .first()
                            .map(|step| job_kind_label(step.kind))
                            .unwrap_or("-");
                        ui.horizontal(|ui| {
                            let selected = selected_ship.ship_id == Some(row.ship_id);
                            if ui
                                .selectable_label(selected, format!("select #{}", row.ship_id.0))
                                .clicked()
                            {
                                selected_ship.ship_id = Some(row.ship_id);
                            }
                            ui.monospace(format!(
                                "c={} role={} sys={} st={} -> {} eta={} segment={} route={} reroutes={} cargo={}",
                                row.company_id.0,
                                ship_role_label(row.role),
                                row.location.0,
                                row.current_station
                                    .map(|station_id| station_id.0.to_string())
                                    .unwrap_or_else(|| "-".to_string()),
                                row.target
                                    .map(|system_id| system_id.0.to_string())
                                    .unwrap_or_else(|| "-".to_string()),
                                row.eta,
                                active_segment,
                                row.route_len,
                                row.reroutes,
                                row.cargo_commodity
                                    .map(|commodity| format!("{}:{:.1}", commodity_label(commodity), row.cargo_amount))
                                    .unwrap_or_else(|| "-".to_string())
                            ));
                        });
                        ui.monospace(format!(
                            "idle={} delay_avg={:.2} profit/run={:.2}",
                            row.idle_ticks_cycle, row.avg_delay_ticks_cycle, row.profit_per_run
                        ));
                        if let Some(warning) = row.warning {
                            ui.colored_label(
                                egui::Color32::YELLOW,
                                format!("warn={}", warning_label(warning)),
                            );
                        }
                        ui.monospace("job_queue:");
                        for step in row.job_queue.iter().take(8) {
                            ui.monospace(format!(
                                " - {} @sys={} eta={}",
                                job_kind_label(step.kind),
                                step.system.0,
                                step.eta_ticks
                            ));
                        }
                    });
                }
            });
        panels.fleet = open;
    }

    if panels.markets {
        let mut open = panels.markets;
        egui::Window::new("Markets")
            .open(&mut open)
            .show(ctx, |ui| {
                ui.label(format!("System: {}", snapshot.selected_system_id.0));
                ui.label(format!(
                    "Station: {} ({})",
                    snapshot
                        .selected_station_id
                        .map(|station_id| station_id.0.to_string())
                        .unwrap_or_else(|| "-".to_string()),
                    snapshot
                        .selected_station_profile
                        .map(station_profile_label)
                        .unwrap_or("-")
                ));
                ui.label(format!(
                    "Intel: staleness={} ticks, confidence={:.2}",
                    snapshot.intel_staleness_ticks, snapshot.intel_confidence
                ));
                ui.separator();
                ui.heading("Station Market");
                for row in &snapshot.market_rows {
                    ui.monospace(format!(
                        "{:<11} price={:>6.2} stock={:>6.1} in={:>5.1} out={:>5.1}",
                        commodity_label(row.commodity),
                        row.price,
                        row.stock,
                        row.inflow,
                        row.outflow
                    ));
                }
                ui.separator();
                ui.heading("System Aggregate");
                for row in &snapshot.system_market_rows {
                    ui.monospace(format!(
                        "{:<11} avg_price={:>6.2} stock_sum={:>6.1} in_sum={:>5.1} out_sum={:>5.1}",
                        commodity_label(row.commodity),
                        row.price,
                        row.stock,
                        row.inflow,
                        row.outflow
                    ));
                }
                ui.separator();
                ui.heading("Throughput");
                for row in snapshot.throughput_rows.iter().take(6) {
                    ui.monospace(format!(
                        "gate={} player_share={:.2} flow={}",
                        row.gate_id.0, row.player_share, row.total_flow
                    ));
                }
                ui.monospace(format!("market_share={:.2}", snapshot.market_share));
                ui.separator();
                ui.heading("Insights");
                for row in snapshot.market_insights.iter().take(7) {
                    ui.monospace(format!(
                        "{:<11} trend={:+.2} forecast={:.2} imbalance={:+.2} congestion={:.2} fuel={:.2}",
                        commodity_label(row.commodity),
                        row.trend_delta,
                        row.forecast_next,
                        row.imbalance_factor,
                        row.congestion_factor,
                        row.fuel_factor
                    ));
                }
            });
        panels.markets = open;
    }

    if panels.station_ops && station_ui.station_panel_open {
        let mut open = station_ui.station_panel_open;
        egui::Window::new("Station Operations")
            .open(&mut open)
            .show(ctx, |ui| {
                if selected_ship.ship_id.is_none() {
                    selected_ship.ship_id = snapshot.default_player_ship_id;
                }
                let Some(ship_id) = selected_ship.ship_id else {
                    ui.label("No player ship available");
                    return;
                };
                let station_id = station_ui
                    .context_station_id
                    .or(selected_station.station_id)
                    .or(snapshot.selected_station_id);
                let Some(station_id) = station_id else {
                    ui.label("No station selected");
                    return;
                };
                station_ui.context_station_id = Some(station_id);

                let Some(ops_view) = sim.simulation.station_ops_view(ship_id, station_id) else {
                    ui.label("Selected ship not found");
                    return;
                };

                ui.label(format!("Station: {}", station_id.0));
                ui.label(format!("Ship #{} docked={}", ship_id.0, ops_view.docked));

                let cargo_line = ops_view
                    .cargo
                    .map(|cargo| {
                        format!(
                            "{} {:.1} ({})",
                            commodity_label(cargo.commodity),
                            cargo.amount,
                            cargo_source_label(cargo.source)
                        )
                    })
                    .unwrap_or_else(|| "-".to_string());
                ui.label(format!("Cargo: {cargo_line}"));
                if let Some(contract) = ops_view.active_contract.as_ref() {
                    ui.label(format!(
                        "Active contract: #{} {:?} {} progress={}",
                        contract.id.0,
                        contract.kind,
                        commodity_label(contract.commodity),
                        contract_progress_label(contract.progress)
                    ));
                } else {
                    ui.label("Active contract: -");
                }

                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("Commodity");
                    egui::ComboBox::from_id_salt("station_ops_commodity")
                        .selected_text(commodity_label(station_ui.trade_commodity))
                        .show_ui(ui, |ui| {
                            for commodity in Commodity::ALL {
                                ui.selectable_value(
                                    &mut station_ui.trade_commodity,
                                    commodity,
                                    commodity_label(commodity),
                                );
                            }
                        });
                    ui.label("Qty");
                    ui.add(
                        egui::DragValue::new(&mut station_ui.trade_quantity)
                            .speed(0.5)
                            .range(0.1..=10_000.0),
                    );
                });

                let station_stock = ops_view
                    .market_rows
                    .iter()
                    .find(|row| row.commodity == station_ui.trade_commodity)
                    .map(|row| row.stock)
                    .unwrap_or(0.0);
                let mut free_capacity = match ops_view.cargo {
                    None => ops_view.cargo_capacity,
                    Some(cargo)
                        if cargo.source == CargoSource::Spot
                            && cargo.commodity == station_ui.trade_commodity =>
                    {
                        (ops_view.cargo_capacity - cargo.amount).max(0.0)
                    }
                    _ => 0.0,
                };
                if free_capacity < 0.0 {
                    free_capacity = 0.0;
                }
                let buy_cap = station_stock.min(free_capacity).max(0.0);
                let sell_cap = ops_view
                    .cargo
                    .filter(|cargo| {
                        cargo.source == CargoSource::Spot
                            && cargo.commodity == station_ui.trade_commodity
                    })
                    .map(|cargo| cargo.amount)
                    .unwrap_or(0.0);

                ui.horizontal(|ui| {
                    if ui.button("25%").clicked() {
                        station_ui.trade_quantity = (buy_cap * 0.25).max(0.1);
                    }
                    if ui.button("50%").clicked() {
                        station_ui.trade_quantity = (buy_cap * 0.50).max(0.1);
                    }
                    if ui.button("100%").clicked() {
                        station_ui.trade_quantity = buy_cap.max(0.1);
                    }
                });

                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(ops_view.docked, egui::Button::new("Buy"))
                        .clicked()
                    {
                        kpi.record_manual_action(sim.simulation.tick());
                        match sim.simulation.player_buy(
                            ship_id,
                            station_id,
                            station_ui.trade_commodity,
                            station_ui.trade_quantity,
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
                        .add_enabled(ops_view.docked, egui::Button::new("Sell"))
                        .clicked()
                    {
                        kpi.record_manual_action(sim.simulation.tick());
                        match sim.simulation.player_sell(
                            ship_id,
                            station_id,
                            station_ui.trade_commodity,
                            station_ui.trade_quantity.min(sell_cap.max(0.0)),
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

                let active_contract = ops_view
                    .active_contract
                    .as_ref()
                    .map(|contract| contract.id);
                let has_contract = active_contract.is_some();
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(
                            ops_view.docked && has_contract,
                            egui::Button::new("Load contract"),
                        )
                        .clicked()
                    {
                        if let Some(contract_id) = active_contract {
                            kpi.record_manual_action(sim.simulation.tick());
                            match sim.simulation.player_contract_load(
                                ship_id,
                                contract_id,
                                station_ui.trade_quantity,
                            ) {
                                Ok(()) => messages.push(format!(
                                    "Loaded {:.1} to contract {}",
                                    station_ui.trade_quantity, contract_id.0
                                )),
                                Err(err) => messages.push(format!(
                                    "Load failed: {}",
                                    contract_action_error_label(err)
                                )),
                            }
                        }
                    }
                    if ui
                        .add_enabled(
                            ops_view.docked && has_contract,
                            egui::Button::new("Unload contract"),
                        )
                        .clicked()
                    {
                        if let Some(contract_id) = active_contract {
                            kpi.record_manual_action(sim.simulation.tick());
                            match sim.simulation.player_contract_unload(
                                ship_id,
                                contract_id,
                                station_ui.trade_quantity,
                            ) {
                                Ok(()) => messages.push(format!(
                                    "Unloaded {:.1} from contract {}",
                                    station_ui.trade_quantity, contract_id.0
                                )),
                                Err(err) => messages.push(format!(
                                    "Unload failed: {}",
                                    contract_action_error_label(err)
                                )),
                            }
                        }
                    }
                });

                ui.label(format!(
                    "Buy capacity {:.1}, Sell capacity {:.1}",
                    buy_cap, sell_cap
                ));
            });
        station_ui.station_panel_open = open;
    }

    if panels.assets {
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

    if panels.policies {
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

    Ok(())
}
