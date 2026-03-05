use bevy::prelude::*;
use gatebound_core::{CycleReport, RiskEvent, Simulation, TickReport};

use crate::hud::HudMessages;

#[derive(Resource, Debug, Clone)]
pub struct SimResource {
    pub simulation: Simulation,
    pub last_tick_report: TickReport,
    pub last_cycle_report: CycleReport,
}

impl SimResource {
    pub fn new(simulation: Simulation) -> Self {
        Self {
            last_tick_report: TickReport {
                tick: 0,
                cycle: 0,
                active_ships: simulation.ships.len(),
                active_contracts: simulation
                    .contracts
                    .values()
                    .filter(|contract| !contract.completed && !contract.failed)
                    .count(),
                total_queue_delay: 0,
                avg_price_index: 1.0,
            },
            last_cycle_report: CycleReport {
                cycle: 0,
                sla_success_rate: 1.0,
                reroute_count: 0,
                economy_stress_index: 0.0,
            },
            simulation,
        }
    }
}

#[derive(Resource, Debug, Clone, PartialEq)]
pub struct SimClock {
    pub paused: bool,
    pub speed_multiplier: u32,
    pub accumulator_seconds: f64,
}

impl Default for SimClock {
    fn default() -> Self {
        Self {
            paused: false,
            speed_multiplier: 1,
            accumulator_seconds: 0.0,
        }
    }
}

pub fn consume_ticks(clock: &mut SimClock, delta_seconds: f64, tick_seconds: u32) -> u32 {
    if clock.paused {
        return 0;
    }

    let tick_seconds = f64::from(tick_seconds.max(1));
    clock.accumulator_seconds += delta_seconds * f64::from(clock.speed_multiplier.max(1));

    let ticks = (clock.accumulator_seconds / tick_seconds).floor().max(0.0) as u32;
    clock.accumulator_seconds -= f64::from(ticks) * tick_seconds;
    ticks
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiskHotkey {
    GateCongestion,
    DockCongestion,
    FuelShock,
}

pub fn hotkey_to_risk(ch: char) -> Option<RiskHotkey> {
    match ch.to_ascii_lowercase() {
        'g' => Some(RiskHotkey::GateCongestion),
        'd' => Some(RiskHotkey::DockCongestion),
        'f' => Some(RiskHotkey::FuelShock),
        _ => None,
    }
}

pub fn apply_time_controls(keys: Res<ButtonInput<KeyCode>>, mut clock: ResMut<SimClock>) {
    if keys.just_pressed(KeyCode::Space) {
        clock.paused = !clock.paused;
    }

    if keys.just_pressed(KeyCode::Digit1) {
        clock.speed_multiplier = 1;
    }
    if keys.just_pressed(KeyCode::Digit2) {
        clock.speed_multiplier = 2;
    }
    if keys.just_pressed(KeyCode::Digit4) {
        clock.speed_multiplier = 4;
    }
}

pub fn handle_risk_hotkeys(
    keys: Res<ButtonInput<KeyCode>>,
    mut sim: ResMut<SimResource>,
    mut messages: ResMut<HudMessages>,
) {
    let cycle_ticks = sim.simulation.config.time.cycle_ticks;
    let action = if keys.just_pressed(KeyCode::KeyG) {
        hotkey_to_risk('g')
    } else if keys.just_pressed(KeyCode::KeyD) {
        hotkey_to_risk('d')
    } else if keys.just_pressed(KeyCode::KeyF) {
        hotkey_to_risk('f')
    } else {
        None
    };

    let Some(action) = action else {
        return;
    };

    match action {
        RiskHotkey::GateCongestion => {
            if let Some(edge) = sim.simulation.world.edges.first().copied() {
                sim.simulation.apply_event(RiskEvent::GateCongestion {
                    edge: edge.id,
                    capacity_factor: 0.5,
                    duration_ticks: cycle_ticks * 5,
                });
                messages.push(format!(
                    "Risk event: Gate congestion on edge {} (capacity x0.5)",
                    edge.id.0
                ));
            }
        }
        RiskHotkey::DockCongestion => {
            sim.simulation.apply_event(RiskEvent::DockCongestion {
                delay_factor: 3.0,
                duration_ticks: cycle_ticks * 4,
            });
            messages.push("Risk event: Dock congestion (delay x3.0)".to_string());
        }
        RiskHotkey::FuelShock => {
            sim.simulation.apply_event(RiskEvent::FuelShock {
                production_factor: 0.5,
                duration_ticks: cycle_ticks * 6,
            });
            messages.push("Risk event: Fuel shock (production x0.5)".to_string());
        }
    }
}

pub fn drive_simulation(
    time: Res<Time>,
    mut clock: ResMut<SimClock>,
    mut sim: ResMut<SimResource>,
) {
    let tick_seconds = sim.simulation.config.time.tick_seconds;
    let ticks = consume_ticks(&mut clock, time.delta_secs_f64(), tick_seconds);

    for _ in 0..ticks {
        let prev_cycle = sim.simulation.cycle;
        sim.last_tick_report = sim.simulation.step_tick();
        if sim.simulation.cycle != prev_cycle {
            sim.last_cycle_report = derive_cycle_report(&sim.simulation);
        }
    }
}

pub fn derive_cycle_report(simulation: &Simulation) -> CycleReport {
    let total_sla = simulation.sla_successes + simulation.sla_failures;
    let sla_success_rate = if total_sla == 0 {
        1.0
    } else {
        simulation.sla_successes as f64 / total_sla as f64
    };

    let average_gate_load = if simulation.world.edges.is_empty() {
        0.0
    } else {
        simulation
            .world
            .edges
            .iter()
            .map(|edge| {
                let load = simulation
                    .gate_queue_load
                    .get(&edge.id)
                    .copied()
                    .unwrap_or(0.0);
                let effective_capacity = (edge.base_capacity * edge.capacity_factor).max(1.0);
                load / effective_capacity
            })
            .sum::<f64>()
            / simulation.world.edges.len() as f64
    };

    let mut price_samples = 0_u64;
    let mut total_price_index = 0.0_f64;
    for market in simulation.markets.values() {
        for state in market.goods.values() {
            if state.base_price > 0.0 {
                total_price_index += state.price / state.base_price;
                price_samples += 1;
            }
        }
    }
    let average_price_index = if price_samples == 0 {
        1.0
    } else {
        total_price_index / price_samples as f64
    };

    let economy_stress_index = (1.0 - sla_success_rate).clamp(0.0, 1.0)
        + average_gate_load.clamp(0.0, 1.0)
        + average_price_index.max(1.0)
        - 1.0;

    CycleReport {
        cycle: simulation.cycle,
        sla_success_rate,
        reroute_count: simulation.reroute_count,
        economy_stress_index,
    }
}
