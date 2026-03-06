use std::collections::BTreeMap;

use gatebound_domain::*;

use super::state::Simulation;

impl Simulation {
    pub fn player_contract_load(
        &mut self,
        ship_id: ShipId,
        contract_id: ContractId,
        quantity: f64,
    ) -> Result<(), ContractActionError> {
        if quantity <= 0.0 {
            return Err(ContractActionError::InvalidQuantity);
        }
        let Some(ship_snapshot) = self.ships.get(&ship_id).cloned() else {
            return Err(ContractActionError::UnknownShip);
        };
        if ship_snapshot.company_id != CompanyId(0) {
            return Err(ContractActionError::InvalidAssignment);
        }
        let Some(contract_snapshot) = self.contracts.get(&contract_id).cloned() else {
            return Err(ContractActionError::UnknownContract);
        };
        if contract_snapshot.assigned_ship != Some(ship_id) {
            return Err(ContractActionError::NotAssignedShip);
        }
        if contract_snapshot.completed || contract_snapshot.failed {
            return Err(ContractActionError::ContractState);
        }
        if contract_snapshot.progress != ContractProgress::AwaitPickup {
            return Err(ContractActionError::ContractState);
        }
        if !self.is_ship_docked_at(ship_id, contract_snapshot.origin_station) {
            return Err(ContractActionError::NotDocked);
        }

        let Some(available) = self
            .markets
            .get(&contract_snapshot.origin_station)
            .and_then(|book| book.goods.get(&contract_snapshot.commodity))
            .map(|state| state.stock)
        else {
            return Err(ContractActionError::InsufficientStock);
        };

        if let Some(cargo) = ship_snapshot.cargo {
            if cargo.commodity != contract_snapshot.commodity {
                return Err(ContractActionError::CommodityMismatch);
            }
            if cargo.source
                != (CargoSource::Contract {
                    contract_id: contract_snapshot.id,
                })
            {
                return Err(ContractActionError::ContractState);
            }
            if cargo.amount + quantity > ship_snapshot.cargo_capacity + 1e-9 {
                return Err(ContractActionError::CargoCapacityExceeded);
            }
        } else if quantity > ship_snapshot.cargo_capacity + 1e-9 {
            return Err(ContractActionError::CargoCapacityExceeded);
        }

        let remaining_target = if contract_snapshot.kind == ContractTypeStageA::Delivery {
            (contract_snapshot.quantity
                - contract_snapshot.delivered_amount
                - contract_snapshot.loaded_amount)
                .max(0.0)
        } else {
            (contract_snapshot.per_cycle
                - contract_snapshot.delivered_cycle_amount
                - contract_snapshot.loaded_amount)
                .max(0.0)
        };
        if remaining_target <= 1e-9 {
            return Err(ContractActionError::ContractState);
        }
        let amount = quantity.min(remaining_target).min(available);
        if amount <= 1e-9 {
            return Err(ContractActionError::InsufficientStock);
        }

        if let Some(state) = self
            .markets
            .get_mut(&contract_snapshot.origin_station)
            .and_then(|book| book.goods.get_mut(&contract_snapshot.commodity))
        {
            state.stock = (state.stock - amount).max(0.0);
            state.cycle_outflow += amount;
        }
        if let Some(ship) = self.ships.get_mut(&ship_id) {
            match &mut ship.cargo {
                Some(cargo) => cargo.amount += amount,
                None => {
                    ship.cargo = Some(CargoLoad {
                        commodity: contract_snapshot.commodity,
                        amount,
                        source: CargoSource::Contract {
                            contract_id: contract_snapshot.id,
                        },
                    });
                }
            }
        }
        if let Some(contract) = self.contracts.get_mut(&contract_snapshot.id) {
            contract.loaded_amount += amount;
            if contract.loaded_amount > 0.0 {
                contract.progress = ContractProgress::InTransit;
            }
        }
        Ok(())
    }

