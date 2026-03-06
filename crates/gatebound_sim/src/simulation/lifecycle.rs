use super::*;

impl Simulation {
    pub fn new(mut config: RuntimeConfig, seed: u64) -> Self {
        config.galaxy.seed = seed;
        let world = World::generate(&config.galaxy, seed);
        let mut markets = BTreeMap::new();
        for station in &world.stations {
            let mut goods = BTreeMap::new();
            for commodity in Commodity::ALL {
                let base_price = base_price_for(commodity);
                goods.insert(
                    commodity,
                    MarketState {
                        base_price,
                        price: base_price,
                        stock: 100.0,
                        target_stock: 100.0,
                        cycle_inflow: 0.0,
                        cycle_outflow: 0.0,
                    },
                );
            }
            markets.insert(station.id, MarketBook { goods });
        }
        let mut previous_cycle_prices = BTreeMap::new();
        for (station_id, book) in &markets {
            for commodity in Commodity::ALL {
                if let Some(state) = book.goods.get(&commodity) {
                    previous_cycle_prices.insert((*station_id, commodity), state.price);
                }
            }
        }

        let companies = seed_stage_a_companies();
        let mut ships = seed_stage_a_ships(&world);
        let npc_company_runtimes = seed_stage_a_npc_company_runtimes(&config);
        let mut contracts = BTreeMap::new();
        if world.system_count() >= 2 && ships.contains_key(&ShipId(0)) {
            let origin_station = world.first_station(SystemId(0)).unwrap_or(StationId(0));
            let destination_station = world.first_station(SystemId(1)).unwrap_or(origin_station);
            contracts.insert(
                ContractId(0),
                Contract {
                    id: ContractId(0),
                    kind: ContractTypeStageA::Delivery,
                    progress: ContractProgress::AwaitPickup,
                    commodity: Commodity::Fuel,
                    origin: SystemId(0),
                    destination: SystemId(1),
                    origin_station,
                    destination_station,
                    quantity: 10.0,
                    deadline_tick: u64::from(config.time.cycle_ticks) * 3,
                    per_cycle: 0.0,
                    total_cycles: 0,
                    payout: 50.0,
                    penalty: 25.0,
                    assigned_ship: Some(ShipId(0)),
                    loaded_amount: 0.0,
                    delivered_cycle_amount: 0.0,
                    delivered_amount: 0.0,
                    missed_cycles: 0,
                    completed: false,
                    failed: false,
                    last_eval_cycle: 0,
                },
            );
            if let Some(player_ship) = ships.get_mut(&ShipId(0)) {
                player_ship.active_contract = Some(ContractId(0));
            }
        }

        let mut simulation = Self {
            config,
            world,
            tick: 0,
            cycle: 0,
            companies,
            npc_company_runtimes,
            markets,
            contracts,
            contract_offers: BTreeMap::new(),
            next_offer_id: 0,
            trade_orders: BTreeMap::new(),
            next_trade_order_id: 0,
            ships,
            milestones: Vec::new(),
            capital: 500.0,
            active_loan: None,
            outstanding_debt: 0.0,
            reputation: 1.0,
            current_loan_interest_rate: 0.0,
            gate_traversals_cycle: BTreeMap::new(),
            gate_traversals_window: VecDeque::new(),
            queue_delay_accumulator: 0,
            reroute_count: 0,
            sla_successes: 0,
            sla_failures: 0,
            gate_queue_load: BTreeMap::new(),
            ship_idle_ticks_cycle: BTreeMap::new(),
            ship_delay_ticks_cycle: BTreeMap::new(),
            ship_runs_completed: BTreeMap::new(),
            ship_profit_earned: BTreeMap::new(),
            previous_cycle_prices,
            modifiers: Vec::new(),
        };
        simulation.milestones = vec![
            MilestoneStatus {
                id: MilestoneId::Capital,
                current: simulation.capital,
                target: simulation.config.pressure.milestone_capital_target,
                completed: false,
                completed_cycle: None,
            },
            MilestoneStatus {
                id: MilestoneId::MarketShare,
                current: 0.0,
                target: simulation.config.pressure.milestone_market_share_target,
                completed: false,
                completed_cycle: None,
            },
            MilestoneStatus {
                id: MilestoneId::ThroughputControl,
                current: 0.0,
                target: simulation.config.pressure.milestone_throughput_target_share,
                completed: false,
                completed_cycle: None,
            },
            MilestoneStatus {
                id: MilestoneId::Reputation,
                current: simulation.reputation,
                target: simulation.config.pressure.milestone_reputation_target,
                completed: false,
                completed_cycle: None,
            },
        ];
        simulation.normalize_player_ship_roster();
        simulation.refresh_contract_offers();
        simulation
    }

