use std::collections::BTreeMap;

use gatebound_domain::*;

use super::state::Simulation;

const MISSION_EPSILON: f64 = 1e-9;
const MISSION_SURPLUS_RATIO: f64 = 1.15;
const MISSION_DEFICIT_RATIO: f64 = 0.85;
const MISSION_LOAD_CAP: f64 = 18.0;
const MISSION_FRACTION_OF_GAP: f64 = 0.55;

fn round_decimal(value: f64, digits: i32) -> f64 {
    let scale = 10_f64.powi(digits);
    let rounded = (value * scale).round() / scale;
    if rounded == -0.0 {
        0.0
    } else {
        rounded
    }
}

fn stable_quantity(value: f64) -> f64 {
    round_decimal(value, 3)
}

fn stable_ratio(value: f64) -> f64 {
    round_decimal(value, 4)
}

fn stable_money(value: f64) -> f64 {
    round_decimal(value, 2)
}

impl Simulation {
    pub fn refresh_mission_offers(&mut self) {
        let mut offers = BTreeMap::new();
        let stations = self
            .world
            .stations
            .iter()
            .map(|station| (station.system_id, station.id))
            .collect::<Vec<_>>();

        for (origin_system, origin_station) in &stations {
            for (destination_system, destination_station) in &stations {
                if origin_station == destination_station {
                    continue;
                }

                let Some(route) = self.build_station_route_with_speed(
                    *origin_station,
                    *destination_station,
                    AutopilotPolicy {
                        max_hops: super::stage_a_route_hop_limit(&self.world),
                        ..AutopilotPolicy::default()
                    },
                    MISSION_LOAD_CAP,
                ) else {
                    continue;
                };
                let route_gate_ids = route
                    .segments
                    .iter()
                    .filter(|segment| segment.kind == SegmentKind::Warp)
                    .filter_map(|segment| segment.edge)
                    .collect::<Vec<_>>();

                for commodity in Commodity::ALL {
                    let Some(origin_state) = self
                        .markets
                        .get(origin_station)
                        .and_then(|book| book.goods.get(&commodity))
                    else {
                        continue;
                    };
                    let Some(destination_state) = self
                        .markets
                        .get(destination_station)
                        .and_then(|book| book.goods.get(&commodity))
                    else {
                        continue;
                    };

                    let surplus = (origin_state.stock
                        - origin_state.target_stock * MISSION_SURPLUS_RATIO)
                        .max(0.0);
                    let deficit = (destination_state.target_stock * MISSION_DEFICIT_RATIO
                        - destination_state.stock)
                        .max(0.0);
                    if surplus <= MISSION_EPSILON || deficit <= MISSION_EPSILON {
                        continue;
                    }

                    let quantity = stable_quantity(
                        deficit
                            .min(surplus)
                            .min(origin_state.stock.max(0.0))
                            .min(MISSION_LOAD_CAP)
                            .min((deficit * MISSION_FRACTION_OF_GAP).max(3.0)),
                    );
                    if quantity <= MISSION_EPSILON {
                        continue;
                    }

                    let urgency = deficit / destination_state.target_stock.max(1.0);
                    let price_spread = (destination_state.price - origin_state.price).max(0.0);
                    let score = stable_money(
                        quantity * (1.0 + urgency * 2.0 + price_spread * 0.35)
                            - route.eta_ticks as f64 * 0.08
                            - route.risk_score * 4.5,
                    );
                    if score <= 0.0 {
                        continue;
                    }

                    let reward = stable_money(
                        10.0 + quantity * 1.9
                            + route.eta_ticks as f64 * 0.18
                            + route.risk_score * 5.0,
                    );
                    let penalty = stable_money(quantity * origin_state.price * 5.0);
                    let offer = MissionOffer {
                        id: self.next_mission_offer_id,
                        kind: MissionKind::Transport,
                        commodity,
                        origin: *origin_system,
                        destination: *destination_system,
                        origin_station: *origin_station,
                        destination_station: *destination_station,
                        quantity,
                        reward,
                        penalty,
                        eta_ticks: route.eta_ticks,
                        risk_score: stable_ratio(route.risk_score),
                        score,
                        route_gate_ids: route_gate_ids.clone(),
                        expires_cycle: self.cycle.saturating_add(u64::from(
                            self.config.pressure.offer_ttl_cycles.max(1),
                        )),
                    };
                    offers.insert(self.next_mission_offer_id, offer);
                    self.next_mission_offer_id = self.next_mission_offer_id.saturating_add(1);
                }
            }
        }

        self.mission_offers = offers;
    }

