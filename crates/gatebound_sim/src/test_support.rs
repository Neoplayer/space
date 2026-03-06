use crate::Simulation;
use gatebound_domain::*;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FinanceStateFixture {
    pub active_loan: Option<ActiveLoan>,
    pub reputation: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShipCycleMetricsFixture {
    pub idle_ticks_cycle: u32,
    pub delay_ticks_cycle: u32,
    pub runs_completed: u32,
    pub profit_earned: f64,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct MarketStatePatch {
    pub base_price: Option<f64>,
    pub price: Option<f64>,
    pub stock: Option<f64>,
    pub target_stock: Option<f64>,
    pub cycle_inflow: Option<f64>,
    pub cycle_outflow: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ShipPatch {
    pub company_id: Option<CompanyId>,
    pub role: Option<ShipRole>,
    pub location: Option<SystemId>,
    pub current_station: Option<Option<StationId>>,
    pub eta_ticks_remaining: Option<u32>,
    pub sub_light_speed: Option<f64>,
    pub cargo_capacity: Option<f64>,
    pub cargo: Option<Option<CargoLoad>>,
    pub trade_order_id: Option<Option<TradeOrderId>>,
    pub movement_queue: Option<Vec<RouteSegment>>,
    pub segment_eta_remaining: Option<u32>,
    pub segment_progress_total: Option<u32>,
    pub current_segment_kind: Option<Option<SegmentKind>>,
    pub active_contract: Option<Option<ContractId>>,
    pub route_cursor: Option<usize>,
    pub policy: Option<AutopilotPolicy>,
    pub planned_path: Option<Vec<SystemId>>,
    pub current_target: Option<Option<SystemId>>,
    pub last_gate_arrival: Option<Option<GateId>>,
    pub last_risk_score: Option<f64>,
    pub reroutes: Option<u64>,
    pub descriptor: Option<ShipDescriptor>,
    pub modules: Option<Vec<ShipModule>>,
    pub technical_state: Option<ShipTechnicalState>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EdgeFixture {
    pub gate_id: GateId,
    pub from_system: SystemId,
    pub to_system: SystemId,
}

#[derive(Debug, Clone)]
pub struct SimulationScenarioBuilder {
    simulation: Simulation,
}

impl SimulationScenarioBuilder {
    pub fn new(config: RuntimeConfig, seed: u64) -> Self {
        Self {
            simulation: Simulation::new(config, seed),
        }
    }

    pub fn stage_a(seed: u64) -> Self {
        Self::new(RuntimeConfig::default(), seed)
    }

    pub fn player_ship_id(&self) -> Option<ShipId> {
        self.simulation.fleet_panel_view().default_player_ship_id
    }

    pub fn first_ship_id(&self) -> Option<ShipId> {
        self.simulation
            .fleet_panel_view()
            .rows
            .iter()
            .map(|row| row.ship_id)
            .min_by_key(|ship_id| ship_id.0)
    }

    pub fn first_npc_ship_id(&self) -> Option<ShipId> {
        self.simulation
            .fleet_panel_view()
            .rows
            .iter()
            .find(|row| row.role == ShipRole::NpcTrade)
            .map(|row| row.ship_id)
    }

    pub fn stations_in_system(&self, system_id: SystemId) -> Vec<StationId> {
        self.simulation
            .camera_topology_view()
            .systems
            .into_iter()
            .find(|system| system.system_id == system_id)
            .map(|system| {
                system
                    .stations
                    .into_iter()
                    .map(|station| station.station_id)
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn first_edge(&self) -> Option<EdgeFixture> {
        self.simulation
            .world_render_snapshot()
            .edges
            .first()
            .map(|edge| EdgeFixture {
                gate_id: edge.gate_id,
                from_system: edge.from_system,
                to_system: edge.to_system,
            })
    }

    pub fn station_coords(&self, station_id: StationId) -> Option<(f64, f64)> {
        self.simulation
            .camera_topology_view()
            .systems
            .into_iter()
            .flat_map(|system| system.stations.into_iter())
            .find(|station| station.station_id == station_id)
            .map(|station| (station.x, station.y))
    }

    pub fn system_position(&self, system_id: SystemId) -> Option<(f64, f64)> {
        self.simulation
            .camera_topology_view()
            .systems
            .into_iter()
            .find(|system| system.system_id == system_id)
            .map(|system| (system.x, system.y))
    }

    pub fn gate_position(&self, system_id: SystemId, gate_id: GateId) -> Option<(f64, f64)> {
        self.simulation
            .world_render_snapshot()
            .systems
            .into_iter()
            .find(|system| system.system_id == system_id)
            .and_then(|system| {
                system
                    .gate_nodes
                    .into_iter()
                    .find(|gate| gate.gate_id == gate_id)
                    .map(|gate| (gate.x, gate.y))
            })
    }

    pub fn first_station_in_system(&self, system_id: SystemId) -> Option<StationId> {
        self.stations_in_system(system_id).into_iter().next()
    }

    pub fn with_contract_offer(&mut self, offer: ContractOffer) -> &mut Self {
        self.simulation
            .test_support_contract_offers_mut()
            .insert(offer.id, offer);
        self
    }

    pub fn with_ship_patch(&mut self, ship_id: ShipId, patch: ShipPatch) -> &mut Self {
        if let Some(ship) = self.simulation.test_support_ships_mut().get_mut(&ship_id) {
            if let Some(company_id) = patch.company_id {
                ship.company_id = company_id;
            }
            if let Some(role) = patch.role {
                ship.role = role;
            }
            if let Some(location) = patch.location {
                ship.location = location;
            }
            if let Some(current_station) = patch.current_station {
                ship.current_station = current_station;
            }
            if let Some(eta_ticks_remaining) = patch.eta_ticks_remaining {
                ship.eta_ticks_remaining = eta_ticks_remaining;
            }
            if let Some(sub_light_speed) = patch.sub_light_speed {
                ship.sub_light_speed = sub_light_speed;
            }
            if let Some(cargo_capacity) = patch.cargo_capacity {
                ship.cargo_capacity = cargo_capacity;
            }
            if let Some(cargo) = patch.cargo {
                ship.cargo = cargo;
            }
            if let Some(trade_order_id) = patch.trade_order_id {
                ship.trade_order_id = trade_order_id;
            }
            if let Some(movement_queue) = patch.movement_queue {
                ship.movement_queue = movement_queue.into();
            }
            if let Some(segment_eta_remaining) = patch.segment_eta_remaining {
                ship.segment_eta_remaining = segment_eta_remaining;
            }
            if let Some(segment_progress_total) = patch.segment_progress_total {
                ship.segment_progress_total = segment_progress_total;
            }
            if let Some(current_segment_kind) = patch.current_segment_kind {
                ship.current_segment_kind = current_segment_kind;
            }
            if let Some(active_contract) = patch.active_contract {
                ship.active_contract = active_contract;
            }
            if let Some(route_cursor) = patch.route_cursor {
                ship.route_cursor = route_cursor;
            }
            if let Some(policy) = patch.policy {
                ship.policy = policy;
            }
            if let Some(planned_path) = patch.planned_path {
                ship.planned_path = planned_path;
            }
            if let Some(current_target) = patch.current_target {
                ship.current_target = current_target;
            }
            if let Some(last_gate_arrival) = patch.last_gate_arrival {
                ship.last_gate_arrival = last_gate_arrival;
            }
            if let Some(last_risk_score) = patch.last_risk_score {
                ship.last_risk_score = last_risk_score;
            }
            if let Some(reroutes) = patch.reroutes {
                ship.reroutes = reroutes;
            }
            if let Some(descriptor) = patch.descriptor {
                ship.descriptor = descriptor;
            }
            if let Some(modules) = patch.modules {
                ship.modules = modules;
            }
            if let Some(technical_state) = patch.technical_state {
                ship.technical_state = technical_state;
            }
        }
        self
    }

    pub fn dock_ship_at(&mut self, ship_id: ShipId, station_id: StationId) -> &mut Self {
        let location = self
            .simulation
            .camera_topology_view()
            .systems
            .into_iter()
            .find(|system| {
                system
                    .stations
                    .iter()
                    .any(|station| station.station_id == station_id)
            })
            .map(|system| system.system_id);
        self.with_ship_patch(
            ship_id,
            ShipPatch {
                location,
                current_station: Some(Some(station_id)),
                eta_ticks_remaining: Some(0),
                movement_queue: Some(Vec::new()),
                segment_eta_remaining: Some(0),
                segment_progress_total: Some(0),
                current_segment_kind: Some(None),
                current_target: Some(None),
                ..ShipPatch::default()
            },
        )
    }

    pub fn with_ship_cycle_metrics(
        &mut self,
        ship_id: ShipId,
        metrics: ShipCycleMetricsFixture,
    ) -> &mut Self {
        self.simulation
            .test_support_ship_idle_ticks_cycle_mut()
            .insert(ship_id, metrics.idle_ticks_cycle);
        self.simulation
            .test_support_ship_delay_ticks_cycle_mut()
            .insert(ship_id, metrics.delay_ticks_cycle);
        self.simulation
            .test_support_ship_runs_completed_mut()
            .insert(ship_id, metrics.runs_completed);
        self.simulation
            .test_support_ship_profit_earned_mut()
            .insert(ship_id, metrics.profit_earned);
        self
    }

    pub fn with_market_state_patch(
        &mut self,
        station_id: StationId,
        commodity: Commodity,
        patch: MarketStatePatch,
    ) -> &mut Self {
        if let Some(state) = self
            .simulation
            .test_support_markets_mut()
            .get_mut(&station_id)
            .and_then(|book| book.goods.get_mut(&commodity))
        {
            if let Some(base_price) = patch.base_price {
                state.base_price = base_price;
            }
            if let Some(price) = patch.price {
                state.price = price;
            }
            if let Some(stock) = patch.stock {
                state.stock = stock;
            }
            if let Some(target_stock) = patch.target_stock {
                state.target_stock = target_stock;
            }
            if let Some(cycle_inflow) = patch.cycle_inflow {
                state.cycle_inflow = cycle_inflow;
            }
            if let Some(cycle_outflow) = patch.cycle_outflow {
                state.cycle_outflow = cycle_outflow;
            }
        }
        self
    }

    pub fn with_finance_state(&mut self, fixture: FinanceStateFixture) -> &mut Self {
        self.simulation.active_loan = fixture.active_loan;
        self.simulation.outstanding_debt = fixture
            .active_loan
            .map(|loan| loan.principal_remaining)
            .unwrap_or(0.0);
        self.simulation.current_loan_interest_rate = fixture
            .active_loan
            .map(|loan| loan.monthly_interest_rate)
            .unwrap_or(0.0);
        self.simulation.reputation = fixture.reputation;
        self
    }

    pub fn build(self) -> Simulation {
        self.simulation
    }
}