    pub fn step_tick(&mut self) -> TickReport {
        self.tick = self.tick.saturating_add(1);
        self.run_economy_flow();
        self.dispatch_npc_trade_orders();
        self.update_ship_movements();
        self.update_contracts_tick();
        self.expire_modifiers();

        if self
            .tick
            .is_multiple_of(u64::from(self.config.time.cycle_ticks))
        {
            self.step_cycle();
        }

        if self.tick.is_multiple_of(self.month_ticks()) {
            self.step_month();
        }

        TickReport {
            tick: self.tick,
            cycle: self.cycle,
            active_ships: self.ships.len(),
            active_contracts: self
                .contracts
                .values()
                .filter(|c| !c.completed && !c.failed)
                .count(),
            total_queue_delay: self.queue_delay_accumulator,
            avg_price_index: self.average_price_index(),
        }
    }

    pub fn step_cycle(&mut self) -> CycleReport {
        self.cycle = self.cycle.saturating_add(1);
        self.capture_previous_cycle_prices();
        self.update_market_prices();
        self.evaluate_supply_contracts();
        self.roll_gate_traversal_window();
        self.expire_contract_offers();
        if self
            .cycle
            .is_multiple_of(u64::from(self.config.pressure.offer_refresh_cycles.max(1)))
        {
            self.refresh_contract_offers();
        }
        self.update_milestones();

        let total_sla = self.sla_successes + self.sla_failures;
        let sla_success_rate = if total_sla == 0 {
            1.0
        } else {
            self.sla_successes as f64 / total_sla as f64
        };

        let economy_stress_index = (1.0 - sla_success_rate).clamp(0.0, 1.0)
            + self.average_gate_load().clamp(0.0, 1.0)
            + self.average_price_index().max(1.0)
            - 1.0;

        CycleReport {
            cycle: self.cycle,
            sla_success_rate,
            reroute_count: self.reroute_count,
            economy_stress_index,
        }
    }

    pub fn save_snapshot(&self, path: &Path) -> Result<(), SnapshotError> {
        crate::snapshot::save_snapshot(self, path)
    }

    pub fn load_snapshot(path: &Path, config: RuntimeConfig) -> Result<Self, SnapshotError> {
        crate::snapshot::load_snapshot(path, config)
    }

    pub fn snapshot_hash(&self) -> u64 {
        crate::snapshot::snapshot_hash(self)
    }

    pub fn config(&self) -> &RuntimeConfig {
        &self.config
    }

    pub fn tick(&self) -> u64 {
        self.tick
    }

    pub fn cycle(&self) -> u64 {
        self.cycle
    }

    pub fn capital(&self) -> f64 {
        self.capital
    }

    pub fn outstanding_debt(&self) -> f64 {
        self.active_loan
            .map(|loan| loan.principal_remaining)
            .unwrap_or(0.0)
    }

    pub fn reputation(&self) -> f64 {
        self.reputation
    }

    pub fn current_loan_interest_rate(&self) -> f64 {
        self.active_loan
            .map(|loan| loan.monthly_interest_rate)
            .unwrap_or(0.0)
    }

    pub fn update_ship_policy(
        &mut self,
        ship_id: ShipId,
        policy: AutopilotPolicy,
    ) -> Result<(), CommandError> {
        let Some(ship) = self.ships.get_mut(&ship_id) else {
            return Err(CommandError::UnknownShip);
        };
        if ship.company_id != CompanyId(0) {
            return Err(CommandError::InvalidAssignment);
        }
        ship.policy = policy;
        Ok(())
    }

    pub fn cycle_report(&self) -> CycleReport {
        let total_sla = self.sla_successes + self.sla_failures;
        let sla_success_rate = if total_sla == 0 {
            1.0
        } else {
            self.sla_successes as f64 / total_sla as f64
        };

        let average_gate_load = if self.world.edges.is_empty() {
            0.0
        } else {
            self.world
                .edges
                .iter()
                .map(|edge| {
                    let load = self.gate_queue_load.get(&edge.id).copied().unwrap_or(0.0);
                    let effective_capacity = (edge.base_capacity * edge.capacity_factor).max(1.0);
                    load / effective_capacity
                })
                .sum::<f64>()
                / self.world.edges.len() as f64
        };

        let economy_stress_index = (1.0 - sla_success_rate).clamp(0.0, 1.0)
            + average_gate_load.clamp(0.0, 1.0)
            + self.average_price_index().max(1.0)
            - 1.0;

        CycleReport {
            cycle: self.cycle,
            sla_success_rate,
            reroute_count: self.reroute_count,
            economy_stress_index,
        }
    }

