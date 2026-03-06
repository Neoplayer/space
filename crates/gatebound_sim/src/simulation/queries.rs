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
                    owner_faction_id: system.owner_faction_id,
                    faction_color_rgb: self
                        .world
                        .faction_color(system.owner_faction_id)
                        .unwrap_or([255, 255, 255]),
                    x: system.x,
                    y: system.y,
                    radius: system.radius,
                    stations: self.camera_stations_for_system(system.id),
                })
                .collect(),
            gate_ids: self.world.edges.iter().map(|edge| edge.id).collect(),
        }
    }

    pub fn system_details_view(&self, system_id: SystemId) -> Option<SystemDetailsView> {
        let system = self
            .world
            .systems
            .iter()
            .find(|system| system.id == system_id)?;
        let faction = self
            .world
            .factions
            .iter()
            .find(|faction| faction.id == system.owner_faction_id)?;
        let stress = self
            .system_market_stress_rows()
            .into_iter()
            .find(|row| row.system_id == system_id)
            .unwrap_or(SystemMarketStressRowView {
                system_id,
                avg_price_index: 1.0,
                stock_coverage: 0.0,
                net_flow: 0.0,
                congestion: 0.0,
                fuel_stress: 0.0,
                stress_score: 0.0,
            });

        let mut stations = self
            .camera_stations_for_system(system_id)
            .into_iter()
            .map(|station| {
                let detail = self.station_market_detail(station.station_id);
                SystemStationSummaryView {
                    station_id: station.station_id,
                    profile: station.profile,
                    x: station.x,
                    y: station.y,
                    price_index: detail.as_ref().map_or(1.0, |detail| detail.price_index),
                    stock_coverage: detail.as_ref().map_or(0.0, |detail| detail.stock_coverage),
                    strongest_shortage_commodity: detail
                        .as_ref()
                        .and_then(|detail| detail.strongest_shortage_commodity),
                    strongest_surplus_commodity: detail
                        .as_ref()
                        .and_then(|detail| detail.strongest_surplus_commodity),
                    best_buy_commodity: detail
                        .as_ref()
                        .and_then(|detail| detail.best_buy_commodity),
                    best_sell_commodity: detail
                        .as_ref()
                        .and_then(|detail| detail.best_sell_commodity),
                }
            })
            .collect::<Vec<_>>();
        stations.sort_by_key(|station| station.station_id.0);

        let mut ships = self
            .ships
            .values()
            .filter(|ship| ship.location == system_id)
            .filter_map(|ship| {
                let owner = self.companies.get(&ship.company_id)?;
                Some(SystemShipSummaryView {
                    ship_id: ship.id,
                    company_id: ship.company_id,
                    owner_name: owner.name.clone(),
                    owner_archetype: owner.archetype,
                    role: ship.role,
                    ship_name: ship.descriptor.name.clone(),
                    ship_class: ship.descriptor.class,
                    location: ship.location,
                    current_station: ship.current_station,
                    current_target: ship.current_target,
                    eta_ticks_remaining: ship.eta_ticks_remaining,
                    current_segment_kind: ship.current_segment_kind,
                    last_risk_score: ship.last_risk_score,
                    reroutes: ship.reroutes,
                })
            })
            .collect::<Vec<_>>();
        ships.sort_by_key(|ship| ship.ship_id.0);

        Some(SystemDetailsView {
            system_id,
            owner_faction_id: system.owner_faction_id,
            owner_faction_name: faction.name.clone(),
            faction_color_rgb: faction.color_rgb,
            dock_capacity: system.dock_capacity,
            outgoing_gate_count: self
                .world
                .adjacency
                .get(&system_id)
                .map_or(0, |entries| entries.len()),
            avg_price_index: stress.avg_price_index,
            stock_coverage: stress.stock_coverage,
            net_flow: stress.net_flow,
            congestion: stress.congestion,
            fuel_stress: stress.fuel_stress,
            stress_score: stress.stress_score,
            stations,
            ships,
        })
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
                    owner_faction_id: system.owner_faction_id,
                    faction_color_rgb: self
                        .world
                        .faction_color(system.owner_faction_id)
                        .unwrap_or([255, 255, 255]),
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

    pub fn corporation_panel_view(&self) -> CorporationPanelView {
        let mut rows = self
            .npc_company_runtimes
            .values()
            .filter_map(|runtime| {
                let company = self.companies.get(&runtime.company_id)?;
                let mut idle_ships = 0_usize;
                let mut in_transit_ships = 0_usize;
                for ship in self
                    .ships
                    .values()
                    .filter(|ship| ship.company_id == runtime.company_id)
                {
                    let busy = ship.trade_order_id.is_some()
                        || ship.segment_eta_remaining > 0
                        || !ship.movement_queue.is_empty();
                    if busy {
                        in_transit_ships += 1;
                    } else {
                        idle_ships += 1;
                    }
                }
                let active_orders = self
                    .trade_orders
                    .values()
                    .filter(|order| order.company_id == runtime.company_id)
                    .count();
                Some(CorporationRowView {
                    company_id: runtime.company_id,
                    name: company.name.clone(),
                    archetype: company.archetype,
                    balance: runtime.balance,
                    last_realized_profit: runtime.last_realized_profit,
                    idle_ships,
                    in_transit_ships,
                    active_orders,
                    next_plan_tick: runtime.next_plan_tick,
                })
            })
            .collect::<Vec<_>>();
        rows.sort_by_key(|row| row.company_id.0);
        CorporationPanelView { rows }
    }

    pub fn market_panel_view(
        &self,
        selected_system_id: SystemId,
        detail_station_id: Option<StationId>,
        focused_commodity: Commodity,
    ) -> MarketPanelView {
        MarketPanelView {
            focused_commodity,
            global_kpis: self.market_global_kpis(),
            commodity_rows: Commodity::ALL
                .into_iter()
                .filter_map(|commodity| self.commodity_market_row(commodity))
                .collect(),
            system_stress_rows: self.system_market_stress_rows(),
            commodity_hotspots: self.commodity_hotspots(focused_commodity),
            station_anomaly_rows: self.station_market_anomaly_rows(),
            station_detail: detail_station_id
                .or_else(|| self.world.first_station(selected_system_id))
                .and_then(|station_id| self.station_market_detail(station_id)),
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

    pub fn ship_card_view(&self, ship_id: ShipId) -> Option<ShipCardView> {
        let ship = self.ships.get(&ship_id)?;
        let owner = self.companies.get(&ship.company_id)?;

        Some(ShipCardView {
            ship_id,
            company_id: ship.company_id,
            owner_name: owner.name.clone(),
            owner_archetype: owner.archetype,
            role: ship.role,
            ship_name: ship.descriptor.name.clone(),
            ship_class: ship.descriptor.class,
            description: ship.descriptor.description.clone(),
            location: ship.location,
            current_station: ship.current_station,
            current_target: ship.current_target,
            eta_ticks_remaining: ship.eta_ticks_remaining,
            current_segment_kind: ship.current_segment_kind,
            cargo_capacity: ship.cargo_capacity,
            cargo: ship.cargo,
            active_contract: ship
                .active_contract
                .and_then(|contract_id| self.contracts.get(&contract_id).cloned()),
            policy: ship.policy.clone(),
            route_len: ship.planned_path.len(),
            reroutes: ship.reroutes,
            last_risk_score: ship.last_risk_score,
            modules: ship.modules.clone(),
            technical_state: ship.technical_state.clone(),
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

    fn market_global_kpis(&self) -> MarketGlobalKpisView {
        let mut aggregate_stock = 0.0;
        let mut aggregate_target_stock = 0.0;
        let mut aggregate_inflow = 0.0;
        let mut aggregate_outflow = 0.0;

        for market in self.markets.values() {
            for state in market.goods.values() {
                aggregate_stock += state.stock;
                aggregate_target_stock += state.target_stock;
                aggregate_inflow += state.cycle_inflow;
                aggregate_outflow += state.cycle_outflow;
            }
        }

        let rolling_window_total_flow = self
            .gate_traversals_window
            .iter()
            .flat_map(|cycle_map| cycle_map.values())
            .flat_map(|by_company| by_company.values())
            .map(|count| u64::from(*count))
            .sum::<u64>();

        MarketGlobalKpisView {
            avg_price_index: self.average_price_index(),
            system_count: self.world.systems.len(),
            station_count: self.world.stations.len(),
            aggregate_stock,
            aggregate_target_stock,
            aggregate_stock_coverage: if aggregate_target_stock <= 0.0 {
                0.0
            } else {
                aggregate_stock / aggregate_target_stock
            },
            aggregate_net_flow: aggregate_inflow - aggregate_outflow,
            market_fee_rate: self.config.pressure.market_fee_rate,
            rolling_window_total_flow,
            player_market_share: self.market_share_view(),
            gate_congestion_active: self
                .modifiers
                .iter()
                .any(|modifier| modifier.risk == RiskStageA::GateCongestion),
            dock_congestion_active: self
                .modifiers
                .iter()
                .any(|modifier| modifier.risk == RiskStageA::DockCongestion),
            fuel_shock_active: self
                .modifiers
                .iter()
                .any(|modifier| modifier.risk == RiskStageA::FuelShock),
        }
    }

    fn commodity_market_row(&self, commodity: Commodity) -> Option<CommodityMarketRowView> {
        let mut price_sum = 0.0;
        let mut base_price_sum = 0.0;
        let mut total_stock = 0.0;
        let mut total_target_stock = 0.0;
        let mut inflow = 0.0;
        let mut outflow = 0.0;
        let mut trend_sum = 0.0;
        let mut forecast_sum = 0.0;
        let mut count = 0.0;
        let mut min_price = f64::INFINITY;
        let mut max_price = f64::NEG_INFINITY;
        let mut min_price_station_id = None;
        let mut max_price_station_id = None;
        let mut stations_below_target = 0_usize;
        let mut stations_above_target = 0_usize;
        let mut system_price_sums = BTreeMap::<SystemId, (f64, f64)>::new();

        for (station_id, market) in &self.markets {
            let Some(state) = market.goods.get(&commodity) else {
                continue;
            };
            let Some(system_id) = self.station_system_id(*station_id) else {
                continue;
            };

            price_sum += state.price;
            base_price_sum += state.base_price;
            total_stock += state.stock;
            total_target_stock += state.target_stock;
            inflow += state.cycle_inflow;
            outflow += state.cycle_outflow;
            trend_sum += state.price - self.previous_price_for(*station_id, commodity, state.price);
            forecast_sum += self.market_forecast(state);
            count += 1.0;

            if state.price < min_price {
                min_price = state.price;
                min_price_station_id = Some(*station_id);
            }
            if state.price > max_price {
                max_price = state.price;
                max_price_station_id = Some(*station_id);
            }
            if state.stock + 1e-9 < state.target_stock {
                stations_below_target += 1;
            } else if state.stock > state.target_stock + 1e-9 {
                stations_above_target += 1;
            }

            let entry = system_price_sums.entry(system_id).or_insert((0.0, 0.0));
            entry.0 += state.price;
            entry.1 += 1.0;
        }

        if count <= 0.0 {
            return None;
        }

        let galaxy_avg_price = price_sum / count;
        let base_price = base_price_sum / count;
        let stock_coverage = if total_target_stock <= 0.0 {
            0.0
        } else {
            total_stock / total_target_stock
        };
        let mut cheapest_system = None;
        let mut priciest_system = None;
        let mut cheapest_system_avg_price = f64::INFINITY;
        let mut priciest_system_avg_price = f64::NEG_INFINITY;
        for (system_id, (system_price_sum, system_count)) in system_price_sums {
            if system_count <= 0.0 {
                continue;
            }
            let avg_price = system_price_sum / system_count;
            if avg_price < cheapest_system_avg_price {
                cheapest_system_avg_price = avg_price;
                cheapest_system = Some(system_id);
            }
            if avg_price > priciest_system_avg_price {
                priciest_system_avg_price = avg_price;
                priciest_system = Some(system_id);
            }
        }

        let spread_abs = max_price - min_price;
        let spread_pct = if galaxy_avg_price <= 0.0 {
            0.0
        } else {
            spread_abs / galaxy_avg_price
        };

        Some(CommodityMarketRowView {
            commodity,
            galaxy_avg_price,
            min_price_station_id,
            min_price,
            max_price_station_id,
            max_price,
            spread_abs,
            spread_pct,
            cheapest_system_id: cheapest_system,
            cheapest_system_avg_price: if cheapest_system.is_some() {
                cheapest_system_avg_price
            } else {
                0.0
            },
            priciest_system_id: priciest_system,
            priciest_system_avg_price: if priciest_system.is_some() {
                priciest_system_avg_price
            } else {
                0.0
            },
            total_stock,
            total_target_stock,
            stock_coverage,
            inflow,
            outflow,
            net_flow: inflow - outflow,
            trend_delta: trend_sum / count,
            forecast_next_avg: forecast_sum / count,
            base_price,
            price_vs_base: if base_price <= 0.0 {
                0.0
            } else {
                galaxy_avg_price / base_price
            },
            stations_below_target,
            stations_above_target,
        })
    }

    fn system_market_stress_rows(&self) -> Vec<SystemMarketStressRowView> {
        let mut rows = self
            .world
            .systems
            .iter()
            .map(|system| {
                let mut price_index_sum = 0.0;
                let mut price_index_count = 0.0;
                let mut total_stock = 0.0;
                let mut total_target_stock = 0.0;
                let mut inflow = 0.0;
                let mut outflow = 0.0;

                for station_id in self
                    .world
                    .stations_by_system
                    .get(&system.id)
                    .into_iter()
                    .flatten()
                {
                    if let Some(market) = self.markets.get(station_id) {
                        for state in market.goods.values() {
                            price_index_sum += state.price / state.base_price.max(1e-9);
                            price_index_count += 1.0;
                            total_stock += state.stock;
                            total_target_stock += state.target_stock;
                            inflow += state.cycle_inflow;
                            outflow += state.cycle_outflow;
                        }
                    }
                }

                let avg_price_index = if price_index_count <= 0.0 {
                    1.0
                } else {
                    price_index_sum / price_index_count
                };
                let stock_coverage = if total_target_stock <= 0.0 {
                    0.0
                } else {
                    total_stock / total_target_stock
                };
                let congestion = self.system_congestion_signal(system.id);
                let fuel_stress = f64::from(self.fuel_stress_index(system.id));
                let flow_drain = if total_target_stock <= 0.0 {
                    0.0
                } else {
                    ((outflow - inflow) / total_target_stock).max(0.0)
                };
                let scarcity = (1.0 - stock_coverage).max(0.0);
                let price_pressure = (avg_price_index - 1.0).max(0.0);
                let stress_score = price_pressure * 0.40
                    + scarcity * 0.35
                    + congestion * 0.15
                    + fuel_stress * 0.10
                    + flow_drain * 0.10;

                SystemMarketStressRowView {
                    system_id: system.id,
                    avg_price_index,
                    stock_coverage,
                    net_flow: inflow - outflow,
                    congestion,
                    fuel_stress,
                    stress_score,
                }
            })
            .collect::<Vec<_>>();

        rows.sort_by(|a, b| {
            b.stress_score
                .total_cmp(&a.stress_score)
                .then_with(|| a.system_id.0.cmp(&b.system_id.0))
        });
        rows
    }

    fn commodity_hotspots(&self, commodity: Commodity) -> CommodityHotspotsView {
        let mut station_rows = self
            .world
            .stations
            .iter()
            .filter_map(|station| {
                self.markets
                    .get(&station.id)
                    .and_then(|market| market.goods.get(&commodity))
                    .map(|state| StationCommodityHotspotView {
                        station_id: station.id,
                        system_id: station.system_id,
                        price: state.price,
                        stock_coverage: if state.target_stock <= 0.0 {
                            0.0
                        } else {
                            state.stock / state.target_stock
                        },
                        net_flow: state.cycle_inflow - state.cycle_outflow,
                    })
            })
            .collect::<Vec<_>>();
        let mut cheapest_stations = station_rows.clone();
        cheapest_stations.sort_by(|a, b| {
            a.price
                .total_cmp(&b.price)
                .then_with(|| a.station_id.0.cmp(&b.station_id.0))
        });
        let mut priciest_stations = station_rows.clone();
        priciest_stations.sort_by(|a, b| {
            b.price
                .total_cmp(&a.price)
                .then_with(|| a.station_id.0.cmp(&b.station_id.0))
        });
        station_rows.clear();

        let mut system_rows = self
            .world
            .systems
            .iter()
            .filter_map(|system| {
                let mut price_sum = 0.0;
                let mut stock_sum = 0.0;
                let mut target_sum = 0.0;
                let mut inflow_sum = 0.0;
                let mut outflow_sum = 0.0;
                let mut count = 0.0;

                for station_id in self
                    .world
                    .stations_by_system
                    .get(&system.id)
                    .into_iter()
                    .flatten()
                {
                    if let Some(state) = self
                        .markets
                        .get(station_id)
                        .and_then(|market| market.goods.get(&commodity))
                    {
                        price_sum += state.price;
                        stock_sum += state.stock;
                        target_sum += state.target_stock;
                        inflow_sum += state.cycle_inflow;
                        outflow_sum += state.cycle_outflow;
                        count += 1.0;
                    }
                }

                if count <= 0.0 {
                    None
                } else {
                    Some(SystemCommodityHotspotView {
                        system_id: system.id,
                        avg_price: price_sum / count,
                        stock_coverage: if target_sum <= 0.0 {
                            0.0
                        } else {
                            stock_sum / target_sum
                        },
                        net_flow: inflow_sum - outflow_sum,
                    })
                }
            })
            .collect::<Vec<_>>();
        let mut cheapest_systems = system_rows.clone();
        cheapest_systems.sort_by(|a, b| {
            a.avg_price
                .total_cmp(&b.avg_price)
                .then_with(|| a.system_id.0.cmp(&b.system_id.0))
        });
        let mut priciest_systems = system_rows.clone();
        priciest_systems.sort_by(|a, b| {
            b.avg_price
                .total_cmp(&a.avg_price)
                .then_with(|| a.system_id.0.cmp(&b.system_id.0))
        });
        system_rows.clear();

        CommodityHotspotsView {
            focused_commodity: commodity,
            cheapest_stations: cheapest_stations.into_iter().take(3).collect(),
            priciest_stations: priciest_stations.into_iter().take(3).collect(),
            cheapest_systems: cheapest_systems.into_iter().take(3).collect(),
            priciest_systems: priciest_systems.into_iter().take(3).collect(),
        }
    }

    fn station_market_anomaly_rows(&self) -> Vec<StationMarketAnomalyRowView> {
        let mut rows = self
            .world
            .stations
            .iter()
            .filter_map(|station| {
                let market = self.markets.get(&station.id)?;
                let mut price_index_sum = 0.0;
                let mut price_index_count = 0.0;
                let mut total_stock = 0.0;
                let mut total_target_stock = 0.0;
                let mut inflow = 0.0;
                let mut outflow = 0.0;
                let mut price_deviation_sum = 0.0;
                let mut price_deviation_count = 0.0;
                let mut shortage_count = 0_usize;
                let mut surplus_count = 0_usize;
                let mut max_shortage = 0.0_f64;
                let mut max_surplus = 0.0_f64;

                for commodity in Commodity::ALL {
                    let Some(state) = market.goods.get(&commodity) else {
                        continue;
                    };
                    price_index_sum += state.price / state.base_price.max(1e-9);
                    price_index_count += 1.0;
                    total_stock += state.stock;
                    total_target_stock += state.target_stock;
                    inflow += state.cycle_inflow;
                    outflow += state.cycle_outflow;

                    let galaxy_avg_price = self.average_market_price_for(commodity);
                    if galaxy_avg_price > 0.0 {
                        price_deviation_sum +=
                            ((state.price - galaxy_avg_price) / galaxy_avg_price).abs();
                        price_deviation_count += 1.0;
                    }

                    let shortage =
                        ((state.target_stock - state.stock) / state.target_stock.max(1.0)).max(0.0);
                    let surplus =
                        ((state.stock - state.target_stock) / state.target_stock.max(1.0)).max(0.0);
                    if shortage > 0.0 {
                        shortage_count += 1;
                        max_shortage = max_shortage.max(shortage);
                    }
                    if surplus > 0.0 {
                        surplus_count += 1;
                        max_surplus = max_surplus.max(surplus);
                    }
                }

                let price_index = if price_index_count <= 0.0 {
                    1.0
                } else {
                    price_index_sum / price_index_count
                };
                let stock_coverage = if total_target_stock <= 0.0 {
                    0.0
                } else {
                    total_stock / total_target_stock
                };
                let avg_price_deviation = if price_deviation_count <= 0.0 {
                    0.0
                } else {
                    price_deviation_sum / price_deviation_count
                };
                let anomaly_score = avg_price_deviation
                    + max_shortage * 0.6
                    + max_surplus * 0.3
                    + (price_index - 1.0).abs() * 0.25;

                Some(StationMarketAnomalyRowView {
                    station_id: station.id,
                    system_id: station.system_id,
                    price_index,
                    stock_coverage,
                    net_flow: inflow - outflow,
                    avg_price_deviation,
                    shortage_count,
                    surplus_count,
                    anomaly_score,
                })
            })
            .collect::<Vec<_>>();

        rows.sort_by(|a, b| {
            b.anomaly_score
                .total_cmp(&a.anomaly_score)
                .then_with(|| a.station_id.0.cmp(&b.station_id.0))
        });
        rows
    }

    fn station_market_detail(&self, station_id: StationId) -> Option<StationMarketDetailView> {
        let market = self.markets.get(&station_id)?;
        let system_id = self.station_system_id(station_id)?;
        let mut commodity_rows = Vec::new();
        let mut price_index_sum = 0.0;
        let mut price_index_count = 0.0;
        let mut price_deviation_sum = 0.0;
        let mut price_deviation_count = 0.0;
        let mut total_stock = 0.0;
        let mut total_target_stock = 0.0;
        let mut inflow = 0.0;
        let mut outflow = 0.0;
        let mut shortage_count = 0_usize;
        let mut surplus_count = 0_usize;
        let mut strongest_shortage = (None, 0.0);
        let mut strongest_surplus = (None, 0.0);
        let mut best_buy = (None, 1.0);
        let mut best_sell = (None, 1.0);

        for commodity in Commodity::ALL {
            let Some(state) = market.goods.get(&commodity) else {
                continue;
            };
            let galaxy_avg_price = self.average_market_price_for(commodity);
            let stock_coverage = if state.target_stock <= 0.0 {
                0.0
            } else {
                state.stock / state.target_stock
            };
            let trend_delta =
                state.price - self.previous_price_for(station_id, commodity, state.price);
            let forecast_next = self.market_forecast(state);
            let price_ratio = if galaxy_avg_price <= 0.0 {
                1.0
            } else {
                state.price / galaxy_avg_price
            };
            let shortage =
                ((state.target_stock - state.stock) / state.target_stock.max(1.0)).max(0.0);
            let surplus =
                ((state.stock - state.target_stock) / state.target_stock.max(1.0)).max(0.0);

            if shortage > 0.0 {
                shortage_count += 1;
                if shortage > strongest_shortage.1 {
                    strongest_shortage = (Some(commodity), shortage);
                }
            }
            if surplus > 0.0 {
                surplus_count += 1;
                if surplus > strongest_surplus.1 {
                    strongest_surplus = (Some(commodity), surplus);
                }
            }
            if price_ratio < best_buy.1 {
                best_buy = (Some(commodity), price_ratio);
            }
            if price_ratio > best_sell.1 {
                best_sell = (Some(commodity), price_ratio);
            }

            price_index_sum += state.price / state.base_price.max(1e-9);
            price_index_count += 1.0;
            total_stock += state.stock;
            total_target_stock += state.target_stock;
            inflow += state.cycle_inflow;
            outflow += state.cycle_outflow;

            if galaxy_avg_price > 0.0 {
                price_deviation_sum += ((state.price - galaxy_avg_price) / galaxy_avg_price).abs();
                price_deviation_count += 1.0;
            }

            commodity_rows.push(StationCommodityDetailView {
                commodity,
                local_price: state.price,
                galaxy_avg_price,
                price_delta: state.price - galaxy_avg_price,
                local_stock: state.stock,
                local_target_stock: state.target_stock,
                stock_coverage,
                inflow: state.cycle_inflow,
                outflow: state.cycle_outflow,
                net_flow: state.cycle_inflow - state.cycle_outflow,
                trend_delta,
                forecast_next,
                price_vs_base: if state.base_price <= 0.0 {
                    0.0
                } else {
                    state.price / state.base_price
                },
            });
        }

        let price_index = if price_index_count <= 0.0 {
            1.0
        } else {
            price_index_sum / price_index_count
        };
        let avg_price_deviation = if price_deviation_count <= 0.0 {
            0.0
        } else {
            price_deviation_sum / price_deviation_count
        };

        Some(StationMarketDetailView {
            station_id,
            system_id,
            price_index,
            avg_price_deviation,
            total_stock,
            total_target_stock,
            stock_coverage: if total_target_stock <= 0.0 {
                0.0
            } else {
                total_stock / total_target_stock
            },
            inflow,
            outflow,
            net_flow: inflow - outflow,
            shortage_count,
            surplus_count,
            strongest_shortage_commodity: strongest_shortage.0,
            strongest_surplus_commodity: strongest_surplus.0,
            best_buy_commodity: best_buy.0,
            best_sell_commodity: best_sell.0,
            commodity_rows,
        })
    }

    fn station_system_id(&self, station_id: StationId) -> Option<SystemId> {
        self.world
            .stations
            .iter()
            .find(|station| station.id == station_id)
            .map(|station| station.system_id)
    }

    fn previous_price_for(
        &self,
        station_id: StationId,
        commodity: Commodity,
        fallback: f64,
    ) -> f64 {
        self.previous_cycle_prices
            .get(&(station_id, commodity))
            .copied()
            .unwrap_or(fallback)
    }

    fn market_forecast(&self, state: &MarketState) -> f64 {
        let imbalance = (state.target_stock - state.stock) / state.target_stock.max(1.0);
        let flow_pressure =
            (state.cycle_outflow - state.cycle_inflow) / state.target_stock.max(1.0);
        let raw_delta =
            self.config.market.k_stock * imbalance + self.config.market.k_flow * flow_pressure;
        let delta = raw_delta.clamp(-self.config.market.delta_cap, self.config.market.delta_cap);
        let floor = state.base_price * self.config.market.floor_mult;
        let ceil = state.base_price * self.config.market.ceiling_mult;
        (state.price * (1.0 + delta)).clamp(floor, ceil)
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
