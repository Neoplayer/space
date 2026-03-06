use super::*;

impl Simulation {
    pub fn time_settings_view(&self) -> TimeSettingsView {
        TimeSettingsView {
            tick_seconds: self.config.time.tick_seconds,
            cycle_ticks: self.config.time.cycle_ticks,
            rolling_window_cycles: self.config.time.rolling_window_cycles,
            day_ticks: self.config.time.day_ticks,
            days_per_month: self.config.time.days_per_month,
            months_per_year: self.config.time.months_per_year,
            start_year: self.config.time.start_year,
        }
    }

    pub fn camera_topology_view(&self) -> CameraTopologyView {
        CameraTopologyView {
            systems: self
                .world
                .systems
                .iter()
                .map(|system| CameraSystemView {
                    system_id: system.id,
                    x: system.x,
                    y: system.y,
                    radius: system.radius,
                    stations: self.camera_stations_for_system(system.id),
                })
                .collect(),
            gate_ids: self.world.edges.iter().map(|edge| edge.id).collect(),
        }
    }

    pub fn world_render_snapshot(&self) -> WorldRenderSnapshot {
        WorldRenderSnapshot {
            tick: self.tick,
            systems: self
                .world
                .systems
                .iter()
                .map(|system| RenderSystemView {
                    system_id: system.id,
                    x: system.x,
                    y: system.y,
                    radius: system.radius,
                    gate_nodes: system
                        .gate_nodes
                        .iter()
                        .map(|gate| RenderGateNodeView {
                            gate_id: gate.gate_id,
                            x: gate.x,
                            y: gate.y,
                        })
                        .collect(),
                    stations: self
                        .camera_stations_for_system(system.id)
                        .into_iter()
                        .map(|station| RenderStationView {
                            station_id: station.station_id,
                            profile: station.profile,
                            x: station.x,
                            y: station.y,
                        })
                        .collect(),
                    dock_congestion: self.dock_congestion_index(system.id),
                    fuel_stress: self.fuel_stress_index(system.id),
                })
                .collect(),
            edges: self
                .world
                .edges
                .iter()
                .map(|edge| {
                    let load = self.gate_queue_load.get(&edge.id).copied().unwrap_or(0.0);
                    let effective_capacity = (edge.base_capacity * edge.capacity_factor).max(1.0);
                    RenderEdgeView {
                        gate_id: edge.id,
                        from_system: edge.a,
                        to_system: edge.b,
                        load,
                        effective_capacity,
                    }
                })
                .collect(),
            ships: self
                .ships
                .values()
                .map(|ship| RenderShipView {
                    ship_id: ship.id,
                    company_id: ship.company_id,
                    location: ship.location,
                    current_station: ship.current_station,
                    current_target: ship.current_target,
                    eta_ticks_remaining: ship.eta_ticks_remaining,
                    segment_eta_remaining: ship.segment_eta_remaining,
                    segment_progress_total: ship.segment_progress_total,
                    current_segment_kind: ship.current_segment_kind,
                    front_segment: ship.movement_queue.front().cloned(),
                    cargo: ship.cargo,
                    last_gate_arrival: ship.last_gate_arrival,
                    last_risk_score: ship.last_risk_score,
                    reroutes: ship.reroutes,
                })
                .collect(),
        }
    }

    pub fn contracts_board_view(&self) -> ContractsBoardView {
        let mut active_contracts = self
            .contracts
            .values()
            .filter(|contract| !contract.completed && !contract.failed)
            .cloned()
            .collect::<Vec<_>>();
        active_contracts.sort_by_key(|contract| contract.id.0);

        let mut offers = self
            .contract_offers
            .values()
            .cloned()
            .map(|offer| ContractOfferView {
                destination_intel: self.market_intel(offer.destination, false),
                offer,
            })
            .collect::<Vec<_>>();
        offers.sort_by_key(|entry| entry.offer.id);

        ContractsBoardView {
            active_contracts,
            route_gates: self.world.edges.iter().map(|edge| edge.id).collect(),
            offers,
        }
    }