    pub fn player_contract_unload(
        &mut self,
        ship_id: ShipId,
        contract_id: ContractId,
        quantity: f64,
    ) -> Result<(), ContractActionError> {
        if quantity <= 0.0 {
            return Err(ContractActionError::InvalidQuantity);
        }
        let Some(ship_snapshot) = self.ships.get(&ship_id).cloned() else {
            return Err(ContractActionError::UnknownShip);
        };
        if ship_snapshot.company_id != CompanyId(0) {
            return Err(ContractActionError::InvalidAssignment);
        }
        let Some(contract_snapshot) = self.contracts.get(&contract_id).cloned() else {
            return Err(ContractActionError::UnknownContract);
        };
        if contract_snapshot.assigned_ship != Some(ship_id) {
            return Err(ContractActionError::NotAssignedShip);
        }
        if contract_snapshot.completed || contract_snapshot.failed {
            return Err(ContractActionError::ContractState);
        }
        if contract_snapshot.progress != ContractProgress::InTransit {
            return Err(ContractActionError::ContractState);
        }
        if !self.is_ship_docked_at(ship_id, contract_snapshot.destination_station) {
            return Err(ContractActionError::NotDocked);
        }

        let Some(cargo) = ship_snapshot.cargo else {
            return Err(ContractActionError::InsufficientCargo);
        };
        if cargo.commodity != contract_snapshot.commodity {
            return Err(ContractActionError::CommodityMismatch);
        }
        if cargo.source
            != (CargoSource::Contract {
                contract_id: contract_snapshot.id,
            })
        {
            return Err(ContractActionError::ContractState);
        }
        if cargo.amount <= 1e-9 || contract_snapshot.loaded_amount <= 1e-9 {
            return Err(ContractActionError::InsufficientCargo);
        }

        let amount = quantity
            .min(cargo.amount)
            .min(contract_snapshot.loaded_amount);
        if amount <= 1e-9 {
            return Err(ContractActionError::InsufficientCargo);
        }

        if let Some(state) = self
            .markets
            .get_mut(&contract_snapshot.destination_station)
            .and_then(|book| book.goods.get_mut(&contract_snapshot.commodity))
        {
            state.stock += amount;
            state.cycle_inflow += amount;
        }
        if let Some(ship) = self.ships.get_mut(&ship_id) {
            if let Some(ship_cargo) = &mut ship.cargo {
                ship_cargo.amount = (ship_cargo.amount - amount).max(0.0);
                if ship_cargo.amount <= 1e-9 {
                    ship.cargo = None;
                }
            }
        }

        let mut delivery_completed = false;
        if let Some(contract) = self.contracts.get_mut(&contract_snapshot.id) {
            contract.loaded_amount = (contract.loaded_amount - amount).max(0.0);
            contract.delivered_amount += amount;
            if contract.kind == ContractTypeStageA::Supply {
                contract.delivered_cycle_amount += amount;
            }
            if contract.kind == ContractTypeStageA::Delivery
                && contract.delivered_amount + 1e-9 >= contract.quantity
            {
                contract.delivered_amount = contract.quantity;
                contract.completed = true;
                contract.failed = false;
                contract.progress = ContractProgress::Completed;
                delivery_completed = true;
            } else if contract.loaded_amount <= 1e-9 {
                contract.progress = ContractProgress::AwaitPickup;
            } else {
                contract.progress = ContractProgress::InTransit;
            }
        }

        if delivery_completed {
            if let Some(ship) = self.ships.get_mut(&ship_id) {
                ship.active_contract = None;
            }
            let net_payout = self.apply_market_fee(contract_snapshot.payout);
            self.capital += net_payout;
            self.record_ship_profit(ship_id, net_payout);
            self.sla_successes = self.sla_successes.saturating_add(1);
        }

        Ok(())
    }

    pub fn create_supply_contract(
        &mut self,
        origin: SystemId,
        destination: SystemId,
        per_cycle: f64,
        total_cycles: u32,
    ) -> ContractId {
        self.create_supply_contract_for_commodity(
            origin,
            destination,
            Commodity::Fuel,
            per_cycle,
            total_cycles,
        )
    }