    pub fn accept_mission_offer(&mut self, offer_id: u64) -> Result<MissionId, MissionOfferError> {
        let Some(offer) = self.mission_offers.get(&offer_id).cloned() else {
            return Err(MissionOfferError::UnknownOffer);
        };
        if offer.expires_cycle < self.cycle {
            self.mission_offers.remove(&offer_id);
            return Err(MissionOfferError::ExpiredOffer);
        }

        let (available, current_price) = self
            .markets
            .get(&offer.origin_station)
            .and_then(|book| book.goods.get(&offer.commodity))
            .map(|state| (state.stock, state.price))
            .unwrap_or((0.0, 0.0));
        if available + MISSION_EPSILON < offer.quantity {
            self.mission_offers.remove(&offer_id);
            return Err(MissionOfferError::InsufficientStock);
        }

        if let Some(state) = self
            .markets
            .get_mut(&offer.origin_station)
            .and_then(|book| book.goods.get_mut(&offer.commodity))
        {
            state.stock = (state.stock - offer.quantity).max(0.0);
            state.cycle_outflow += offer.quantity;
        }
        let penalty = stable_money(offer.quantity * current_price * 5.0);

        let mission_id = MissionId(
            self.missions
                .keys()
                .map(|id| id.0)
                .max()
                .unwrap_or(0)
                .saturating_add(1),
        );
        *self
            .player_station_storage
            .entry(offer.origin_station)
            .or_default()
            .entry(offer.commodity)
            .or_insert(0.0) += offer.quantity;
        self.missions.insert(
            mission_id,
            Mission {
                id: mission_id,
                kind: offer.kind,
                status: MissionStatus::Accepted,
                commodity: offer.commodity,
                origin: offer.origin,
                destination: offer.destination,
                origin_station: offer.origin_station,
                destination_station: offer.destination_station,
                quantity: offer.quantity,
                reward: offer.reward,
                penalty,
                eta_ticks: offer.eta_ticks,
                risk_score: offer.risk_score,
                route_gate_ids: offer.route_gate_ids,
                accepted_tick: self.tick,
                accepted_cycle: self.cycle,
                loaded_amount: 0.0,
                delivered_amount: 0.0,
            },
        );
        self.mission_offers.remove(&offer_id);
        Ok(mission_id)
    }

    pub fn cancel_mission(&mut self, mission_id: MissionId) -> Result<(), MissionActionError> {
        let Some(mission) = self.missions.get(&mission_id).cloned() else {
            return Err(MissionActionError::UnknownMission);
        };
        if matches!(
            mission.status,
            MissionStatus::Completed | MissionStatus::Cancelled
        ) {
            return Err(MissionActionError::MissionState);
        }

        if let Some(existing) = self.missions.get_mut(&mission_id) {
            existing.status = MissionStatus::Cancelled;
        }
        self.capital -= mission.penalty;
        Ok(())
    }

    pub fn complete_mission(
        &mut self,
        ship_id: ShipId,
        mission_id: MissionId,
    ) -> Result<(), MissionActionError> {
        let Some(ship_snapshot) = self.ships.get(&ship_id).cloned() else {
            return Err(MissionActionError::UnknownShip);
        };
        if ship_snapshot.company_id != CompanyId(0) {
            return Err(MissionActionError::UnknownShip);
        }
        let Some(mission) = self.missions.get(&mission_id).cloned() else {
            return Err(MissionActionError::UnknownMission);
        };
        if matches!(
            mission.status,
            MissionStatus::Completed | MissionStatus::Cancelled
        ) {
            return Err(MissionActionError::MissionState);
        }
        if !self.is_ship_docked_at(ship_id, mission.destination_station) {
            return Err(ship_snapshot
                .current_station
                .map(|_| MissionActionError::WrongStation)
                .unwrap_or(MissionActionError::NotDocked));
        }

        let available = self
            .player_station_storage
            .get(&mission.destination_station)
            .and_then(|goods| goods.get(&mission.commodity))
            .copied()
            .unwrap_or(0.0);
        if available + MISSION_EPSILON < mission.quantity {
            return Err(MissionActionError::InsufficientStoredCargo);
        }

        let amount = self.take_station_storage(
            mission.destination_station,
            mission.commodity,
            mission.quantity,
        );
        if amount <= MISSION_EPSILON {
            return Err(MissionActionError::InsufficientStoredCargo);
        }
        if let Some(existing) = self.missions.get_mut(&mission_id) {
            existing.delivered_amount = amount.min(existing.quantity);
            existing.status = MissionStatus::Completed;
        }
        if let Some(state) = self
            .markets
            .get_mut(&mission.destination_station)
            .and_then(|book| book.goods.get_mut(&mission.commodity))
        {
            state.stock += amount;
            state.cycle_inflow += amount;
        }
        self.capital += mission.reward;

        Ok(())
    }

    pub(crate) fn expire_mission_offers(&mut self) {
        self.mission_offers
            .retain(|_, offer| offer.expires_cycle >= self.cycle);
    }

    fn take_station_storage(
        &mut self,
        station_id: StationId,
        commodity: Commodity,
        quantity: f64,
    ) -> f64 {
        let mut remove_station_entry = false;
        let amount = self
            .player_station_storage
            .get_mut(&station_id)
            .and_then(|goods| {
                let removed = goods.get_mut(&commodity).map(|stored| {
                    let removed = stored.min(quantity);
                    *stored = (*stored - removed).max(0.0);
                    removed
                })?;
                if goods
                    .get(&commodity)
                    .is_some_and(|stored| *stored <= MISSION_EPSILON)
                {
                    goods.remove(&commodity);
                }
                remove_station_entry = goods.is_empty();
                Some(removed)
            })
            .unwrap_or(0.0);
        if remove_station_entry {
            self.player_station_storage.remove(&station_id);
        }
        amount
    }
}
