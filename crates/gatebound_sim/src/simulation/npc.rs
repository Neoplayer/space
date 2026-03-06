use std::collections::VecDeque;

use gatebound_domain::*;

use super::state::Simulation;

const NPC_PLAN_INTERVAL_TICKS: u64 = 10;

#[derive(Debug, Clone)]
struct PlannedCompanyOrder {
    ship_id: ShipId,
    commodity: Commodity,
    amount: f64,
    expected_profit: f64,
    source_station: StationId,
    destination_station: StationId,
    route_to_source: RoutePlan,
    route_to_destination: RoutePlan,
}

impl Simulation {
    pub(in crate::simulation) fn dispatch_npc_trade_orders(&mut self) {
        let mut company_ids = self
            .npc_company_runtimes
            .keys()
            .copied()
            .collect::<Vec<_>>();
        company_ids.sort_by_key(|company_id| company_id.0);
        for company_id in company_ids {
            let due = self
                .npc_company_runtimes
                .get(&company_id)
                .is_some_and(|runtime| self.tick >= runtime.next_plan_tick);
            if !due {
                continue;
            }

            self.plan_company_orders(company_id);

            if let Some(runtime) = self.npc_company_runtimes.get_mut(&company_id) {
                runtime.next_plan_tick = self.tick.saturating_add(NPC_PLAN_INTERVAL_TICKS);
            }
        }
    }

    pub(in crate::simulation) fn plan_company_orders(&mut self, company_id: CompanyId) {
        if !self.npc_company_runtimes.contains_key(&company_id) {
            return;
        }

        let idle_ships = self
            .ships
            .values()
            .filter(|ship| {
                ship.company_id == company_id
                    && ship.role == ShipRole::NpcTrade
                    && ship.active_contract.is_none()
                    && ship.trade_order_id.is_none()
                    && ship.segment_eta_remaining == 0
                    && ship.movement_queue.is_empty()
                    && ship.current_segment_kind.is_none()
            })
            .map(|ship| ship.id)
            .collect::<Vec<_>>();

        let mut plans = idle_ships
            .into_iter()
            .filter_map(|ship_id| self.best_plan_for_company_ship(ship_id))
            .collect::<Vec<_>>();
        plans.sort_by(|a, b| {
            b.expected_profit
                .total_cmp(&a.expected_profit)
                .then_with(|| a.ship_id.0.cmp(&b.ship_id.0))
        });

        for plan in plans {
            self.assign_company_trade_order(company_id, plan);
        }
    }

    fn best_plan_for_company_ship(&self, ship_id: ShipId) -> Option<PlannedCompanyOrder> {
        let ship_snapshot = self.ships.get(&ship_id)?.clone();
        let ship_station = ship_snapshot
            .current_station
            .or_else(|| self.world.first_station(ship_snapshot.location))?;
        let station_ids = self
            .world
            .stations
            .iter()
            .map(|station| station.id)
            .collect::<Vec<_>>();

        let mut best: Option<PlannedCompanyOrder> = None;
        for destination_station in &station_ids {
            for commodity in Commodity::ALL {
                let deficit = self
                    .markets
                    .get(destination_station)
                    .and_then(|book| book.goods.get(&commodity))
                    .map(|state| (state.target_stock * 0.85 - state.stock).max(0.0))
                    .unwrap_or(0.0);
                if deficit <= 0.0 {
                    continue;
                }

                for source_station in &station_ids {
                    if source_station == destination_station {
                        continue;
                    }

                    let source_state = self
                        .markets
                        .get(source_station)
                        .and_then(|book| book.goods.get(&commodity));
                    let destination_state = self
                        .markets
                        .get(destination_station)
                        .and_then(|book| book.goods.get(&commodity));
                    let Some(source_state) = source_state else {
                        continue;
                    };
                    let Some(destination_state) = destination_state else {
                        continue;
                    };

                    let surplus = (source_state.stock - source_state.target_stock * 1.15).max(0.0);
                    if surplus <= 0.0 {
                        continue;
                    }

                    let amount = deficit.min(surplus).min(ship_snapshot.cargo_capacity);
                    if amount <= 0.0 {
                        continue;
                    }

                    let policy = AutopilotPolicy {
                        max_hops: 6,
                        ..AutopilotPolicy::default()
                    };
                    let Some(route_to_source) = self.build_station_route_with_speed(
                        ship_station,
                        *source_station,
                        policy.clone(),
                        ship_snapshot.sub_light_speed,
                    ) else {
                        continue;
                    };
                    let Some(route_to_destination) = self.build_station_route_with_speed(
                        *source_station,
                        *destination_station,
                        policy,
                        ship_snapshot.sub_light_speed,
                    ) else {
                        continue;
                    };

                    let expected_profit = self.trade_sale_net(amount, destination_state.price)
                        - self.trade_purchase_total(amount, source_state.price)
                        - self.estimated_gate_fees(&route_to_source)
                        - self.estimated_gate_fees(&route_to_destination);
                    if expected_profit <= 0.0 {
                        continue;
                    }

                    let plan = PlannedCompanyOrder {
                        ship_id,
                        commodity,
                        amount,
                        expected_profit,
                        source_station: *source_station,
                        destination_station: *destination_station,
                        route_to_source,
                        route_to_destination,
                    };
                    let replace = best.as_ref().is_none_or(|current| {
                        expected_profit > current.expected_profit
                            || (expected_profit == current.expected_profit
                                && plan.route_to_destination.eta_ticks
                                    < current.route_to_destination.eta_ticks)
                    });
                    if replace {
                        best = Some(plan);
                    }
                }
            }
        }

        best
    }

