use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use gatebound_core::{
    CargoSource, CommandError, Commodity, ContractActionError, ContractOffer, ContractProgress,
    ContractTypeStageA, FleetWarning, MarketInsightRow, MilestoneStatus, OfferError,
    OfferProblemTag, PriorityMode, ShipId, Simulation, SlotType, StationId, StationProfile,
    SystemId, TradeError,
};

use crate::sim_runtime::{
    apply_offer_filters, derive_cycle_report, ContractsFilterState, OfferSortMode, SelectedShip,
    SelectedStation, SelectedSystem, SimClock, SimResource, StationUiState, UiKpiTracker,
    UiPanelState,
};
use crate::view_mode::CameraMode;

#[derive(Resource, Debug, Clone, Default)]
pub struct HudMessages {
    pub entries: Vec<String>,
}

impl HudMessages {
    pub fn push(&mut self, message: String) {
        self.entries.push(message);
        if self.entries.len() > 8 {
            let drain_len = self.entries.len() - 8;
            self.entries.drain(0..drain_len);
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MarketRow {
    pub commodity: Commodity,
    pub price: f64,
    pub stock: f64,
    pub inflow: f64,
    pub outflow: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HudSnapshot {
    pub tick: u64,
    pub cycle: u64,
    pub capital: f64,
    pub debt: f64,
    pub interest_rate: f64,
    pub reputation: f64,
    pub recovery_events: u32,
    pub active_contracts: usize,
    pub active_ships: usize,
    pub active_leases: usize,
    pub selected_system_id: SystemId,
    pub selected_station_id: Option<StationId>,
    pub selected_station_profile: Option<StationProfile>,
    pub selected_ship_id: Option<ShipId>,
    pub paused: bool,
    pub speed_multiplier: u32,
    pub sla_success_rate: f64,
    pub reroutes: u64,
    pub avg_price_index: f64,
    pub camera_mode: String,
    pub intel_staleness_ticks: u64,
    pub intel_confidence: f64,
    pub contract_lines: Vec<String>,
    pub ship_lines: Vec<String>,
    pub lease_lines: Vec<String>,
    pub lease_market_lines: Vec<String>,
    pub offers: Vec<ContractOffer>,
    pub fleet_rows: Vec<gatebound_core::FleetShipStatus>,
    pub market_rows: Vec<MarketRow>,
    pub system_market_rows: Vec<MarketRow>,
    pub milestones: Vec<MilestoneStatus>,
    pub throughput_rows: Vec<gatebound_core::GateThroughputSnapshot>,
    pub market_share: f64,
    pub market_insights: Vec<MarketInsightRow>,
    pub recovery_actions: Vec<gatebound_core::RecoveryAction>,
    pub manual_actions_per_min: f64,
    pub policy_edits_per_min: f64,
    pub avg_route_hops_player: f64,
}

#[allow(clippy::too_many_arguments)]
pub fn build_hud_snapshot(
    simulation: &Simulation,
    paused: bool,
    speed_multiplier: u32,
    camera_mode: CameraMode,
    selected_system_id: SystemId,
    selected_station_id: Option<StationId>,
    selected_ship_id: Option<ShipId>,
    filters: ContractsFilterState,
    kpi: &UiKpiTracker,
) -> HudSnapshot {
    let cycle_report = derive_cycle_report(simulation);

    let mut contracts: Vec<_> = simulation
        .contracts
        .values()
        .filter(|contract| !contract.completed && !contract.failed)
        .collect();
    contracts.sort_by_key(|contract| contract.id.0);
    let contract_lines = contracts
        .iter()
        .take(8)
        .map(|contract| {
            let kind = match contract.kind {
                ContractTypeStageA::Delivery => "Delivery",
                ContractTypeStageA::Supply => "Supply",
            };
            format!(
                "#{} {kind} {} S{}:A{} -> S{}:A{} qty={:.1} deadline={} miss={}",
                contract.id.0,
                commodity_label(contract.commodity),
                contract.origin.0,
                contract.origin_station.0,
                contract.destination.0,
                contract.destination_station.0,
                contract.quantity,
                contract.deadline_tick,
                contract.missed_cycles,
            )
        })
        .collect::<Vec<_>>();

    let mut ships: Vec<_> = simulation.ships.values().collect();
    ships.sort_by_key(|ship| ship.id.0);
    let ship_lines = ships
        .iter()
        .take(10)
        .map(|ship| {
            let target = ship
                .current_target
                .map(|target| target.0.to_string())
                .unwrap_or_else(|| "-".to_string());
            let segment = ship
                .current_segment_kind
                .map(|kind| format!("{kind:?}"))
                .unwrap_or_else(|| "-".to_string());
            format!(
                "#{} c={} sys={} -> {} eta={} seg={} seg_eta={} risk={:.2} reroutes={}",
                ship.id.0,
                ship.company_id.0,
                ship.location.0,
                target,
                ship.eta_ticks_remaining,
                segment,
                ship.segment_eta_remaining,
                ship.last_risk_score,
                ship.reroutes,
            )
        })
        .collect::<Vec<_>>();

    let mut leases: Vec<_> = simulation.active_leases.iter().collect();
    leases.sort_by_key(|lease| (lease.system_id.0, lease.slot_type, lease.cycles_remaining));
    let lease_lines = leases
        .into_iter()
        .take(10)
        .map(|lease| {
            format!(
                "sys={} {:?} cycles={} price/cycle={:.1}",
                lease.system_id.0, lease.slot_type, lease.cycles_remaining, lease.price_per_cycle
            )
        })
        .collect::<Vec<_>>();

    let lease_market_lines = simulation
        .lease_market_for_system(selected_system_id)
        .into_iter()
        .map(|entry| {
            format!(
                "{} {}/{} @ {:.1}/cycle",
                slot_type_label(entry.slot_type),
                entry.available,
                entry.total,
                entry.price_per_cycle
            )
        })
        .collect::<Vec<_>>();

    let offers = apply_offer_filters(
        simulation
            .contract_offers
            .values()
            .cloned()
            .collect::<Vec<_>>(),
        filters,
    );
    let fleet_rows = simulation.fleet_status();

    let selected_station_id = selected_station_id.or_else(|| {
        simulation
            .world
            .stations_by_system
            .get(&selected_system_id)
            .and_then(|stations| stations.first().copied())
    });
    let selected_station_profile = selected_station_id.and_then(|station_id| {
        simulation
            .world
            .stations
            .iter()
            .find(|station| station.id == station_id)
            .map(|station| station.profile)
    });

    let market_rows = selected_station_id
        .and_then(|station_id| simulation.markets.get(&station_id))
        .map(|market| {
            Commodity::ALL
                .iter()
                .filter_map(|commodity| {
                    market.goods.get(commodity).map(|state| MarketRow {
                        commodity: *commodity,
                        price: state.price,
                        stock: state.stock,
                        inflow: state.cycle_inflow,
                        outflow: state.cycle_outflow,
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let system_market_rows = simulation
        .world
        .stations_by_system
        .get(&selected_system_id)
        .map(|stations| {
            let mut rows = Vec::new();
            for commodity in Commodity::ALL {
                let mut price_sum = 0.0;
                let mut stock_sum = 0.0;
                let mut inflow_sum = 0.0;
                let mut outflow_sum = 0.0;
                let mut count = 0.0;
                for station_id in stations {
                    if let Some(state) = simulation
                        .markets
                        .get(station_id)
                        .and_then(|book| book.goods.get(&commodity))
                    {
                        price_sum += state.price;
                        stock_sum += state.stock;
                        inflow_sum += state.cycle_inflow;
                        outflow_sum += state.cycle_outflow;
                        count += 1.0;
                    }
                }
                if count > 0.0 {
                    rows.push(MarketRow {
                        commodity,
                        price: price_sum / count,
                        stock: stock_sum,
                        inflow: inflow_sum,
                        outflow: outflow_sum,
                    });
                }
            }
            rows
        })
        .unwrap_or_default();

    let intel = simulation.market_intel(
        selected_system_id,
        matches!(camera_mode, CameraMode::System(system_id) if system_id == selected_system_id),
    );

    let mut throughput_rows = simulation.gate_throughput_view();
    throughput_rows.sort_by(|a, b| b.player_share.total_cmp(&a.player_share));

    let milestones = simulation.milestone_status().to_vec();
    let market_share = simulation.market_share_view();
    let market_insights = selected_station_id
        .map(|station_id| simulation.market_insights(station_id))
        .unwrap_or_default();
    let recovery_actions = simulation.recent_recovery_actions().to_vec();

    let mut price_samples = 0_u64;
    let mut total_price_index = 0.0_f64;
    for market in simulation.markets.values() {
        for state in market.goods.values() {
            if state.base_price > 0.0 {
                total_price_index += state.price / state.base_price;
                price_samples += 1;
            }
        }
    }
    let avg_price_index = if price_samples == 0 {
        1.0
    } else {
        total_price_index / price_samples as f64
    };

    HudSnapshot {
        tick: simulation.tick,
        cycle: simulation.cycle,
        capital: simulation.capital,
        debt: simulation.outstanding_debt,
        interest_rate: simulation.current_loan_interest_rate,
        reputation: simulation.reputation,
        recovery_events: simulation.recovery_events,
        active_contracts: contracts.len(),
        active_ships: simulation.ships.len(),
        active_leases: simulation.active_leases.len(),
        selected_system_id,
        selected_station_id,
        selected_station_profile,
        selected_ship_id,
        paused,
        speed_multiplier,
        sla_success_rate: cycle_report.sla_success_rate,
        reroutes: simulation.reroute_count,
        avg_price_index,
        camera_mode: match camera_mode {
            CameraMode::Galaxy => "Galaxy".to_string(),
            CameraMode::System(system_id) => format!("System({})", system_id.0),
        },
        intel_staleness_ticks: intel.map_or(0, |info| info.staleness_ticks),
        intel_confidence: intel.map_or(1.0, |info| info.confidence),
        contract_lines,
        ship_lines,
        lease_lines,
        lease_market_lines,
        offers,
        fleet_rows,
        market_rows,
        system_market_rows,
        milestones,
        throughput_rows,
        market_share,
        market_insights,
        recovery_actions,
        manual_actions_per_min: kpi.manual_actions_per_min,
        policy_edits_per_min: kpi.policy_edits_per_min,
        avg_route_hops_player: kpi.avg_route_hops_player,
    }
}

#[allow(clippy::too_many_arguments)]
pub fn draw_hud_panel(
    mut egui_contexts: EguiContexts,
    mut sim: ResMut<SimResource>,
    clock: Res<SimClock>,
    camera: Res<crate::view_mode::CameraUiState>,
    selected_system: Res<SelectedSystem>,
    selected_station: Res<SelectedStation>,
    mut selected_ship: ResMut<SelectedShip>,
    mut filters: ResMut<ContractsFilterState>,
    mut panels: ResMut<UiPanelState>,
    mut kpi: ResMut<UiKpiTracker>,
    mut messages: ResMut<HudMessages>,
    mut station_ui: ResMut<StationUiState>,
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
            ui.label("Z/X/C/V: lease Dock/Storage/Factory/Market");
            ui.label("R: release one lease of last selected slot type");
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
                    selected_ship.ship_id = sim
                        .simulation
                        .ships
                        .values()
                        .filter(|ship| ship.company_id.0 == 0)
                        .map(|ship| ship.id)
                        .min_by_key(|ship_id| ship_id.0);
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
                let docked = sim.simulation.is_ship_docked_at(ship_id, station_id);
                ui.label(format!("Ship #{} docked={}", ship_id.0, docked));
                if ui.button("Fly to station").clicked() {
                    kpi.record_manual_action(sim.simulation.tick);
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
                        for edge in &sim.simulation.world.edges {
                            ui.selectable_value(
                                &mut filters.route_gate,
                                Some(edge.id),
                                format!("Gate {}", edge.id.0),
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
                    let gates = if offer.route_gate_ids.is_empty() {
                        "-".to_string()
                    } else {
                        offer
                            .route_gate_ids
                            .iter()
                            .map(|gate_id| gate_id.0.to_string())
                            .collect::<Vec<_>>()
                            .join(">")
                    };
                    let intel = sim
                        .simulation
                        .market_intel(offer.destination, false)
                        .map(|info| format!("s={} c={:.2}", info.staleness_ticks, info.confidence))
                        .unwrap_or_else(|| "s=0 c=1.00".to_string());
                    ui.horizontal(|ui| {
                        ui.monospace(format!(
                            "#{:03} {:?} {} S{}:A{}->S{}:A{} qty={:.1} eta={} risk={:.2} margin={:.1} ppt={:.2} problem={} gates={} intel={}{}",
                            offer.id,
                            offer.kind,
                            commodity_label(offer.commodity),
                            offer.origin.0,
                            offer.origin_station.0,
                            offer.destination.0,
                            offer.destination_station.0,
                            offer.quantity,
                            offer.eta_ticks,
                            offer.risk_score,
                            offer.margin_estimate,
                            offer.profit_per_ton,
                            problem_label(offer.problem_tag),
                            gates,
                            intel,
                            if offer.premium { " premium" } else { "" }
                        ));
                        if let Some(ship_id) = selected_ship.ship_id {
                            if ui.button("Accept").clicked() {
                                kpi.record_manual_action(sim.simulation.tick);
                                match sim.simulation.accept_contract_offer(offer.id, ship_id) {
                                    Ok(contract_id) => messages.push(format!(
                                        "Accepted offer {} as contract {} for ship {}",
                                        offer.id, contract_id.0, ship_id.0
                                    )),
                                    Err(err) => messages.push(format!(
                                        "Accept offer {} failed: {}",
                                        offer.id,
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
                    selected_ship.ship_id = sim
                        .simulation
                        .ships
                        .values()
                        .filter(|ship| ship.company_id.0 == 0)
                        .map(|ship| ship.id)
                        .min_by_key(|ship_id| ship_id.0);
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

                ui.label(format!("Station: {}", station_id.0));
                let docked = sim.simulation.is_ship_docked_at(ship_id, station_id);
                ui.label(format!("Ship #{} docked={}", ship_id.0, docked));

                if let Some(ship) = sim.simulation.ships.get(&ship_id) {
                    let cargo_line = ship
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
                    if let Some(contract_id) = ship.active_contract {
                        let contract_line = sim
                            .simulation
                            .contracts
                            .get(&contract_id)
                            .map(|contract| {
                                format!(
                                    "#{} {:?} {} progress={}",
                                    contract_id.0,
                                    contract.kind,
                                    commodity_label(contract.commodity),
                                    contract_progress_label(contract.progress)
                                )
                            })
                            .unwrap_or_else(|| format!("#{} <missing>", contract_id.0));
                        ui.label(format!("Active contract: {contract_line}"));
                    } else {
                        ui.label("Active contract: -");
                    }
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

                let station_stock = sim
                    .simulation
                    .markets
                    .get(&station_id)
                    .and_then(|book| book.goods.get(&station_ui.trade_commodity))
                    .map(|state| state.stock)
                    .unwrap_or(0.0);
                let mut free_capacity = sim
                    .simulation
                    .ships
                    .get(&ship_id)
                    .map(|ship| match ship.cargo {
                        None => ship.cargo_capacity,
                        Some(cargo)
                            if cargo.source == CargoSource::Spot
                                && cargo.commodity == station_ui.trade_commodity =>
                        {
                            (ship.cargo_capacity - cargo.amount).max(0.0)
                        }
                        _ => 0.0,
                    })
                    .unwrap_or(0.0);
                if free_capacity < 0.0 {
                    free_capacity = 0.0;
                }
                let buy_cap = station_stock.min(free_capacity).max(0.0);
                let sell_cap = sim
                    .simulation
                    .ships
                    .get(&ship_id)
                    .and_then(|ship| ship.cargo)
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
                    if ui.add_enabled(docked, egui::Button::new("Buy")).clicked() {
                        kpi.record_manual_action(sim.simulation.tick);
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
                    if ui.add_enabled(docked, egui::Button::new("Sell")).clicked() {
                        kpi.record_manual_action(sim.simulation.tick);
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

                let active_contract = sim
                    .simulation
                    .ships
                    .get(&ship_id)
                    .and_then(|ship| ship.active_contract);
                let has_contract = active_contract.is_some();
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(docked && has_contract, egui::Button::new("Load contract"))
                        .clicked()
                    {
                        if let Some(contract_id) = active_contract {
                            kpi.record_manual_action(sim.simulation.tick);
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
                        .add_enabled(docked && has_contract, egui::Button::new("Unload contract"))
                        .clicked()
                    {
                        if let Some(contract_id) = active_contract {
                            kpi.record_manual_action(sim.simulation.tick);
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
        egui::Window::new("Assets / Real Estate")
            .open(&mut open)
            .show(ctx, |ui| {
                ui.heading("Economy Pressure");
                ui.label(format!("Debt: {:.1}", snapshot.debt));
                ui.label(format!("Rate: {:.2}%", snapshot.interest_rate * 100.0));
                ui.label(format!("Reputation: {:.2}", snapshot.reputation));
                ui.label(format!("Recovery events: {}", snapshot.recovery_events));
                ui.separator();
                ui.heading("Leases");
                ui.label(format!("Active leases: {}", snapshot.active_leases));
                let lease_burden = sim
                    .simulation
                    .active_leases
                    .iter()
                    .map(|lease| lease.price_per_cycle)
                    .sum::<f64>();
                let offers_avg = if snapshot.offers.is_empty() {
                    0.0
                } else {
                    snapshot
                        .offers
                        .iter()
                        .map(|offer| offer.payout)
                        .sum::<f64>()
                        / snapshot.offers.len() as f64
                };
                let roi_proxy = offers_avg - lease_burden;
                ui.label(format!(
                    "Lease burden/cycle: {:.1} | ROI proxy: {:.1}",
                    lease_burden, roi_proxy
                ));
                for line in &snapshot.lease_lines {
                    ui.monospace(line);
                }
                ui.separator();
                ui.label(format!(
                    "Selected system: {}",
                    snapshot.selected_system_id.0
                ));
                for line in &snapshot.lease_market_lines {
                    ui.monospace(line);
                }
                ui.separator();
                ui.heading("Recovery log");
                for action in snapshot.recovery_actions.iter().rev().take(6) {
                    ui.monospace(format!(
                        "cycle={} released={} capital={:.1} debt={:.1}",
                        action.cycle,
                        action.released_leases,
                        action.capital_after,
                        action.debt_after
                    ));
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
                    selected_ship.ship_id = sim
                        .simulation
                        .ships
                        .values()
                        .filter(|ship| ship.company_id.0 == 0)
                        .map(|ship| ship.id)
                        .min_by_key(|ship_id| ship_id.0);
                }
                let Some(ship_id) = selected_ship.ship_id else {
                    ui.label("No player ship available");
                    return;
                };
                ui.label(format!("Selected ship: #{}", ship_id.0));
                let tick_now = sim.simulation.tick;
                if let Some(ship) = sim.simulation.ships.get_mut(&ship_id) {
                    let mut policy_changed = false;
                    ui.horizontal(|ui| {
                        ui.label("min_margin");
                        policy_changed |= ui
                            .add(egui::DragValue::new(&mut ship.policy.min_margin).speed(0.1))
                            .changed();
                        ui.label("max_risk");
                        policy_changed |= ui
                            .add(egui::DragValue::new(&mut ship.policy.max_risk_score).speed(0.1))
                            .changed();
                        ui.label("max_hops");
                        policy_changed |= ui
                            .add(egui::DragValue::new(&mut ship.policy.max_hops).speed(1.0))
                            .changed();
                    });
                    egui::ComboBox::from_label("priority_mode")
                        .selected_text(priority_mode_label(ship.policy.priority_mode))
                        .show_ui(ui, |ui| {
                            policy_changed |= ui
                                .selectable_value(
                                    &mut ship.policy.priority_mode,
                                    PriorityMode::Profit,
                                    priority_mode_label(PriorityMode::Profit),
                                )
                                .changed();
                            policy_changed |= ui
                                .selectable_value(
                                    &mut ship.policy.priority_mode,
                                    PriorityMode::Stability,
                                    priority_mode_label(PriorityMode::Stability),
                                )
                                .changed();
                            policy_changed |= ui
                                .selectable_value(
                                    &mut ship.policy.priority_mode,
                                    PriorityMode::Hybrid,
                                    priority_mode_label(PriorityMode::Hybrid),
                                )
                                .changed();
                        });
                    if policy_changed {
                        kpi.record_manual_action(tick_now);
                        kpi.record_policy_edit(tick_now);
                    }
                    ui.label(format!(
                        "waypoints={}",
                        ship.policy
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

fn commodity_label(commodity: Commodity) -> &'static str {
    match commodity {
        Commodity::Ore => "Ore",
        Commodity::Ice => "Ice",
        Commodity::Gas => "Gas",
        Commodity::Metal => "Metal",
        Commodity::Fuel => "Fuel",
        Commodity::Parts => "Parts",
        Commodity::Electronics => "Electronics",
    }
}

fn slot_type_label(slot_type: SlotType) -> &'static str {
    match slot_type {
        SlotType::Dock => "Dock",
        SlotType::Storage => "Storage",
        SlotType::Factory => "Factory",
        SlotType::Market => "Market",
    }
}

fn station_profile_label(profile: StationProfile) -> &'static str {
    match profile {
        StationProfile::Civilian => "Civilian",
        StationProfile::Industrial => "Industrial",
        StationProfile::Research => "Research",
    }
}

fn warning_label(warning: FleetWarning) -> &'static str {
    match warning {
        FleetWarning::HighRisk => "HighRisk",
        FleetWarning::HighQueueDelay => "HighQueueDelay",
        FleetWarning::NoRoute => "NoRoute",
        FleetWarning::ShipIdle => "ShipIdle",
    }
}

fn ship_role_label(role: gatebound_core::ShipRole) -> &'static str {
    match role {
        gatebound_core::ShipRole::PlayerContract => "player_contract",
        gatebound_core::ShipRole::NpcTrade => "npc_trade",
    }
}

fn milestone_label(milestone: &MilestoneStatus) -> &'static str {
    match milestone.id {
        gatebound_core::MilestoneId::Capital => "Capital",
        gatebound_core::MilestoneId::MarketShare => "MarketShare",
        gatebound_core::MilestoneId::ThroughputControl => "ThroughputControl",
        gatebound_core::MilestoneId::Reputation => "Reputation",
    }
}

fn sort_mode_label(mode: OfferSortMode) -> &'static str {
    match mode {
        OfferSortMode::MarginDesc => "Margin desc",
        OfferSortMode::RiskAsc => "Risk asc",
        OfferSortMode::EtaAsc => "ETA asc",
    }
}

fn priority_mode_label(mode: PriorityMode) -> &'static str {
    match mode {
        PriorityMode::Profit => "profit",
        PriorityMode::Stability => "stability",
        PriorityMode::Hybrid => "hybrid",
    }
}

fn offer_error_label(err: OfferError) -> &'static str {
    match err {
        OfferError::UnknownOffer => "unknown_offer",
        OfferError::ExpiredOffer => "expired_offer",
        OfferError::ShipBusy => "ship_busy",
        OfferError::InvalidAssignment => "invalid_assignment",
        OfferError::InsufficientStock => "insufficient_stock",
    }
}

fn command_error_label(err: CommandError) -> &'static str {
    match err {
        CommandError::UnknownShip => "unknown_ship",
        CommandError::UnknownStation => "unknown_station",
        CommandError::InvalidAssignment => "invalid_assignment",
        CommandError::ShipBusy => "ship_busy",
        CommandError::NoRoute => "no_route",
    }
}

fn trade_error_label(err: TradeError) -> &'static str {
    match err {
        TradeError::UnknownShip => "unknown_ship",
        TradeError::UnknownStation => "unknown_station",
        TradeError::InvalidAssignment => "invalid_assignment",
        TradeError::NotDocked => "not_docked",
        TradeError::InvalidQuantity => "invalid_quantity",
        TradeError::InsufficientStock => "insufficient_stock",
        TradeError::InsufficientCapital => "insufficient_capital",
        TradeError::InsufficientCargo => "insufficient_cargo",
        TradeError::CargoCapacityExceeded => "cargo_capacity_exceeded",
        TradeError::CommodityMismatch => "commodity_mismatch",
        TradeError::ContractCargoLocked => "contract_cargo_locked",
    }
}

fn contract_action_error_label(err: ContractActionError) -> &'static str {
    match err {
        ContractActionError::UnknownShip => "unknown_ship",
        ContractActionError::UnknownContract => "unknown_contract",
        ContractActionError::InvalidAssignment => "invalid_assignment",
        ContractActionError::NotAssignedShip => "not_assigned_ship",
        ContractActionError::NotDocked => "not_docked",
        ContractActionError::InvalidQuantity => "invalid_quantity",
        ContractActionError::ContractState => "contract_state",
        ContractActionError::InsufficientStock => "insufficient_stock",
        ContractActionError::InsufficientCargo => "insufficient_cargo",
        ContractActionError::CargoCapacityExceeded => "cargo_capacity_exceeded",
        ContractActionError::CommodityMismatch => "commodity_mismatch",
    }
}

fn contract_progress_label(progress: ContractProgress) -> &'static str {
    match progress {
        ContractProgress::AwaitPickup => "await_pickup",
        ContractProgress::InTransit => "in_transit",
        ContractProgress::Completed => "completed",
        ContractProgress::Failed => "failed",
    }
}

fn cargo_source_label(source: CargoSource) -> &'static str {
    match source {
        CargoSource::Spot => "spot",
        CargoSource::Contract { .. } => "contract",
    }
}

fn problem_label(problem: OfferProblemTag) -> &'static str {
    match problem {
        OfferProblemTag::HighRisk => "high_risk",
        OfferProblemTag::CongestedRoute => "congested_route",
        OfferProblemTag::LowMargin => "low_margin",
        OfferProblemTag::FuelVolatility => "fuel_volatility",
    }
}

fn job_kind_label(kind: gatebound_core::FleetJobKind) -> &'static str {
    match kind {
        gatebound_core::FleetJobKind::Pickup => "pickup",
        gatebound_core::FleetJobKind::Transit => "transit",
        gatebound_core::FleetJobKind::GateQueue => "gate_queue",
        gatebound_core::FleetJobKind::Warp => "warp",
        gatebound_core::FleetJobKind::Unload => "unload",
        gatebound_core::FleetJobKind::LoopReturn => "loop_return",
    }
}