    pub fn fleet_panel_view(&self) -> FleetPanelView {
        let mut player_ship_ids = self
            .ships
            .values()
            .filter(|ship| ship.company_id == CompanyId(0))
            .map(|ship| ship.id)
            .collect::<Vec<_>>();
        player_ship_ids.sort_by_key(|ship_id| ship_id.0);

        let avg_route_hops_player = if player_ship_ids.is_empty() {
            0.0
        } else {
            player_ship_ids
                .iter()
                .filter_map(|ship_id| self.ships.get(ship_id))
                .map(|ship| ship.planned_path.len() as f64)
                .sum::<f64>()
                / player_ship_ids.len() as f64
        };

        FleetPanelView {
            rows: self.fleet_status(),
            player_ship_ids,
            default_player_ship_id: self.default_player_ship_id(),
            avg_route_hops_player,
        }
    }

    pub fn market_panel_view(
        &self,
        selected_system_id: SystemId,
        selected_station_id: Option<StationId>,
        local_cluster: bool,
    ) -> MarketPanelView {
        let selected_station_id =
            selected_station_id.or_else(|| self.world.first_station(selected_system_id));
        let selected_station_profile = selected_station_id.and_then(|station_id| {
            self.world
                .stations
                .iter()
                .find(|station| station.id == station_id)
                .map(|station| station.profile)
        });

        let station_market_rows = selected_station_id
            .map(|station_id| self.market_rows_for_station(station_id))
            .unwrap_or_default();

        let system_market_rows = self
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
                        if let Some(state) = self
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
                        rows.push(MarketRowView {
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

        let mut throughput_rows = self.gate_throughput_view();
        throughput_rows.sort_by(|a, b| b.player_share.total_cmp(&a.player_share));

        MarketPanelView {
            selected_system_id,
            selected_station_id,
            selected_station_profile,
            intel: self.market_intel(selected_system_id, local_cluster),
            station_market_rows,
            system_market_rows,
            throughput_rows,
            market_share: self.market_share_view(),
            market_insights: selected_station_id
                .map(|station_id| self.market_insights(station_id))
                .unwrap_or_default(),
            avg_price_index: self.average_price_index(),
        }
    }

    pub fn finance_panel_view(&self) -> FinancePanelView {
        FinancePanelView {
            debt: self.outstanding_debt(),
            interest_rate: self.current_loan_interest_rate(),
            reputation: self.reputation,
            active_loan: self.active_loan.map(|loan| ActiveLoanView {
                offer_id: loan.offer_id,
                principal_remaining: loan.principal_remaining,
                monthly_interest_rate: loan.monthly_interest_rate,
                remaining_months: loan.remaining_months,
                next_payment: loan.next_payment,
            }),
            loan_offers: self
                .loan_offers()
                .into_iter()
                .map(|offer| LoanOfferView {
                    id: offer.id,
                    label: offer.id.label(),
                    principal: offer.principal,
                    monthly_interest_rate: offer.monthly_interest_rate,
                    term_months: offer.term_months,
                    monthly_payment: super::finance::annuity_payment(
                        offer.principal,
                        offer.monthly_interest_rate,
                        offer.term_months,
                    ),
                })
                .collect(),
        }
    }

    pub fn station_ops_view(
        &self,
        ship_id: ShipId,
        station_id: StationId,
    ) -> Option<StationOpsView> {
        let ship = self.ships.get(&ship_id)?;
        Some(StationOpsView {
            ship_id,
            station_id,
            docked: self.is_ship_docked_at(ship_id, station_id),
            cargo: ship.cargo,
            cargo_capacity: ship.cargo_capacity,
            active_contract: ship
                .active_contract
                .and_then(|contract_id| self.contracts.get(&contract_id).cloned()),
            market_rows: self.market_rows_for_station(station_id),
        })
    }

    pub fn station_trade_view(
        &self,
        ship_id: ShipId,
        station_id: StationId,
    ) -> Option<StationTradeView> {
        let ship = self.ships.get(&ship_id)?;
        let docked = self.is_ship_docked_at(ship_id, station_id);
        let market_fee_rate = self.config.pressure.market_fee_rate;

        let rows = self
            .markets
            .get(&station_id)
            .map(|market| {
                Commodity::ALL
                    .iter()
                    .filter_map(|commodity| {
                        market.goods.get(commodity).map(|state| {
                            let player_cargo = ship
                                .cargo
                                .filter(|cargo| cargo.commodity == *commodity)
                                .map(|cargo| cargo.amount)
                                .unwrap_or(0.0);

                            let effective_buy_price = state.price * (1.0 + market_fee_rate);
                            let effective_sell_price = state.price * (1.0 - market_fee_rate);
                            let market_avg_price = self.average_market_price_for(*commodity);
                            let affordable_buy_cap =
                                self.affordable_buy_capacity(effective_buy_price);
                            let insufficient_capital =
                                affordable_buy_cap + 1e-9 < minimum_trade_quantity();

                            let buy_cap = if !docked {
                                0.0
                            } else {
                                actionable_trade_cap(
                                    state
                                        .stock
                                        .min(self.buy_capacity_for(ship, *commodity))
                                        .min(affordable_buy_cap)
                                        .max(0.0),
                                )
                            };
                            let sell_cap = if !docked {
                                0.0
                            } else {
                                actionable_trade_cap(self.sell_capacity_for(ship, *commodity))
                            };

                            StationTradeRowView {
                                commodity: *commodity,
                                station_stock: state.stock,
                                station_target_stock: state.target_stock,
                                player_cargo,
                                spot_price: state.price,
                                effective_buy_price,
                                effective_sell_price,
                                market_avg_price,
                                buy_tone: trade_price_tone(
                                    effective_buy_price,
                                    market_avg_price,
                                    PriceComparisonMode::LowerIsBetter,
                                ),
                                sell_tone: trade_price_tone(
                                    effective_sell_price,
                                    market_avg_price,
                                    PriceComparisonMode::HigherIsBetter,
                                ),
                                buy_cap,
                                sell_cap,
                                insufficient_capital,
                                can_buy: buy_cap > 0.0,
                                can_sell: sell_cap > 0.0,
                            }
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        Some(StationTradeView {
            ship_id,
            station_id,
            docked,
            cargo: ship.cargo,
            cargo_capacity: ship.cargo_capacity,
            active_contract: ship
                .active_contract
                .and_then(|contract_id| self.contracts.get(&contract_id).cloned()),
            market_fee_rate,
            rows,
        })
    }

    pub fn ship_policy_view(&self, ship_id: ShipId) -> Option<ShipPolicyView> {
        self.ship_policy(ship_id)
            .cloned()
            .map(|policy| ShipPolicyView { ship_id, policy })
    }

    pub fn hud_overview_view(&self) -> HudOverviewView {
        let cycle_report = self.cycle_report();
        HudOverviewView {
            tick: self.tick,
            cycle: self.cycle,
            capital: self.capital,
            debt: self.outstanding_debt(),
            interest_rate: self.current_loan_interest_rate(),
            reputation: self.reputation,
            active_contracts: self
                .contracts
                .values()
                .filter(|contract| !contract.completed && !contract.failed)
                .count(),
            active_ships: self.ships.len(),
            sla_success_rate: cycle_report.sla_success_rate,
            reroutes: self.reroute_count,
            avg_price_index: self.average_price_index(),
            market_share: self.market_share_view(),
            milestones: self.milestones.clone(),
        }
    }

    pub fn market_intel(&self, system_id: SystemId, local_cluster: bool) -> Option<MarketIntel> {
        let station_id = self.world.first_station(system_id)?;
        self.markets.get(&station_id).map(|_| {
            if local_cluster {
                MarketIntel {
                    system_id,
                    observed_tick: self.tick,
                    staleness_ticks: 0,
                    confidence: 1.0,
                }
            } else {
                let staleness = 5 + (self.tick % 13);
                let confidence = (1.0 - staleness as f64 / 30.0).clamp(0.3, 0.95);
                MarketIntel {
                    system_id,
                    observed_tick: self.tick.saturating_sub(staleness),
                    staleness_ticks: staleness,
                    confidence,
                }
            }
        })
    }

    pub fn station_of_contract(&self, contract_id: ContractId) -> Option<(StationId, StationId)> {
        self.contracts
            .get(&contract_id)
            .map(|contract| (contract.origin_station, contract.destination_station))
    }

    pub fn station_position(&self, station_id: StationId) -> Option<(f64, f64)> {
        self.world.station_coords(station_id)
    }

    pub fn is_ship_docked_at(&self, ship_id: ShipId, station_id: StationId) -> bool {
        self.ships
            .get(&ship_id)
            .map(|ship| {
                ship.current_station == Some(station_id)
                    && ship.eta_ticks_remaining == 0
                    && ship.segment_eta_remaining == 0
                    && ship.movement_queue.is_empty()
                    && ship.current_segment_kind.is_none()
            })
            .unwrap_or(false)
    }

    pub fn fleet_status(&self) -> Vec<FleetShipStatus> {
        let high_queue = self
            .gate_queue_load
            .values()
            .copied()
            .fold(0.0_f64, f64::max)
            > 1.0;
        let mut status = self
            .ships
            .values()
            .map(|ship| {
                let warning = if ship.eta_ticks_remaining == 0 && ship.active_contract.is_none() {
                    Some(FleetWarning::ShipIdle)
                } else if ship.active_contract.is_some()
                    && ship.current_target.is_none()
                    && ship.planned_path.is_empty()
                {
                    Some(FleetWarning::NoRoute)
                } else if ship.last_risk_score >= 1.0 {
                    Some(FleetWarning::HighRisk)
                } else if high_queue {
                    Some(FleetWarning::HighQueueDelay)
                } else {
                    None
                };

                FleetShipStatus {
                    ship_id: ship.id,
                    company_id: ship.company_id,
                    role: ship.role,
                    location: ship.location,
                    current_station: ship.current_station,
                    target: ship.current_target,
                    eta: ship.eta_ticks_remaining,
                    active_contract: ship.active_contract,
                    cargo_commodity: ship.cargo.map(|cargo| cargo.commodity),
                    cargo_amount: ship.cargo.map(|cargo| cargo.amount).unwrap_or(0.0),
                    route_len: ship.planned_path.len(),
                    reroutes: ship.reroutes,
                    warning,
                    job_queue: self.project_ship_job_queue(ship),
                    idle_ticks_cycle: self
                        .ship_idle_ticks_cycle
                        .get(&ship.id)
                        .copied()
                        .unwrap_or(0),
                    avg_delay_ticks_cycle: {
                        let delay = self
                            .ship_delay_ticks_cycle
                            .get(&ship.id)
                            .copied()
                            .unwrap_or(0) as f64;
                        let runs =
                            self.ship_runs_completed.get(&ship.id).copied().unwrap_or(0) as f64;
                        if runs > 0.0 {
                            delay / runs
                        } else {
                            delay
                        }
                    },
                    profit_per_run: {
                        let profit = self
                            .ship_profit_earned
                            .get(&ship.id)
                            .copied()
                            .unwrap_or(0.0);
                        let runs =
                            self.ship_runs_completed.get(&ship.id).copied().unwrap_or(0) as f64;
                        if runs > 0.0 {
                            profit / runs
                        } else {
                            0.0
                        }
                    },
                }
            })
            .collect::<Vec<_>>();
        status.sort_by_key(|entry| entry.ship_id.0);
        status
    }

    pub fn gate_throughput_view(&self) -> Vec<GateThroughputSnapshot> {
        self.world
            .edges
            .iter()
            .map(|edge| {
                let mut total = 0_u32;
                let mut player = 0_u32;
                for cycle_map in &self.gate_traversals_window {
                    if let Some(by_company) = cycle_map.get(&edge.id) {
                        for (company_id, count) in by_company {
                            total = total.saturating_add(*count);
                            if *company_id == CompanyId(0) {
                                player = player.saturating_add(*count);
                            }
                        }
                    }
                }
                let player_share = if total == 0 {
                    0.0
                } else {
                    player as f64 / total as f64
                };
                GateThroughputSnapshot {
                    gate_id: edge.id,
                    player_share,
                    total_flow: total,
                }
            })
            .collect()
    }

    pub fn milestone_status(&self) -> &[MilestoneStatus] {
        &self.milestones
    }

    pub fn market_share_view(&self) -> f64 {
        let mut player_total = 0_u64;
        let mut world_total = 0_u64;
        for cycle_map in &self.gate_traversals_window {
            for by_company in cycle_map.values() {
                for (company_id, count) in by_company {
                    world_total = world_total.saturating_add(u64::from(*count));
                    if *company_id == CompanyId(0) {
                        player_total = player_total.saturating_add(u64::from(*count));
                    }
                }
            }
        }
        if world_total == 0 {
            0.0
        } else {
            player_total as f64 / world_total as f64
        }
    }

    pub fn market_insights(&self, station_id: StationId) -> Vec<MarketInsightRow> {
        let Some(book) = self.markets.get(&station_id) else {
            return Vec::new();
        };
        let system_id = self
            .world
            .stations
            .iter()
            .find(|station| station.id == station_id)
            .map(|station| station.system_id)
            .unwrap_or(SystemId(0));
        let congestion_factor = self.system_congestion_signal(system_id);
        let fuel_factor = self
            .modifiers
            .iter()
            .filter(|m| m.risk == RiskStageA::FuelShock)
            .map(|m| 1.0 - m.magnitude)
            .fold(0.0_f64, f64::max);
        let mut rows = Vec::new();
        for commodity in Commodity::ALL {
            let Some(state) = book.goods.get(&commodity) else {
                continue;
            };
            let prev_price = self
                .previous_cycle_prices
                .get(&(station_id, commodity))
                .copied()
                .unwrap_or(state.price);
            let trend_delta = state.price - prev_price;
            let imbalance = (state.target_stock - state.stock) / state.target_stock.max(1.0);
            let flow_pressure =
                (state.cycle_outflow - state.cycle_inflow) / state.target_stock.max(1.0);
            let raw_delta =
                self.config.market.k_stock * imbalance + self.config.market.k_flow * flow_pressure;
            let delta =
                raw_delta.clamp(-self.config.market.delta_cap, self.config.market.delta_cap);
            let floor = state.base_price * self.config.market.floor_mult;
            let ceil = state.base_price * self.config.market.ceiling_mult;
            let forecast_next = (state.price * (1.0 + delta)).clamp(floor, ceil);
            rows.push(MarketInsightRow {
                commodity,
                trend_delta,
                forecast_next,
                imbalance_factor: imbalance,
                congestion_factor,
                fuel_factor,
            });
        }
        rows
    }

    fn system_congestion_signal(&self, system_id: SystemId) -> f64 {
        let Some(edges) = self.world.adjacency.get(&system_id) else {
            return 0.0;
        };
        if edges.is_empty() {
            return 0.0;
        }

        edges
            .iter()
            .map(|(_, gate_id)| {
                let load = self.gate_queue_load.get(gate_id).copied().unwrap_or(0.0);
                let effective_capacity = self
                    .world
                    .edges
                    .iter()
                    .find(|edge| edge.id == *gate_id)
                    .map(|edge| (edge.base_capacity * edge.capacity_factor).max(1.0))
                    .unwrap_or(1.0);
                load / effective_capacity
            })
            .sum::<f64>()
            / edges.len() as f64
    }

    fn default_player_ship_id(&self) -> Option<ShipId> {
        self.ships
            .values()
            .filter(|ship| ship.company_id == CompanyId(0))
            .map(|ship| ship.id)
            .min_by_key(|ship_id| ship_id.0)
    }

    fn ship_policy(&self, ship_id: ShipId) -> Option<&AutopilotPolicy> {
        self.ships.get(&ship_id).map(|ship| &ship.policy)
    }

    fn camera_stations_for_system(&self, system_id: SystemId) -> Vec<CameraStationView> {
        self.world
            .stations_by_system
            .get(&system_id)
            .into_iter()
            .flatten()
            .filter_map(|station_id| {
                self.world
                    .stations
                    .iter()
                    .find(|station| station.id == *station_id)
                    .map(|station| CameraStationView {
                        station_id: station.id,
                        profile: station.profile,
                        x: station.x,
                        y: station.y,
                    })
            })
            .collect()
    }

    fn market_rows_for_station(&self, station_id: StationId) -> Vec<MarketRowView> {
        self.markets
            .get(&station_id)
            .map(|market| {
                Commodity::ALL
                    .iter()
                    .filter_map(|commodity| {
                        market.goods.get(commodity).map(|state| MarketRowView {
                            commodity: *commodity,
                            price: state.price,
                            stock: state.stock,
                            inflow: state.cycle_inflow,
                            outflow: state.cycle_outflow,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    fn average_market_price_for(&self, commodity: Commodity) -> f64 {
        let mut total = 0.0;
        let mut count = 0.0;
        for market in self.markets.values() {
            if let Some(state) = market.goods.get(&commodity) {
                total += state.price;
                count += 1.0;
            }
        }
        if count <= 0.0 {
            0.0
        } else {
            total / count
        }
    }

    fn buy_capacity_for(&self, ship: &Ship, commodity: Commodity) -> f64 {
        match ship.cargo {
            None => ship.cargo_capacity,
            Some(cargo) if cargo.source == CargoSource::Spot && cargo.commodity == commodity => {
                (ship.cargo_capacity - cargo.amount).max(0.0)
            }
            _ => 0.0,
        }
    }

    fn sell_capacity_for(&self, ship: &Ship, commodity: Commodity) -> f64 {
        ship.cargo
            .filter(|cargo| cargo.source == CargoSource::Spot && cargo.commodity == commodity)
            .map(|cargo| cargo.amount.max(0.0))
            .unwrap_or(0.0)
    }

    fn affordable_buy_capacity(&self, effective_buy_price: f64) -> f64 {
        if effective_buy_price <= 0.0 {
            0.0
        } else {
            (self.capital / effective_buy_price).max(0.0)
        }
    }

    fn dock_congestion_index(&self, system_id: SystemId) -> f32 {
        let inbound = self
            .ships
            .values()
            .filter(|ship| ship.current_target == Some(system_id) && ship.eta_ticks_remaining > 0)
            .count() as f32;
        (inbound / 6.0).clamp(0.0, 1.0)
    }

    fn fuel_stress_index(&self, system_id: SystemId) -> f32 {
        let Some(stations) = self.world.stations_by_system.get(&system_id) else {
            return 0.0;
        };
        let mut stock = 0.0;
        let mut target = 0.0;
        for station_id in stations {
            if let Some(fuel) = self
                .markets
                .get(station_id)
                .and_then(|market| market.goods.get(&Commodity::Fuel))
            {
                stock += fuel.stock;
                target += fuel.target_stock;
            }
        }
        if target <= 0.0 {
            return 0.0;
        }
        let ratio = (stock / target).clamp(0.0, 1.0);
        (1.0 - ratio as f32).clamp(0.0, 1.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PriceComparisonMode {
    LowerIsBetter,
    HigherIsBetter,
}

fn trade_price_tone(
    compared_price: f64,
    market_avg_price: f64,
    mode: PriceComparisonMode,
) -> TradePriceTone {
    if market_avg_price <= 0.0 {
        return TradePriceTone::Neutral;
    }

    let parity = compared_price / market_avg_price;
    if (parity - 1.0).abs() <= 0.02 {
        return TradePriceTone::Neutral;
    }

    match mode {
        PriceComparisonMode::LowerIsBetter => {
            if parity < 1.0 {
                TradePriceTone::Favorable
            } else {
                TradePriceTone::Unfavorable
            }
        }
        PriceComparisonMode::HigherIsBetter => {
            if parity > 1.0 {
                TradePriceTone::Favorable
            } else {
                TradePriceTone::Unfavorable
            }
        }
    }
}

fn minimum_trade_quantity() -> f64 {
    0.1
}

fn actionable_trade_cap(cap: f64) -> f64 {
    if cap + 1e-9 < minimum_trade_quantity() {
        0.0
    } else {
        cap
    }
}