    pub fn create_supply_contract_for_commodity(
        &mut self,
        origin: SystemId,
        destination: SystemId,
        commodity: Commodity,
        per_cycle: f64,
        total_cycles: u32,
    ) -> ContractId {
        let next_id = ContractId(self.contracts.len());
        let origin_station = self.world.first_station(origin).unwrap_or(StationId(0));
        let destination_station = self
            .world
            .first_station(destination)
            .unwrap_or(origin_station);
        self.contracts.insert(
            next_id,
            Contract {
                id: next_id,
                kind: ContractTypeStageA::Supply,
                progress: ContractProgress::AwaitPickup,
                commodity,
                origin,
                destination,
                origin_station,
                destination_station,
                quantity: per_cycle,
                deadline_tick: 0,
                per_cycle,
                total_cycles,
                payout: 20.0,
                penalty: 10.0,
                assigned_ship: None,
                loaded_amount: 0.0,
                delivered_cycle_amount: 0.0,
                delivered_amount: 0.0,
                missed_cycles: 0,
                completed: false,
                failed: false,
                last_eval_cycle: self.cycle,
            },
        );
        next_id
    }

    pub fn refresh_contract_offers(&mut self) {
        let mut offers = BTreeMap::new();
        let system_ids: Vec<SystemId> = self.world.systems.iter().map(|system| system.id).collect();

        for window in system_ids.windows(2) {
            let origin = window[0];
            let destination = window[1];
            self.maybe_push_offer(
                origin,
                destination,
                ContractTypeStageA::Delivery,
                &mut offers,
            );
            self.maybe_push_offer(destination, origin, ContractTypeStageA::Supply, &mut offers);
        }

        self.contract_offers = offers;
    }

    pub fn accept_contract_offer(
        &mut self,
        offer_id: u64,
        ship_id: ShipId,
    ) -> Result<ContractId, OfferError> {
        let Some(offer) = self.contract_offers.get(&offer_id).cloned() else {
            return Err(OfferError::UnknownOffer);
        };
        if offer.expires_cycle < self.cycle {
            self.contract_offers.remove(&offer_id);
            return Err(OfferError::ExpiredOffer);
        }
        let Some(ship_snapshot) = self.ships.get(&ship_id).cloned() else {
            return Err(OfferError::InvalidAssignment);
        };
        if ship_snapshot.company_id != CompanyId(0) {
            return Err(OfferError::InvalidAssignment);
        }
        if ship_snapshot.active_contract.is_some() || ship_snapshot.eta_ticks_remaining > 0 {
            return Err(OfferError::ShipBusy);
        }

        let contract_id = ContractId(
            self.contracts
                .keys()
                .map(|id| id.0)
                .max()
                .unwrap_or(0)
                .saturating_add(1),
        );
        let is_supply = offer.kind == ContractTypeStageA::Supply;
        let cycle_ticks = u64::from(self.config.time.cycle_ticks.max(1));
        self.contracts.insert(
            contract_id,
            Contract {
                id: contract_id,
                kind: offer.kind,
                progress: ContractProgress::AwaitPickup,
                commodity: offer.commodity,
                origin: offer.origin,
                destination: offer.destination,
                origin_station: offer.origin_station,
                destination_station: offer.destination_station,
                quantity: offer.quantity,
                deadline_tick: self
                    .tick
                    .saturating_add(u64::from(offer.eta_ticks).saturating_add(cycle_ticks * 3)),
                per_cycle: if is_supply { offer.quantity } else { 0.0 },
                total_cycles: if is_supply { 6 } else { 0 },
                payout: offer.payout,
                penalty: offer.penalty,
                assigned_ship: Some(ship_id),
                loaded_amount: 0.0,
                delivered_cycle_amount: 0.0,
                delivered_amount: 0.0,
                missed_cycles: 0,
                completed: false,
                failed: false,
                last_eval_cycle: self.cycle,
            },
        );
        if let Some(ship) = self.ships.get_mut(&ship_id) {
            ship.active_contract = Some(contract_id);
            ship.route_cursor = 0;
        }

        self.contract_offers.remove(&offer_id);
        Ok(contract_id)
    }