    pub(crate) fn snapshot_state(&self) -> crate::snapshot::SnapshotState {
        crate::snapshot::SnapshotState {
            world_seed: self.config.galaxy.seed,
            tick: self.tick,
            cycle: self.cycle,
            capital: self.capital,
            active_loan: self.active_loan,
            outstanding_debt: self.outstanding_debt,
            reputation: self.reputation,
            current_loan_interest_rate: self.current_loan_interest_rate,
            queue_delay_accumulator: self.queue_delay_accumulator,
            reroute_count: self.reroute_count,
            sla_successes: self.sla_successes,
            sla_failures: self.sla_failures,
            next_offer_id: self.next_offer_id,
            next_trade_order_id: self.next_trade_order_id,
            edges: self
                .world
                .edges
                .iter()
                .map(|edge| crate::snapshot::EdgeSnapshot {
                    gate_id: edge.id,
                    capacity_factor: edge.capacity_factor,
                    blocked_until_tick: edge.blocked_until_tick,
                })
                .collect(),
            companies: self.companies.values().cloned().collect(),
            company_runtimes: self.npc_company_runtimes.values().cloned().collect(),
            markets: self
                .markets
                .iter()
                .map(|(station_id, book)| crate::snapshot::MarketBookSnapshot {
                    station_id: *station_id,
                    goods: book
                        .goods
                        .iter()
                        .map(|(commodity, state)| crate::snapshot::MarketGoodSnapshot {
                            commodity: *commodity,
                            state: state.clone(),
                        })
                        .collect(),
                })
                .collect(),
            contracts: self.contracts.values().cloned().collect(),
            contract_offers: self.contract_offers.values().cloned().collect(),
            trade_orders: self.trade_orders.values().cloned().collect(),
            ships: self.ships.values().cloned().collect(),
            milestones: self.milestones.clone(),
            gate_traversals_cycle: self
                .gate_traversals_cycle
                .iter()
                .map(
                    |(gate_id, by_company)| crate::snapshot::GateTraversalSnapshot {
                        gate_id: *gate_id,
                        by_company: by_company
                            .iter()
                            .map(
                                |(company_id, count)| crate::snapshot::CompanyTraversalSnapshot {
                                    company_id: *company_id,
                                    count: *count,
                                },
                            )
                            .collect(),
                    },
                )
                .collect(),
            gate_traversals_window: self
                .gate_traversals_window
                .iter()
                .map(|cycle| {
                    cycle
                        .iter()
                        .map(
                            |(gate_id, by_company)| crate::snapshot::GateTraversalSnapshot {
                                gate_id: *gate_id,
                                by_company: by_company
                                    .iter()
                                    .map(|(company_id, count)| {
                                        crate::snapshot::CompanyTraversalSnapshot {
                                            company_id: *company_id,
                                            count: *count,
                                        }
                                    })
                                    .collect(),
                            },
                        )
                        .collect()
                })
                .collect(),
            gate_queue_load: self
                .gate_queue_load
                .iter()
                .map(|(gate_id, load)| crate::snapshot::GateLoadSnapshot {
                    gate_id: *gate_id,
                    load: *load,
                })
                .collect(),
            ship_kpis: self
                .ships
                .keys()
                .copied()
                .map(|ship_id| crate::snapshot::ShipKpiSnapshot {
                    ship_id,
                    idle_ticks_cycle: self
                        .ship_idle_ticks_cycle
                        .get(&ship_id)
                        .copied()
                        .unwrap_or(0),
                    delay_ticks_cycle: self
                        .ship_delay_ticks_cycle
                        .get(&ship_id)
                        .copied()
                        .unwrap_or(0),
                    runs_completed: self.ship_runs_completed.get(&ship_id).copied().unwrap_or(0),
                    profit_earned: self
                        .ship_profit_earned
                        .get(&ship_id)
                        .copied()
                        .unwrap_or(0.0),
                })
                .collect(),
            previous_cycle_prices: self
                .previous_cycle_prices
                .iter()
                .map(
                    |((station_id, commodity), price)| crate::snapshot::PreviousPriceSnapshot {
                        station_id: *station_id,
                        commodity: *commodity,
                        price: *price,
                    },
                )
                .collect(),
            modifiers: self
                .modifiers
                .iter()
                .map(|modifier| crate::snapshot::ActiveModifierSnapshot {
                    until_tick: modifier.until_tick,
                    gate: modifier.gate,
                    risk: modifier.risk,
                    magnitude: modifier.magnitude,
                })
                .collect(),
        }
    }

