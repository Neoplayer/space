use gatebound_domain::*;

use super::state::Simulation;

impl Simulation {
    pub fn lease_slot(
        &mut self,
        system_id: SystemId,
        slot_type: SlotType,
        cycles: u32,
    ) -> Result<(), LeaseError> {
        if cycles == 0 {
            return Err(LeaseError::InvalidCycles);
        }
        if !self.world.systems.iter().any(|system| system.id == system_id) {
            return Err(LeaseError::UnknownSystem);
        }

        let total = self.total_slots_for(slot_type);
        let used = self
            .active_leases
            .iter()
            .filter(|lease| lease.system_id == system_id && lease.slot_type == slot_type)
            .count() as u32;
        if used >= total {
            return Err(LeaseError::NoCapacity);
        }

        let price_per_cycle = self.lease_price_for(system_id, slot_type);
        self.active_leases.push(LeasePosition {
            system_id,
            slot_type,
            cycles_remaining: cycles,
            price_per_cycle,
        });
        Ok(())
    }

    pub fn release_one_slot(&mut self, system_id: SystemId, slot_type: SlotType) -> bool {
        if let Some(idx) = self
            .active_leases
            .iter()
            .position(|lease| lease.system_id == system_id && lease.slot_type == slot_type)
        {
            self.active_leases.remove(idx);
            return true;
        }
        false
    }

    pub fn lease_market_for_system(&self, system_id: SystemId) -> Vec<LeaseMarketView> {
        if !self.world.systems.iter().any(|system| system.id == system_id) {
            return Vec::new();
        }

        SlotType::ALL
            .into_iter()
            .map(|slot_type| {
                let total = self.total_slots_for(slot_type);
                let used = self
                    .active_leases
                    .iter()
                    .filter(|lease| lease.system_id == system_id && lease.slot_type == slot_type)
                    .count() as u32;
                LeaseMarketView {
                    system_id,
                    slot_type,
                    available: total.saturating_sub(used),
                    total,
                    price_per_cycle: self.lease_price_for(system_id, slot_type),
                }
            })
            .collect()
    }

    pub(in crate::simulation) fn advance_lease_cycle(&mut self) {
        for lease in &mut self.active_leases {
            lease.cycles_remaining = lease.cycles_remaining.saturating_sub(1);
        }
        self.active_leases
            .retain(|lease| lease.cycles_remaining > 0);
    }

    pub(in crate::simulation) fn lease_price_for(
        &self,
        system_id: SystemId,
        slot_type: SlotType,
    ) -> f64 {
        let base = self.config.pressure.slot_lease_cost * slot_multiplier(slot_type);

        let throughput_signal = self
            .world
            .stations_by_system
            .get(&system_id)
            .map(|stations| {
                let mut total = 0.0;
                let mut count = 0.0;
                for station_id in stations {
                    if let Some(book) = self.markets.get(station_id) {
                        total += book
                            .goods
                            .values()
                            .map(|state| state.cycle_inflow + state.cycle_outflow)
                            .sum::<f64>();
                        count += book.goods.len() as f64;
                    }
                }
                if count <= 0.0 {
                    0.0
                } else {
                    total / count / 100.0
                }
            })
            .unwrap_or(0.0);

        let max_degree = self
            .world
            .adjacency
            .values()
            .map(Vec::len)
            .max()
            .unwrap_or(1) as f64;
        let degree = self
            .world
            .adjacency
            .get(&system_id)
            .map(Vec::len)
            .unwrap_or(0) as f64;
        let gate_proximity_signal = if max_degree <= 0.0 {
            0.0
        } else {
            degree / max_degree
        };

        let congestion_signal = self.system_congestion_signal(system_id);

        let price_mult_raw = 1.0
            + self.config.pressure.lease_price_throughput_k * throughput_signal
            + self.config.pressure.lease_price_gate_k * gate_proximity_signal
            + self.config.pressure.lease_price_congestion_k * congestion_signal;
        let price_mult = price_mult_raw.clamp(
            self.config.pressure.lease_price_min_mult,
            self.config.pressure.lease_price_max_mult,
        );

        base * price_mult
    }

    pub(in crate::simulation) fn total_slots_for(&self, slot_type: SlotType) -> u32 {
        match slot_type {
            SlotType::Dock => 4,
            SlotType::Storage => 6,
            SlotType::Factory => 3,
            SlotType::Market => 2,
        }
    }

    pub(in crate::simulation) fn system_congestion_signal(&self, system_id: SystemId) -> f64 {
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
}

pub(super) fn slot_multiplier(slot_type: SlotType) -> f64 {
    match slot_type {
        SlotType::Dock => 1.30,
        SlotType::Storage => 1.00,
        SlotType::Factory => 1.50,
        SlotType::Market => 1.20,
    }
}