    fn assign_company_trade_order(&mut self, company_id: CompanyId, plan: PlannedCompanyOrder) {
        let order_id = TradeOrderId(self.next_trade_order_id);
        self.next_trade_order_id = self.next_trade_order_id.saturating_add(1);
        self.trade_orders.insert(
            order_id,
            TradeOrder {
                id: order_id,
                company_id,
                ship_id: plan.ship_id,
                commodity: plan.commodity,
                amount: plan.amount,
                purchased_amount: 0.0,
                cost_basis_total: 0.0,
                gate_fees_accrued: 0.0,
                source_station: plan.source_station,
                destination_station: plan.destination_station,
                stage: TradeOrderStage::ToPickup,
            },
        );

        if let Some(ship) = self.ships.get_mut(&plan.ship_id) {
            ship.trade_order_id = Some(order_id);
            ship.cargo = None;
            ship.policy.max_hops = 6;
        }

        let at_source = self
            .ships
            .get(&plan.ship_id)
            .is_some_and(|ship| ship.current_station == Some(plan.source_station));
        if at_source {
            let _ = self.advance_npc_trade_ship(plan.ship_id);
            return;
        }

        self.apply_npc_route(plan.ship_id, &plan.route_to_source);
    }

    fn apply_npc_route(&mut self, ship_id: ShipId, route: &RoutePlan) {
        if let Some(ship) = self.ships.get_mut(&ship_id) {
            ship.last_risk_score = route.risk_score;
            ship.movement_queue = VecDeque::from(route.segments.clone());
            ship.planned_path = route.segments.iter().map(|segment| segment.to).collect();
            ship.segment_eta_remaining = 0;
            ship.segment_progress_total = 0;
            ship.current_segment_kind = None;
            ship.current_target = None;
            ship.eta_ticks_remaining = 0;
            ship.last_gate_arrival = None;
        }
    }

    fn clear_trade_order_for_ship(&mut self, ship_id: ShipId, order_id: TradeOrderId) {
        self.trade_orders.remove(&order_id);
        if let Some(ship) = self.ships.get_mut(&ship_id) {
            ship.trade_order_id = None;
            ship.cargo = None;
            ship.movement_queue.clear();
            ship.planned_path.clear();
            ship.segment_eta_remaining = 0;
            ship.segment_progress_total = 0;
            ship.current_segment_kind = None;
            ship.current_target = None;
            ship.eta_ticks_remaining = 0;
        }
    }