    pub(crate) fn from_snapshot_state(
        config: RuntimeConfig,
        state: crate::snapshot::SnapshotState,
    ) -> Self {
        let crate::snapshot::SnapshotState {
            world_seed,
            tick,
            cycle,
            capital,
            active_loan,
            outstanding_debt,
            reputation,
            current_loan_interest_rate,
            queue_delay_accumulator,
            reroute_count,
            sla_successes,
            sla_failures,
            next_offer_id,
            next_trade_order_id,
            edges,
            companies,
            company_runtimes,
            markets,
            contracts,
            contract_offers,
            trade_orders,
            ships,
            milestones,
            gate_traversals_cycle,
            gate_traversals_window,
            gate_queue_load,
            ship_kpis,
            previous_cycle_prices,
            modifiers,
        } = state;

        let mut config = config;
        config.galaxy.seed = world_seed;
        let mut simulation = Simulation::new(config, world_seed);
        simulation.tick = tick;
        simulation.cycle = cycle;
        simulation.capital = capital;
        simulation.active_loan = active_loan;
        simulation.outstanding_debt = outstanding_debt;
        simulation.reputation = reputation;
        simulation.current_loan_interest_rate = current_loan_interest_rate;
        simulation.queue_delay_accumulator = queue_delay_accumulator;
        simulation.reroute_count = reroute_count;
        simulation.sla_successes = sla_successes;
        simulation.sla_failures = sla_failures;
        simulation.next_offer_id = next_offer_id;
        simulation.next_trade_order_id = next_trade_order_id;

        for edge_state in edges {
            if let Some(edge) = simulation
                .world
                .edges
                .iter_mut()
                .find(|edge| edge.id == edge_state.gate_id)
            {
                edge.capacity_factor = edge_state.capacity_factor;
                edge.blocked_until_tick = edge_state.blocked_until_tick;
            }
        }

        simulation.companies = companies
            .into_iter()
            .map(|company| (company.id, company))
            .collect();
        simulation.npc_company_runtimes = company_runtimes
            .into_iter()
            .map(|runtime| (runtime.company_id, runtime))
            .collect();
        simulation.markets = markets
            .into_iter()
            .map(|book| {
                (
                    book.station_id,
                    MarketBook {
                        goods: book
                            .goods
                            .into_iter()
                            .map(|entry| (entry.commodity, entry.state))
                            .collect(),
                    },
                )
            })
            .collect();
        simulation.contracts = contracts
            .into_iter()
            .map(|contract| (contract.id, contract))
            .collect();
        simulation.contract_offers = contract_offers
            .into_iter()
            .map(|offer| (offer.id, offer))
            .collect();
        simulation.trade_orders = trade_orders
            .into_iter()
            .map(|order| (order.id, order))
            .collect();
        simulation.ships = ships.into_iter().map(|ship| (ship.id, ship)).collect();
        simulation.milestones = milestones;
        simulation.gate_traversals_cycle = gate_traversals_cycle
            .into_iter()
            .map(|entry| {
                (
                    entry.gate_id,
                    entry
                        .by_company
                        .into_iter()
                        .map(|company| (company.company_id, company.count))
                        .collect(),
                )
            })
            .collect();
        simulation.gate_traversals_window = gate_traversals_window
            .into_iter()
            .map(|cycle| {
                cycle
                    .into_iter()
                    .map(|entry| {
                        (
                            entry.gate_id,
                            entry
                                .by_company
                                .into_iter()
                                .map(|company| (company.company_id, company.count))
                                .collect(),
                        )
                    })
                    .collect()
            })
            .collect();
        simulation.gate_queue_load = gate_queue_load
            .into_iter()
            .map(|entry| (entry.gate_id, entry.load))
            .collect();
        simulation.ship_idle_ticks_cycle = ship_kpis
            .iter()
            .map(|entry| (entry.ship_id, entry.idle_ticks_cycle))
            .collect();
        simulation.ship_delay_ticks_cycle = ship_kpis
            .iter()
            .map(|entry| (entry.ship_id, entry.delay_ticks_cycle))
            .collect();
        simulation.ship_runs_completed = ship_kpis
            .iter()
            .map(|entry| (entry.ship_id, entry.runs_completed))
            .collect();
        simulation.ship_profit_earned = ship_kpis
            .into_iter()
            .map(|entry| (entry.ship_id, entry.profit_earned))
            .collect();
        simulation.previous_cycle_prices = previous_cycle_prices
            .into_iter()
            .map(|entry| ((entry.station_id, entry.commodity), entry.price))
            .collect();
        simulation.modifiers = modifiers
            .into_iter()
            .map(|modifier| ActiveModifier {
                until_tick: modifier.until_tick,
                gate: modifier.gate,
                risk: modifier.risk,
                magnitude: modifier.magnitude,
            })
            .collect();
        simulation.normalize_player_ship_roster();
        simulation
    }
}

#[derive(Debug)]
pub enum SnapshotError {
    Io(String),
    Parse(String),
}

impl Display for SnapshotError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(v) | Self::Parse(v) => write!(f, "{v}"),
        }
    }
}

impl std::error::Error for SnapshotError {}
