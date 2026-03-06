use std::collections::VecDeque;

use gatebound_domain::*;

use super::state::Simulation;

impl Simulation {
    pub(in crate::simulation) fn dispatch_npc_trade_orders(&mut self) {
        let mut idle_ships = self
            .ships
            .values()
            .filter(|ship| {
                ship.role == ShipRole::NpcTrade
                    && ship.active_contract.is_none()
                    && ship.trade_order_id.is_none()
                    && ship.segment_eta_remaining == 0
                    && ship.movement_queue.is_empty()
            })
            .map(|ship| ship.id)
            .collect::<Vec<_>>();
        idle_ships.sort_by_key(|ship_id| ship_id.0);
        if idle_ships.is_empty() {
            return;
        }
        let station_ids = self
            .world
            .stations
            .iter()
            .map(|station| station.id)
            .collect::<Vec<_>>();
        for ship_id in idle_ships {
            let Some(ship_snapshot) = self.ships.get(&ship_id).cloned() else {
                continue;
            };
            let ship_station = ship_snapshot
                .current_station
                .or_else(|| self.world.first_station(ship_snapshot.location));
            let Some(ship_station) = ship_station else {
                continue;
            };
            let mut best: Option<(StationId, StationId, Commodity, f64, RoutePlan, RoutePlan)> =
                None;
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
                        let surplus = self
                            .markets
                            .get(source_station)
                            .and_then(|book| book.goods.get(&commodity))
                            .map(|state| (state.stock - state.target_stock * 1.15).max(0.0))
                            .unwrap_or(0.0);
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
                        let score =
                            f64::from(route_to_source.eta_ticks + route_to_destination.eta_ticks);
                        if best.as_ref().is_none_or(|entry| score < entry.3) {
                            best = Some((
                                *source_station,
                                *destination_station,
                                commodity,
                                score,
                                route_to_source,
                                route_to_destination,
                            ));
                        }
                    }
                }
            }

            let Some((
                source_station,
                destination_station,
                commodity,
                _,
                route_to_source,
                route_to_destination,
            )) = best
            else {
                continue;
            };
            let amount = self
                .markets
                .get(&source_station)
                .and_then(|book| book.goods.get(&commodity))
                .map(|state| {
                    (state.stock - state.target_stock * 1.15)
                        .max(0.0)
                        .min(ship_snapshot.cargo_capacity)
                })
                .unwrap_or(0.0);
            if amount <= 0.0 {
                continue;
            }
            if let Some(state) = self
                .markets
                .get_mut(&source_station)
                .and_then(|book| book.goods.get_mut(&commodity))
            {
                state.stock = (state.stock - amount).max(0.0);
                state.cycle_outflow += amount;
            }
            let order_id = TradeOrderId(self.next_trade_order_id);
            self.next_trade_order_id = self.next_trade_order_id.saturating_add(1);
            let mut stage = TradeOrderStage::ToPickup;
            if let Some(ship) = self.ships.get_mut(&ship_id) {
                ship.trade_order_id = Some(order_id);
                ship.cargo = None;
                ship.policy.max_hops = 6;
                if ship.current_station == Some(source_station) {
                    stage = TradeOrderStage::ToDropoff;
                    ship.cargo = Some(CargoLoad {
                        commodity,
                        amount,
                        source: CargoSource::Spot,
                    });
                    ship.movement_queue = VecDeque::from(route_to_destination.segments.clone());
                } else {
                    ship.movement_queue = VecDeque::from(route_to_source.segments.clone());
                }
                ship.segment_eta_remaining = 0;
                ship.segment_progress_total = 0;
                ship.current_segment_kind = None;
                ship.current_target = None;
                ship.eta_ticks_remaining = 0;
            }
            self.trade_orders.insert(
                order_id,
                TradeOrder {
                    id: order_id,
                    ship_id,
                    commodity,
                    amount,
                    source_station,
                    destination_station,
                    stage,
                },
            );
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
        let ship_snapshot = self.ships.get(&ship_id).cloned();
        let Some(ship_snapshot) = ship_snapshot else {
            return false;
        };
        let at_station = ship_snapshot.current_station;
        match order.stage {
            TradeOrderStage::ToPickup => {
                if at_station == Some(order.source_station) {
                    if let Some(ship) = self.ships.get_mut(&ship_id) {
                        ship.cargo = Some(CargoLoad {
                            commodity: order.commodity,
                            amount: order.amount,
                            source: CargoSource::Spot,
                        });
                    }
                    if let Some(item) = self.trade_orders.get_mut(&order_id) {
                        item.stage = TradeOrderStage::ToDropoff;
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
                    if let Some(ship) = self.ships.get_mut(&ship_id) {
                        ship.movement_queue = VecDeque::from(route.segments);
                        ship.segment_eta_remaining = 0;
                        ship.segment_progress_total = 0;
                        ship.current_segment_kind = None;
                        ship.current_target = None;
                        ship.eta_ticks_remaining = 0;
                    }
                    return true;
                }
                let from_station = ship_snapshot
                    .current_station
                    .or_else(|| self.world.first_station(ship_snapshot.location));
                let Some(from_station) = from_station else {
                    return true;
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
                    return true;
                };
                if let Some(ship) = self.ships.get_mut(&ship_id) {
                    ship.movement_queue = VecDeque::from(route.segments);
                    ship.segment_eta_remaining = 0;
                    ship.segment_progress_total = 0;
                    ship.current_segment_kind = None;
                    ship.current_target = None;
                    ship.eta_ticks_remaining = 0;
                }
                true
            }
            TradeOrderStage::ToDropoff => {
                if at_station == Some(order.destination_station) {
                    if let Some(state) = self
                        .markets
                        .get_mut(&order.destination_station)
                        .and_then(|book| book.goods.get_mut(&order.commodity))
                    {
                        state.stock += order.amount;
                        state.cycle_inflow += order.amount;
                    }
                    self.trade_orders.remove(&order_id);
                    if let Some(ship) = self.ships.get_mut(&ship_id) {
                        ship.cargo = None;
                        ship.trade_order_id = None;
                    }
                    return true;
                }
                let from_station = ship_snapshot
                    .current_station
                    .or_else(|| self.world.first_station(ship_snapshot.location));
                let Some(from_station) = from_station else {
                    return true;
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
                    return true;
                };
                if let Some(ship) = self.ships.get_mut(&ship_id) {
                    ship.movement_queue = VecDeque::from(route.segments);
                    ship.segment_eta_remaining = 0;
                    ship.segment_progress_total = 0;
                    ship.current_segment_kind = None;
                    ship.current_target = None;
                    ship.eta_ticks_remaining = 0;
                }
                true
            }
        }
    }
}