    pub(in crate::simulation) fn update_contracts_tick(&mut self) {
        let contract_ids: Vec<ContractId> = self.contracts.keys().copied().collect();
        for cid in contract_ids {
            let Some(snapshot) = self.contracts.get(&cid).cloned() else {
                continue;
            };
            if snapshot.completed || snapshot.failed {
                continue;
            }

            if snapshot.kind == ContractTypeStageA::Delivery {
                let Some(ship_id) = snapshot.assigned_ship else {
                    continue;
                };
                if self.tick > snapshot.deadline_tick {
                    let penalty_mult = self.penalty_multiplier(snapshot.missed_cycles as usize);
                    self.capital -= snapshot.penalty * penalty_mult;
                    if let Some(c) = self.contracts.get_mut(&cid) {
                        c.failed = true;
                        c.missed_cycles = c.missed_cycles.saturating_add(1);
                        c.progress = ContractProgress::Failed;
                        c.loaded_amount = 0.0;
                    }
                    if let Some(ship) = self.ships.get_mut(&ship_id) {
                        ship.active_contract = None;
                        if let Some(cargo) = ship.cargo {
                            if cargo.source == (CargoSource::Contract { contract_id: cid }) {
                                ship.cargo = None;
                            }
                        }
                    }
                    self.sla_failures = self.sla_failures.saturating_add(1);
                }
            }
        }
    }

    pub(in crate::simulation) fn evaluate_supply_contracts(&mut self) {
        let ids: Vec<ContractId> = self.contracts.keys().copied().collect();
        for cid in ids {
            let Some(current) = self.contracts.get(&cid).cloned() else {
                continue;
            };
            if current.kind != ContractTypeStageA::Supply || current.completed || current.failed {
                continue;
            }
            if current.last_eval_cycle == self.cycle {
                continue;
            }

            let delta = current
                .delivered_cycle_amount
                .min(self.config.pressure.market_depth_per_cycle);
            if delta >= current.per_cycle {
                let net_payout = self.apply_market_fee(current.payout);
                self.capital += net_payout;
                if let Some(ship_id) = current.assigned_ship {
                    self.record_ship_profit(ship_id, net_payout);
                }
                self.sla_successes = self.sla_successes.saturating_add(1);
            } else {
                let miss_index = current.missed_cycles as usize;
                let penalty_mult = self.penalty_multiplier(miss_index);
                self.capital -= current.penalty * penalty_mult;
                self.sla_failures = self.sla_failures.saturating_add(1);
                if let Some(contract) = self.contracts.get_mut(&cid) {
                    contract.missed_cycles = contract.missed_cycles.saturating_add(1);
                }
            }

            if let Some(contract) = self.contracts.get_mut(&cid) {
                contract.delivered_cycle_amount = 0.0;
                contract.last_eval_cycle = self.cycle;
                if self.cycle >= u64::from(contract.total_cycles.max(1)) {
                    contract.completed = true;
                    contract.progress = ContractProgress::Completed;
                    if let Some(ship_id) = contract.assigned_ship {
                        if let Some(ship) = self.ships.get_mut(&ship_id) {
                            ship.active_contract = None;
                        }
                    }
                } else if contract.loaded_amount <= 1e-9 {
                    contract.progress = ContractProgress::AwaitPickup;
                } else {
                    contract.progress = ContractProgress::InTransit;
                }
            }
        }
    }

    pub(in crate::simulation) fn apply_market_fee(&self, gross: f64) -> f64 {
        gross * (1.0 - self.config.pressure.market_fee_rate)
    }

    pub(in crate::simulation) fn expire_contract_offers(&mut self) {
        self.contract_offers
            .retain(|_, offer| offer.expires_cycle >= self.cycle);
    }

