use std::collections::{BTreeMap, VecDeque};

use crate::views::{
    FreightDemand, FreightOrder, FreightSupply, OrderReservation, PlannerDiagnostics, PlannerMode,
    ShipBid,
};
use gatebound_domain::*;

use super::state::Simulation;

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

#[derive(Debug, Default)]
struct ReservationMaps {
    source_reserved: BTreeMap<(StationId, Commodity), f64>,
    demand_reserved: BTreeMap<(StationId, Commodity), f64>,
    lane_reserved: BTreeMap<(StationId, StationId, Commodity), f64>,
    lane_ship_counts: BTreeMap<(StationId, StationId, Commodity), usize>,
    reservations: Vec<OrderReservation>,
}

#[derive(Debug, Clone)]
struct PlannedShipBid {
    order_idx: usize,
    plan: PlannedCompanyOrder,
    bid: ShipBid,
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
                runtime.next_plan_tick = self
                    .tick
                    .saturating_add(self.planner_settings.planning_interval_ticks.max(1));
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
                    && ship.trade_order_id.is_none()
                    && ship.segment_eta_remaining == 0
                    && ship.movement_queue.is_empty()
                    && ship.current_segment_kind.is_none()
            })
            .map(|ship| ship.id)
            .collect::<Vec<_>>();

        match self.planner_mode {
            PlannerMode::GreedyCurrent => {
                self.planner_diagnostics = PlannerDiagnostics {
                    mode: Some(self.planner_mode),
                    ..PlannerDiagnostics::default()
                };
                let mut plans = idle_ships
                    .into_iter()
                    .filter_map(|ship_id| self.best_plan_for_company_ship_greedy(ship_id))
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
            PlannerMode::GlobalOnly => {
                self.plan_company_orders_with_order_book(company_id, idle_ships, false)
            }
            PlannerMode::HybridRecommended => {
                self.plan_company_orders_with_order_book(company_id, idle_ships, true)
            }
        }
    }

    fn best_plan_for_company_ship_greedy(&self, ship_id: ShipId) -> Option<PlannedCompanyOrder> {
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
                        max_hops: super::stage_a_route_hop_limit(&self.world),
                        ..AutopilotPolicy::default()
                    };
                    let Some(route_to_source) = self.build_station_route_with_speed_legacy(
                        ship_station,
                        *source_station,
                        policy.clone(),
                        ship_snapshot.sub_light_speed,
                    ) else {
                        continue;
                    };
                    let Some(route_to_destination) = self.build_station_route_with_speed_legacy(
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

    fn plan_company_orders_with_order_book(
        &mut self,
        company_id: CompanyId,
        idle_ships: Vec<ShipId>,
        hybrid_local_bidding: bool,
    ) {
        let (mut demands, mut supplies, mut orders, mut reservation_maps) =
            self.build_freight_order_book();
        let mut bids = Vec::new();

        if hybrid_local_bidding {
            let mut sorted_ships = idle_ships;
            sorted_ships.sort_by_key(|ship_id| ship_id.0);
            for ship_id in sorted_ships {
                let Some(planned) = self.best_bid_for_ship(ship_id, &orders, &reservation_maps)
                else {
                    continue;
                };
                Self::apply_bid_to_order_book(
                    &mut orders,
                    &mut demands,
                    &mut supplies,
                    &mut reservation_maps,
                    &planned,
                );
                bids.push(planned.bid.clone());
                self.assign_company_trade_order(company_id, planned.plan);
            }
        } else {
            let mut assignments = self
                .all_bids_for_idle_ships(idle_ships, &orders, &reservation_maps)
                .into_iter()
                .collect::<Vec<_>>();
            assignments.sort_by(|left, right| {
                right
                    .bid
                    .score
                    .total_cmp(&left.bid.score)
                    .then_with(|| left.plan.ship_id.0.cmp(&right.plan.ship_id.0))
            });

            let mut assigned_ships = BTreeMap::<ShipId, ()>::new();
            for planned in assignments {
                if assigned_ships.contains_key(&planned.plan.ship_id) {
                    continue;
                }
                let Some(order) = orders.get(planned.order_idx) else {
                    continue;
                };
                if order.remaining_amount <= 1e-9 || order.assigned_ships >= order.lane_ship_cap {
                    continue;
                }
                let mut adjusted = planned.clone();
                adjusted.plan.amount = adjusted.plan.amount.min(order.remaining_amount);
                adjusted.bid.amount = adjusted.plan.amount;
                if adjusted.plan.amount <= 1e-9 {
                    continue;
                }
                Self::apply_bid_to_order_book(
                    &mut orders,
                    &mut demands,
                    &mut supplies,
                    &mut reservation_maps,
                    &adjusted,
                );
                assigned_ships.insert(adjusted.plan.ship_id, ());
                bids.push(adjusted.bid.clone());
                self.assign_company_trade_order(company_id, adjusted.plan);
            }
        }

        let demand_reserved_totals = reservation_maps.reservations.iter().fold(
            BTreeMap::<(StationId, Commodity), f64>::new(),
            |mut totals, reservation| {
                *totals
                    .entry((reservation.destination_station, reservation.commodity))
                    .or_insert(0.0) += reservation.amount;
                totals
            },
        );
        for demand in &mut demands {
            demand.reserved_amount = demand_reserved_totals
                .get(&(demand.station_id, demand.commodity))
                .copied()
                .unwrap_or(demand.reserved_amount);
        }
        let supply_reserved_totals = reservation_maps.reservations.iter().fold(
            BTreeMap::<(StationId, Commodity), f64>::new(),
            |mut totals, reservation| {
                *totals
                    .entry((reservation.source_station, reservation.commodity))
                    .or_insert(0.0) += reservation.amount;
                totals
            },
        );
        for supply in &mut supplies {
            supply.reserved_amount = supply_reserved_totals
                .get(&(supply.station_id, supply.commodity))
                .copied()
                .unwrap_or(supply.reserved_amount);
        }

        self.planner_diagnostics = PlannerDiagnostics {
            mode: Some(self.planner_mode),
            unmatched_critical_demands: demands
                .iter()
                .filter(|demand| {
                    demand.is_critical && demand.reserved_amount + 1e-9 < demand.required_amount
                })
                .count(),
            total_reserved_amount: reservation_maps
                .reservations
                .iter()
                .map(|item| item.amount)
                .sum(),
            demands,
            supplies,
            orders,
            reservations: reservation_maps.reservations,
            bids,
        };
    }

    fn build_freight_order_book(
        &mut self,
    ) -> (
        Vec<FreightDemand>,
        Vec<FreightSupply>,
        Vec<FreightOrder>,
        ReservationMaps,
    ) {
        let stress_by_system = self
            .system_market_stress_rows()
            .into_iter()
            .map(|row| (row.system_id, row.stress_score))
            .collect::<BTreeMap<_, _>>();
        let reservations = self.current_reservations();
        let avg_capacity = self.average_npc_cargo_capacity().max(1.0);

        let mut demands = Vec::new();
        for station in &self.world.stations {
            let Some(market) = self.markets.get(&station.id) else {
                continue;
            };
            for commodity in Commodity::ALL {
                let Some(state) = market.goods.get(&commodity) else {
                    continue;
                };
                let inbound_reserved = reservations
                    .demand_reserved
                    .get(&(station.id, commodity))
                    .copied()
                    .unwrap_or(0.0);
                let effective_stock = state.stock + inbound_reserved;
                let required_amount = (state.target_stock - effective_stock).max(0.0);
                if required_amount <= 1e-9 {
                    self.backlog_started_at.remove(&(station.id, commodity));
                    continue;
                }

                let coverage = if state.target_stock <= 0.0 {
                    0.0
                } else {
                    effective_stock / state.target_stock
                };
                let is_critical = state.stock <= 1e-9
                    || coverage <= self.planner_settings.emergency_stock_coverage;
                let _backlog_tick = *self
                    .backlog_started_at
                    .entry((station.id, commodity))
                    .or_insert(self.tick);
                let stress = stress_by_system
                    .get(&station.system_id)
                    .copied()
                    .unwrap_or(0.0);
                let service_gap = self.tick.saturating_sub(
                    self.last_service_tick
                        .get(&(station.id, commodity))
                        .copied()
                        .unwrap_or(0),
                ) as f64;
                let urgency_score = required_amount / state.target_stock.max(1.0) * 35.0
                    + stress * 12.0
                    + (service_gap / self.planner_settings.dispatch_window_ticks.max(1) as f64)
                        .min(4.0)
                        * 5.0
                    + if is_critical { 30.0 } else { 0.0 };
                demands.push(FreightDemand {
                    station_id: station.id,
                    commodity,
                    required_amount,
                    reserved_amount: inbound_reserved,
                    coverage,
                    urgency_score,
                    is_critical,
                });
            }
        }
        demands.sort_by(|left, right| {
            right
                .is_critical
                .cmp(&left.is_critical)
                .then_with(|| right.urgency_score.total_cmp(&left.urgency_score))
                .then_with(|| left.station_id.0.cmp(&right.station_id.0))
        });

        let mut supplies = Vec::new();
        for station in &self.world.stations {
            let Some(market) = self.markets.get(&station.id) else {
                continue;
            };
            for commodity in Commodity::ALL {
                let Some(state) = market.goods.get(&commodity) else {
                    continue;
                };
                let reserved_amount = reservations
                    .source_reserved
                    .get(&(station.id, commodity))
                    .copied()
                    .unwrap_or(0.0);
                let available_amount = (state.stock
                    - reserved_amount
                    - state.target_stock * self.planner_settings.reservation_safety_buffer)
                    .max(0.0);
                if available_amount <= 1e-9 {
                    continue;
                }
                supplies.push(FreightSupply {
                    station_id: station.id,
                    commodity,
                    available_amount,
                    reserved_amount,
                });
            }
        }
        supplies.sort_by(|left, right| {
            right
                .available_amount
                .total_cmp(&left.available_amount)
                .then_with(|| left.station_id.0.cmp(&right.station_id.0))
        });

        let mut supply_remaining = supplies
            .iter()
            .map(|supply| {
                (
                    (supply.station_id, supply.commodity),
                    supply.available_amount,
                )
            })
            .collect::<BTreeMap<_, _>>();
        let mut orders = Vec::new();
        let mut next_order_id = 0_u64;
        for demand in &demands {
            let candidate = self.best_source_for_demand(demand, &supply_remaining, avg_capacity);
            let Some((source_station, route)) = candidate else {
                continue;
            };
            let available_from_source = supply_remaining
                .get(&(source_station, demand.commodity))
                .copied()
                .unwrap_or(0.0);
            if available_from_source <= 1e-9 {
                continue;
            }
            let backlog_tick = self
                .backlog_started_at
                .get(&(demand.station_id, demand.commodity))
                .copied()
                .unwrap_or(self.tick);
            let waited_ticks = self.tick.saturating_sub(backlog_tick);
            let dispatch_ready = demand.is_critical
                || demand.required_amount
                    >= avg_capacity * self.planner_settings.minimum_load_factor
                || waited_ticks >= self.planner_settings.dispatch_window_ticks;
            let total_amount = demand.required_amount.min(available_from_source);
            let lane_ship_cap = ((total_amount / avg_capacity).ceil() as usize)
                .max(1)
                .min(self.planner_settings.lane_saturation_cap.max(1));
            orders.push(FreightOrder {
                order_id: next_order_id,
                source_station,
                destination_station: demand.station_id,
                commodity: demand.commodity,
                total_amount,
                reserved_amount: reservations
                    .lane_reserved
                    .get(&(source_station, demand.station_id, demand.commodity))
                    .copied()
                    .unwrap_or(0.0),
                remaining_amount: total_amount,
                urgency_score: demand.urgency_score
                    - route.eta_ticks as f64 * 0.12
                    - route.risk_score * 2.0,
                is_critical: demand.is_critical,
                dispatch_after_tick: if dispatch_ready {
                    self.tick
                } else {
                    backlog_tick.saturating_add(self.planner_settings.dispatch_window_ticks)
                },
                assigned_ships: reservations
                    .lane_ship_counts
                    .get(&(source_station, demand.station_id, demand.commodity))
                    .copied()
                    .unwrap_or(0),
                lane_ship_cap,
            });
            if let Some(remaining) = supply_remaining.get_mut(&(source_station, demand.commodity)) {
                *remaining = (*remaining - total_amount).max(0.0);
            }
            next_order_id = next_order_id.saturating_add(1);
        }
        orders.sort_by(|left, right| {
            right
                .is_critical
                .cmp(&left.is_critical)
                .then_with(|| right.urgency_score.total_cmp(&left.urgency_score))
                .then_with(|| left.order_id.cmp(&right.order_id))
        });

        (demands, supplies, orders, reservations)
    }

    fn current_reservations(&self) -> ReservationMaps {
        let mut maps = ReservationMaps::default();
        for order in self.trade_orders.values() {
            let outbound_reservation = if order.stage == TradeOrderStage::ToPickup {
                order.amount
            } else {
                0.0
            };
            if outbound_reservation > 1e-9 {
                *maps
                    .source_reserved
                    .entry((order.source_station, order.commodity))
                    .or_insert(0.0) += outbound_reservation;
            }

            let inbound_reservation = if order.purchased_amount > 1e-9 {
                order.purchased_amount
            } else {
                order.amount
            };
            if inbound_reservation > 1e-9 {
                *maps
                    .demand_reserved
                    .entry((order.destination_station, order.commodity))
                    .or_insert(0.0) += inbound_reservation;
                *maps
                    .lane_reserved
                    .entry((
                        order.source_station,
                        order.destination_station,
                        order.commodity,
                    ))
                    .or_insert(0.0) += inbound_reservation;
                *maps
                    .lane_ship_counts
                    .entry((
                        order.source_station,
                        order.destination_station,
                        order.commodity,
                    ))
                    .or_insert(0) += 1;
                maps.reservations.push(OrderReservation {
                    order_id: None,
                    ship_id: Some(order.ship_id),
                    source_station: order.source_station,
                    destination_station: order.destination_station,
                    commodity: order.commodity,
                    amount: inbound_reservation,
                });
            }
        }
        maps
    }

    fn best_source_for_demand(
        &self,
        demand: &FreightDemand,
        supply_remaining: &BTreeMap<(StationId, Commodity), f64>,
        avg_capacity: f64,
    ) -> Option<(StationId, RoutePlan)> {
        let demand_state = self
            .markets
            .get(&demand.station_id)
            .and_then(|book| book.goods.get(&demand.commodity))?;
        supply_remaining
            .iter()
            .filter(|((station_id, commodity), amount)| {
                *commodity == demand.commodity
                    && *station_id != demand.station_id
                    && **amount > 1e-9
            })
            .filter_map(|((station_id, _), amount)| {
                let route = self.build_npc_planner_route(
                    *station_id,
                    demand.station_id,
                    PriorityMode::Hybrid,
                    18.0,
                    false,
                )?;
                let source_price = self
                    .markets
                    .get(station_id)
                    .and_then(|book| book.goods.get(&demand.commodity))
                    .map(|state| state.price)
                    .unwrap_or(0.0);
                let price_spread = (demand_state.price - source_price).max(0.0);
                let score = demand.urgency_score * 20.0
                    + price_spread * 0.6
                    + (*amount / avg_capacity).min(3.0) * 4.0
                    - route.eta_ticks as f64 * 0.22
                    - route.risk_score * 5.0;
                Some((score, *station_id, route))
            })
            .max_by(|left, right| {
                left.0
                    .total_cmp(&right.0)
                    .then_with(|| left.1 .0.cmp(&right.1 .0))
            })
            .map(|(_, station_id, route)| (station_id, route))
    }

    fn all_bids_for_idle_ships(
        &self,
        idle_ships: Vec<ShipId>,
        orders: &[FreightOrder],
        reservations: &ReservationMaps,
    ) -> Vec<PlannedShipBid> {
        idle_ships
            .into_iter()
            .flat_map(|ship_id| {
                orders
                    .iter()
                    .enumerate()
                    .filter_map(|(order_idx, order)| {
                        self.build_ship_bid(ship_id, order_idx, order, reservations)
                    })
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    fn best_bid_for_ship(
        &self,
        ship_id: ShipId,
        orders: &[FreightOrder],
        reservations: &ReservationMaps,
    ) -> Option<PlannedShipBid> {
        orders
            .iter()
            .enumerate()
            .filter_map(|(order_idx, order)| {
                self.build_ship_bid(ship_id, order_idx, order, reservations)
            })
            .max_by(|left, right| {
                left.bid.score.total_cmp(&right.bid.score).then_with(|| {
                    left.plan
                        .destination_station
                        .0
                        .cmp(&right.plan.destination_station.0)
                })
            })
    }

    fn build_ship_bid(
        &self,
        ship_id: ShipId,
        order_idx: usize,
        order: &FreightOrder,
        reservations: &ReservationMaps,
    ) -> Option<PlannedShipBid> {
        if order.remaining_amount <= 1e-9
            || order.assigned_ships >= order.lane_ship_cap
            || order.dispatch_after_tick > self.tick
        {
            return None;
        }

        let ship = self.ships.get(&ship_id)?.clone();
        let ship_station = ship
            .current_station
            .or_else(|| self.world.first_station(ship.location))?;
        let amount = order.remaining_amount.min(ship.cargo_capacity);
        if amount <= 1e-9 {
            return None;
        }

        let load_factor = if ship.cargo_capacity <= 0.0 {
            0.0
        } else {
            amount / ship.cargo_capacity
        };
        if !order.is_critical && load_factor + 1e-9 < self.planner_settings.minimum_load_factor {
            return None;
        }

        let use_legacy_route = self.planner_mode == PlannerMode::GreedyCurrent;
        let route_to_source = self.build_npc_planner_route(
            ship_station,
            order.source_station,
            PriorityMode::Hybrid,
            ship.sub_light_speed,
            use_legacy_route,
        )?;
        let route_to_destination = self.build_npc_planner_route(
            order.source_station,
            order.destination_station,
            PriorityMode::Hybrid,
            ship.sub_light_speed,
            use_legacy_route,
        )?;
        let reposition_eta_ticks = route_to_source.eta_ticks;
        let delivery_eta_ticks = route_to_destination.eta_ticks;

        let source_price = self
            .markets
            .get(&order.source_station)
            .and_then(|book| book.goods.get(&order.commodity))
            .map(|state| state.price)?;
        let destination_price = self
            .markets
            .get(&order.destination_station)
            .and_then(|book| book.goods.get(&order.commodity))
            .map(|state| state.price)?;
        let expected_profit = self.trade_sale_net(amount, destination_price)
            - self.trade_purchase_total(amount, source_price)
            - self.estimated_gate_fees(&route_to_source)
            - self.estimated_gate_fees(&route_to_destination);
        let lane_key = (
            order.source_station,
            order.destination_station,
            order.commodity,
        );
        let saturation = reservations
            .lane_ship_counts
            .get(&lane_key)
            .copied()
            .unwrap_or(order.assigned_ships);
        let company_balance = self
            .npc_company_runtimes
            .get(&ship.company_id)
            .map(|runtime| runtime.balance)
            .unwrap_or(0.0);
        let balance_pressure = if company_balance < 0.0 { 15.0 } else { 0.0 };
        let score = expected_profit
            + order.urgency_score * 18.0
            + if order.is_critical { 250.0 } else { 0.0 }
            + load_factor * 12.0
            - route_to_source.eta_ticks as f64 * 0.30
            - route_to_destination.eta_ticks as f64 * 0.18
            - (route_to_source.risk_score + route_to_destination.risk_score) * 8.0
            - saturation as f64 * 18.0
            - balance_pressure;

        Some(PlannedShipBid {
            order_idx,
            plan: PlannedCompanyOrder {
                ship_id,
                commodity: order.commodity,
                amount,
                expected_profit,
                source_station: order.source_station,
                destination_station: order.destination_station,
                route_to_source,
                route_to_destination,
            },
            bid: ShipBid {
                ship_id,
                order_id: order.order_id,
                amount,
                score,
                reposition_eta_ticks,
                delivery_eta_ticks,
                load_factor,
            },
        })
    }

    fn build_npc_planner_route(
        &self,
        origin_station: StationId,
        destination_station: StationId,
        priority_mode: PriorityMode,
        sub_light_speed: f64,
        legacy: bool,
    ) -> Option<RoutePlan> {
        let policy = AutopilotPolicy {
            max_hops: super::stage_a_route_hop_limit(&self.world),
            priority_mode,
            max_risk_score: 8.0,
            ..AutopilotPolicy::default()
        };
        if legacy {
            self.build_station_route_with_speed_legacy(
                origin_station,
                destination_station,
                policy,
                sub_light_speed,
            )
        } else {
            self.build_station_route_with_speed(
                origin_station,
                destination_station,
                policy,
                sub_light_speed,
            )
        }
    }

    fn average_npc_cargo_capacity(&self) -> f64 {
        let ships = self
            .ships
            .values()
            .filter(|ship| ship.role == ShipRole::NpcTrade)
            .collect::<Vec<_>>();
        if ships.is_empty() {
            18.0
        } else {
            ships.iter().map(|ship| ship.cargo_capacity).sum::<f64>() / ships.len() as f64
        }
    }

    fn apply_bid_to_order_book(
        orders: &mut [FreightOrder],
        demands: &mut [FreightDemand],
        supplies: &mut [FreightSupply],
        reservations: &mut ReservationMaps,
        planned: &PlannedShipBid,
    ) {
        let Some(order) = orders.get_mut(planned.order_idx) else {
            return;
        };
        order.remaining_amount = (order.remaining_amount - planned.plan.amount).max(0.0);
        order.reserved_amount += planned.plan.amount;
        order.assigned_ships += 1;

        if let Some(demand) = demands.iter_mut().find(|demand| {
            demand.station_id == order.destination_station && demand.commodity == order.commodity
        }) {
            demand.reserved_amount += planned.plan.amount;
        }
        if let Some(supply) = supplies.iter_mut().find(|supply| {
            supply.station_id == order.source_station && supply.commodity == order.commodity
        }) {
            supply.reserved_amount += planned.plan.amount;
            supply.available_amount = (supply.available_amount - planned.plan.amount).max(0.0);
        }

        *reservations
            .lane_ship_counts
            .entry((
                order.source_station,
                order.destination_station,
                order.commodity,
            ))
            .or_insert(0) += 1;
        *reservations
            .lane_reserved
            .entry((
                order.source_station,
                order.destination_station,
                order.commodity,
            ))
            .or_insert(0.0) += planned.plan.amount;
        reservations.reservations.push(OrderReservation {
            order_id: Some(order.order_id),
            ship_id: Some(planned.plan.ship_id),
            source_station: order.source_station,
            destination_station: order.destination_station,
            commodity: order.commodity,
            amount: planned.plan.amount,
        });
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
            ship.cargo = CargoManifest::default();
            ship.policy.max_hops = super::stage_a_route_hop_limit(&self.world);
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
            ship.cargo = CargoManifest::default();
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
                        ship.cargo = CargoManifest::from(CargoLoad {
                            commodity: order.commodity,
                            amount,
                            source: CargoSource::Spot,
                        });
                    }

                    let Some(route) = self.build_npc_planner_route(
                        order.source_station,
                        order.destination_station,
                        PriorityMode::Hybrid,
                        ship_snapshot.sub_light_speed,
                        self.planner_mode == PlannerMode::GreedyCurrent,
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
                let Some(route) = self.build_npc_planner_route(
                    from_station,
                    order.source_station,
                    PriorityMode::Hybrid,
                    ship_snapshot.sub_light_speed,
                    self.planner_mode == PlannerMode::GreedyCurrent,
                ) else {
                    self.clear_trade_order_for_ship(ship_id, order_id);
                    return false;
                };
                self.apply_npc_route(ship_id, &route);
                true
            }
            TradeOrderStage::ToDropoff => {
                if at_station == Some(order.destination_station) {
                    let delivered_amount = {
                        let spot_amount = ship_snapshot.spot_amount(order.commodity);
                        if spot_amount > 0.0 {
                            spot_amount
                        } else {
                            order.purchased_amount
                        }
                    };
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
                    self.last_service_tick
                        .insert((order.destination_station, order.commodity), self.tick);
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
                let Some(route) = self.build_npc_planner_route(
                    from_station,
                    order.destination_station,
                    PriorityMode::Hybrid,
                    ship_snapshot.sub_light_speed,
                    self.planner_mode == PlannerMode::GreedyCurrent,
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