    pub(in crate::simulation) fn advance_npc_trade_ship(&mut self, ship_id: ShipId) -> bool {
        let Some(order_id) = self
            .ships
            .get(&ship_id)
            .and_then(|ship| ship.trade_order_id)
        else {
            return false;
        };
        let Some(order) = self.trade_orders.get(&order_id).cloned() else {
            if let Some(ship) = self.ships.get_mut(&ship_id) {
                ship.trade_order_id = None;
            }
            return false;
        };
        let Some(ship_snapshot) = self.ships.get(&ship_id).cloned() else {
            return false;
        };
        let at_station = ship_snapshot.current_station;

        match order.stage {
            TradeOrderStage::ToPickup => {
                if at_station == Some(order.source_station) {
                    let Some(price) = self
                        .markets
                        .get(&order.source_station)
                        .and_then(|book| book.goods.get(&order.commodity))
                        .map(|state| state.price)
                    else {
                        self.clear_trade_order_for_ship(ship_id, order_id);
                        return false;
                    };
                    let available = self
                        .markets
                        .get(&order.source_station)
                        .and_then(|book| book.goods.get(&order.commodity))
                        .map(|state| state.stock)
                        .unwrap_or(0.0);
                    let amount = available
                        .min(order.amount)
                        .min(ship_snapshot.cargo_capacity);
                    if amount <= 0.0 {
                        self.clear_trade_order_for_ship(ship_id, order_id);
                        return false;
                    }

                    if let Some(state) = self
                        .markets
                        .get_mut(&order.source_station)
                        .and_then(|book| book.goods.get_mut(&order.commodity))
                    {
                        state.stock = (state.stock - amount).max(0.0);
                        state.cycle_outflow += amount;
                    }

                    let total_cost = self.trade_purchase_total(amount, price);
                    self.apply_company_balance_delta(order.company_id, -total_cost);

                    if let Some(item) = self.trade_orders.get_mut(&order_id) {
                        item.purchased_amount = amount;
                        item.cost_basis_total = total_cost;
                        item.stage = TradeOrderStage::ToDropoff;
                    }
                    if let Some(ship) = self.ships.get_mut(&ship_id) {
                        ship.cargo = Some(CargoLoad {
                            commodity: order.commodity,
                            amount,
                            source: CargoSource::Spot,
                        });
                    }

                    let Some(route) = self.build_station_route_with_speed(
                        order.source_station,
                        order.destination_station,
                        AutopilotPolicy {
                            max_hops: 6,
                            ..AutopilotPolicy::default()
                        },
                        ship_snapshot.sub_light_speed,
                    ) else {
                        return true;
                    };
                    self.apply_npc_route(ship_id, &route);
                    return true;
                }

                let from_station = ship_snapshot
                    .current_station
                    .or_else(|| self.world.first_station(ship_snapshot.location));
                let Some(from_station) = from_station else {
                    self.clear_trade_order_for_ship(ship_id, order_id);
                    return false;
                };
                let Some(route) = self.build_station_route_with_speed(
                    from_station,
                    order.source_station,
                    AutopilotPolicy {
                        max_hops: 6,
                        ..AutopilotPolicy::default()
                    },
                    ship_snapshot.sub_light_speed,
                ) else {
                    self.clear_trade_order_for_ship(ship_id, order_id);
                    return false;
                };
                self.apply_npc_route(ship_id, &route);
                true
            }
            TradeOrderStage::ToDropoff => {
                if at_station == Some(order.destination_station) {
                    let delivered_amount = ship_snapshot
                        .cargo
                        .map(|cargo| cargo.amount)
                        .unwrap_or(order.purchased_amount);
                    if delivered_amount <= 0.0 {
                        self.clear_trade_order_for_ship(ship_id, order_id);
                        return false;
                    }

                    let Some(price) = self
                        .markets
                        .get(&order.destination_station)
                        .and_then(|book| book.goods.get(&order.commodity))
                        .map(|state| state.price)
                    else {
                        self.clear_trade_order_for_ship(ship_id, order_id);
                        return false;
                    };

                    if let Some(state) = self
                        .markets
                        .get_mut(&order.destination_station)
                        .and_then(|book| book.goods.get_mut(&order.commodity))
                    {
                        state.stock += delivered_amount;
                        state.cycle_inflow += delivered_amount;
                    }

                    let net_revenue = self.trade_sale_net(delivered_amount, price);
                    self.apply_company_balance_delta(order.company_id, net_revenue);
                    let realized_profit =
                        net_revenue - order.cost_basis_total - order.gate_fees_accrued;
                    if let Some(runtime) = self.npc_company_runtimes.get_mut(&order.company_id) {
                        runtime.last_realized_profit = realized_profit;
                    }
                    self.record_ship_profit(ship_id, realized_profit);
                    self.clear_trade_order_for_ship(ship_id, order_id);
                    return true;
                }

                let from_station = ship_snapshot
                    .current_station
                    .or_else(|| self.world.first_station(ship_snapshot.location));
                let Some(from_station) = from_station else {
                    self.clear_trade_order_for_ship(ship_id, order_id);
                    return false;
                };
                let Some(route) = self.build_station_route_with_speed(
                    from_station,
                    order.destination_station,
                    AutopilotPolicy {
                        max_hops: 6,
                        ..AutopilotPolicy::default()
                    },
                    ship_snapshot.sub_light_speed,
                ) else {
                    self.clear_trade_order_for_ship(ship_id, order_id);
                    return false;
                };
                self.apply_npc_route(ship_id, &route);
                true
            }
        }
    }

    pub(in crate::simulation) fn trade_purchase_total(
        &self,
        quantity: f64,
        unit_price: f64,
    ) -> f64 {
        let gross = quantity * unit_price;
        gross * (1.0 + self.config.pressure.market_fee_rate)
    }

    pub(in crate::simulation) fn trade_sale_net(&self, quantity: f64, unit_price: f64) -> f64 {
        let gross = quantity * unit_price;
        gross * (1.0 - self.config.pressure.market_fee_rate)
    }

    fn estimated_gate_fees(&self, route: &RoutePlan) -> f64 {
        route
            .segments
            .iter()
            .filter(|segment| segment.kind == SegmentKind::Warp)
            .count() as f64
            * self.config.pressure.gate_fee_per_jump
    }

    pub(in crate::simulation) fn apply_company_balance_delta(
        &mut self,
        company_id: CompanyId,
        delta: f64,
    ) {
        if let Some(runtime) = self.npc_company_runtimes.get_mut(&company_id) {
            runtime.balance += delta;
        }
    }
}