    pub(in crate::simulation) fn maybe_push_offer(
        &mut self,
        origin: SystemId,
        destination: SystemId,
        kind: ContractTypeStageA,
        offers: &mut BTreeMap<u64, ContractOffer>,
    ) {
        if origin == destination {
            return;
        }
        let origin_station = self.world.first_station(origin).unwrap_or(StationId(0));
        let destination_station = self
            .world
            .first_station(destination)
            .unwrap_or(origin_station);
        let Some(origin_market) = self.markets.get(&origin_station) else {
            return;
        };
        let Some(destination_market) = self.markets.get(&destination_station) else {
            return;
        };
        let commodity = Commodity::ALL
            .iter()
            .copied()
            .map(|item| {
                let deficit = destination_market
                    .goods
                    .get(&item)
                    .map(|state| (state.target_stock * 0.85 - state.stock).max(0.0))
                    .unwrap_or(0.0);
                let surplus = origin_market
                    .goods
                    .get(&item)
                    .map(|state| (state.stock - state.target_stock * 1.15).max(0.0))
                    .unwrap_or(0.0);
                (item, deficit.min(surplus))
            })
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .map(|entry| entry.0)
            .unwrap_or(Commodity::Fuel);
        let Some(route) = self.build_station_route_with_speed(
            origin_station,
            destination_station,
            AutopilotPolicy {
                max_hops: 16,
                ..AutopilotPolicy::default()
            },
            18.0,
        ) else {
            return;
        };
        let eta_ticks = route.eta_ticks;
        let risk_score = route.risk_score;
        let route_gate_ids = route
            .segments
            .iter()
            .filter(|segment| segment.kind == SegmentKind::Warp)
            .filter_map(|segment| segment.edge)
            .collect::<Vec<_>>();
        let imbalance = destination_market
            .goods
            .get(&commodity)
            .map(|state| {
                ((state.target_stock - state.stock) / state.target_stock.max(1.0)).max(0.0)
            })
            .unwrap_or(0.0);
        let flow_pressure = destination_market
            .goods
            .get(&commodity)
            .map(|state| (state.cycle_outflow - state.cycle_inflow).max(0.0))
            .unwrap_or(0.0);
        let origin_stock = origin_market
            .goods
            .get(&commodity)
            .map(|state| state.stock)
            .unwrap_or(0.0);

        let quantity = (8.0 + imbalance * 12.0 + flow_pressure * 0.8)
            .clamp(5.0, 30.0)
            .min(origin_stock.max(0.0));
        if quantity <= 0.0 {
            return;
        }
        let payout = 18.0 + quantity * 2.2 + eta_ticks as f64 * 0.3;
        let penalty = (payout * 0.45).max(8.0);
        let margin_estimate = payout
            - f64::from(eta_ticks) * 0.15
            - risk_score * 10.0
            - self.config.pressure.gate_fee_per_jump;
        let profit_per_ton = margin_estimate / quantity.max(1.0);
        let route_is_congested = route_gate_ids.iter().any(|gate_id| {
            let load = self.gate_queue_load.get(gate_id).copied().unwrap_or(0.0);
            let effective_capacity = self
                .world
                .edges
                .iter()
                .find(|edge| edge.id == *gate_id)
                .map(|edge| (edge.base_capacity * edge.capacity_factor).max(1.0))
                .unwrap_or(1.0);
            load / effective_capacity > 0.95
        });
        let fuel_ratio = destination_market
            .goods
            .get(&Commodity::Fuel)
            .map(|state| state.stock / state.target_stock.max(1.0))
            .unwrap_or(1.0);
        let problem_tag = if risk_score >= 1.0 {
            OfferProblemTag::HighRisk
        } else if route_is_congested {
            OfferProblemTag::CongestedRoute
        } else if fuel_ratio < 0.75 {
            OfferProblemTag::FuelVolatility
        } else {
            OfferProblemTag::LowMargin
        };
        let premium = self.reputation >= self.config.pressure.premium_offer_reputation_min;
        let offer = ContractOffer {
            id: self.next_offer_id,
            kind,
            commodity,
            origin,
            destination,
            origin_station,
            destination_station,
            quantity,
            payout,
            penalty,
            eta_ticks,
            risk_score,
            margin_estimate,
            route_gate_ids,
            problem_tag,
            premium,
            profit_per_ton,
            expires_cycle: self
                .cycle
                .saturating_add(u64::from(self.config.pressure.offer_ttl_cycles.max(1))),
        };
        offers.insert(self.next_offer_id, offer);
        self.next_offer_id = self.next_offer_id.saturating_add(1);
    }

    pub(in crate::simulation) fn penalty_multiplier(&self, misses: usize) -> f64 {
        let curve = &self.config.pressure.sla_penalty_curve;
        let idx = misses.min(curve.len().saturating_sub(1));
        curve[idx]
    }
}
