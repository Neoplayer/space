use super::*;

impl Simulation {
    pub fn apply_event(&mut self, event: RiskEvent) {
        match event {
            RiskEvent::GateCongestion {
                edge,
                capacity_factor,
                duration_ticks,
            } => {
                if let Some(found) = self.world.edges.iter_mut().find(|e| e.id == edge) {
                    found.capacity_factor = capacity_factor;
                }
                self.modifiers.push(ActiveModifier {
                    until_tick: self.tick + u64::from(duration_ticks),
                    gate: Some(edge),
                    risk: RiskStageA::GateCongestion,
                    magnitude: capacity_factor,
                });
            }
            RiskEvent::DockCongestion {
                delay_factor,
                duration_ticks,
            } => {
                self.modifiers.push(ActiveModifier {
                    until_tick: self.tick + u64::from(duration_ticks),
                    gate: None,
                    risk: RiskStageA::DockCongestion,
                    magnitude: delay_factor,
                });
            }
            RiskEvent::FuelShock {
                production_factor,
                duration_ticks,
            } => {
                self.modifiers.push(ActiveModifier {
                    until_tick: self.tick + u64::from(duration_ticks),
                    gate: None,
                    risk: RiskStageA::FuelShock,
                    magnitude: production_factor,
                });
            }
        }
    }

    pub fn inject_gate_congestion(
        &mut self,
        edge: GateId,
        capacity_factor: f64,
        duration_ticks: u32,
    ) {
        self.apply_event(RiskEvent::GateCongestion {
            edge,
            capacity_factor,
            duration_ticks,
        });
    }

    pub fn inject_dock_congestion(&mut self, delay_factor: f64, duration_ticks: u32) {
        self.apply_event(RiskEvent::DockCongestion {
            delay_factor,
            duration_ticks,
        });
    }

    pub fn inject_fuel_shock(&mut self, production_factor: f64, duration_ticks: u32) {
        self.apply_event(RiskEvent::FuelShock {
            production_factor,
            duration_ticks,
        });
    }

    pub(in crate::simulation) fn expire_modifiers(&mut self) {
        let tick = self.tick;
        let mut remaining = Vec::new();
        for modifier in self.modifiers.drain(..) {
            if tick < modifier.until_tick {
                remaining.push(modifier);
                continue;
            }

            if modifier.risk == RiskStageA::GateCongestion {
                if let Some(gate_id) = modifier.gate {
                    if let Some(edge) = self.world.edges.iter_mut().find(|e| e.id == gate_id) {
                        edge.capacity_factor = 1.0;
                    }
                }
            }
        }
        self.modifiers = remaining;
    }
}
