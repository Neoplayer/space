use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use gatebound_core::{ContractTypeStageA, Simulation, SlotType, SystemId};

use crate::sim_runtime::{derive_cycle_report, SimClock, SimResource};
use crate::view_mode::CameraMode;

#[derive(Resource, Debug, Clone, Default)]
pub struct HudMessages {
    pub entries: Vec<String>,
}

impl HudMessages {
    pub fn push(&mut self, message: String) {
        self.entries.push(message);
        if self.entries.len() > 8 {
            let drain_len = self.entries.len() - 8;
            self.entries.drain(0..drain_len);
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct HudSnapshot {
    pub tick: u64,
    pub cycle: u64,
    pub capital: f64,
    pub debt: f64,
    pub interest_rate: f64,
    pub reputation: f64,
    pub recovery_events: u32,
    pub active_contracts: usize,
    pub active_ships: usize,
    pub active_leases: usize,
    pub selected_system_id: SystemId,
    pub paused: bool,
    pub speed_multiplier: u32,
    pub sla_success_rate: f64,
    pub reroutes: u64,
    pub avg_price_index: f64,
    pub camera_mode: String,
    pub contract_lines: Vec<String>,
    pub ship_lines: Vec<String>,
    pub lease_lines: Vec<String>,
    pub lease_market_lines: Vec<String>,
}

pub fn build_hud_snapshot(
    simulation: &Simulation,
    paused: bool,
    speed_multiplier: u32,
    camera_mode: CameraMode,
) -> HudSnapshot {
    let cycle_report = derive_cycle_report(simulation);
    let selected_system_id = match camera_mode {
        CameraMode::System(system_id) => system_id,
        CameraMode::Galaxy => SystemId(0),
    };

    let mut contracts: Vec<_> = simulation
        .contracts
        .values()
        .filter(|contract| !contract.completed && !contract.failed)
        .collect();
    contracts.sort_by_key(|contract| contract.id.0);

    let contract_lines = contracts
        .iter()
        .take(6)
        .map(|contract| {
            let kind = match contract.kind {
                ContractTypeStageA::Delivery => "Delivery",
                ContractTypeStageA::Supply => "Supply",
            };
            format!(
                "#{} {kind} {} -> {} qty={:.1} deadline={} miss={} ",
                contract.id.0,
                contract.origin.0,
                contract.destination.0,
                contract.quantity,
                contract.deadline_tick,
                contract.missed_cycles,
            )
        })
        .collect::<Vec<_>>();

    let mut ships: Vec<_> = simulation.ships.values().collect();
    ships.sort_by_key(|ship| ship.id.0);

    let ship_lines = ships
        .iter()
        .take(8)
        .map(|ship| {
            let target = ship
                .current_target
                .map(|target| target.0.to_string())
                .unwrap_or_else(|| "-".to_string());
            format!(
                "#{} sys={} -> {} eta={} risk={:.2} reroutes={}",
                ship.id.0,
                ship.location.0,
                target,
                ship.eta_ticks_remaining,
                ship.last_risk_score,
                ship.reroutes,
            )
        })
        .collect::<Vec<_>>();

    let mut leases: Vec<_> = simulation.active_leases.iter().collect();
    leases.sort_by_key(|lease| (lease.system_id.0, lease.slot_type, lease.cycles_remaining));
    let lease_lines = leases
        .into_iter()
        .take(8)
        .map(|lease| {
            format!(
                "sys={} {:?} cycles={} price/cycle={:.1}",
                lease.system_id.0, lease.slot_type, lease.cycles_remaining, lease.price_per_cycle
            )
        })
        .collect::<Vec<_>>();

    let lease_market_lines = simulation
        .lease_market_for_system(selected_system_id)
        .into_iter()
        .map(|entry| {
            format!(
                "{} {}/{} @ {:.1}/cycle",
                slot_type_label(entry.slot_type),
                entry.available,
                entry.total,
                entry.price_per_cycle
            )
        })
        .collect::<Vec<_>>();

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
    let avg_price_index = if price_samples == 0 {
        1.0
    } else {
        total_price_index / price_samples as f64
    };

    HudSnapshot {
        tick: simulation.tick,
        cycle: simulation.cycle,
        capital: simulation.capital,
        debt: simulation.outstanding_debt,
        interest_rate: simulation.current_loan_interest_rate,
        reputation: simulation.reputation,
        recovery_events: simulation.recovery_events,
        active_contracts: contracts.len(),
        active_ships: simulation.ships.len(),
        active_leases: simulation.active_leases.len(),
        selected_system_id,
        paused,
        speed_multiplier,
        sla_success_rate: cycle_report.sla_success_rate,
        reroutes: simulation.reroute_count,
        avg_price_index,
        camera_mode: match camera_mode {
            CameraMode::Galaxy => "Galaxy".to_string(),
            CameraMode::System(system_id) => format!("System({})", system_id.0),
        },
        contract_lines,
        ship_lines,
        lease_lines,
        lease_market_lines,
    }
}

pub fn draw_hud_panel(
    mut egui_contexts: EguiContexts,
    sim: Res<SimResource>,
    clock: Res<SimClock>,
    camera: Res<crate::view_mode::CameraUiState>,
    messages: Res<HudMessages>,
) -> Result {
    let snapshot = build_hud_snapshot(
        &sim.simulation,
        clock.paused,
        clock.speed_multiplier,
        camera.mode,
    );

    let ctx = egui_contexts.ctx_mut()?;

    egui::TopBottomPanel::top("gatebound_top_panel").show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.label(format!("View: {}", snapshot.camera_mode));
            ui.separator();
            ui.label(format!("Tick: {}", snapshot.tick));
            ui.separator();
            ui.label(format!("Cycle: {}", snapshot.cycle));
            ui.separator();
            ui.label(format!(
                "Time: {} @ {}x",
                if snapshot.paused { "paused" } else { "running" },
                snapshot.speed_multiplier
            ));
            ui.separator();
            ui.label(format!("Capital: {:.1}", snapshot.capital));
            ui.separator();
            ui.label(format!("Debt: {:.1}", snapshot.debt));
            ui.separator();
            ui.label(format!("Rate: {:.2}%", snapshot.interest_rate * 100.0));
            ui.separator();
            ui.label(format!("Rep: {:.2}", snapshot.reputation));
            ui.separator();
            ui.label(format!("SLA: {:.2}", snapshot.sla_success_rate));
            ui.separator();
            ui.label(format!("Reroutes: {}", snapshot.reroutes));
            ui.separator();
            ui.label(format!("PriceIdx: {:.2}", snapshot.avg_price_index));
        });
    });

    egui::SidePanel::left("gatebound_left_hud")
        .resizable(true)
        .show(ctx, |ui| {
            ui.heading("Contracts");
            ui.label(format!("Active: {}", snapshot.active_contracts));
            for line in &snapshot.contract_lines {
                ui.monospace(line);
            }

            ui.separator();
            ui.heading("Fleet");
            ui.label(format!("Ships: {}", snapshot.active_ships));
            for line in &snapshot.ship_lines {
                ui.monospace(line);
            }

            ui.separator();
            ui.heading("Economy Pressure");
            ui.label(format!("Debt: {:.1}", snapshot.debt));
            ui.label(format!(
                "Interest rate: {:.2}%",
                snapshot.interest_rate * 100.0
            ));
            ui.label(format!("Reputation: {:.2}", snapshot.reputation));
            ui.label(format!("Recovery events: {}", snapshot.recovery_events));

            ui.separator();
            ui.heading("Leases");
            ui.label(format!("Active leases: {}", snapshot.active_leases));
            for line in &snapshot.lease_lines {
                ui.monospace(line);
            }
            ui.label(format!(
                "Selected system: {}",
                snapshot.selected_system_id.0
            ));
            for line in &snapshot.lease_market_lines {
                ui.monospace(line);
            }

            ui.separator();
            ui.heading("Controls");
            ui.label("Space: pause/resume");
            ui.label("1/2/4: sim speed");
            ui.label("Mouse wheel / +/-: zoom");
            ui.label("Double-click system: enter System view");
            ui.label("Esc: back to Galaxy view");
            ui.label("Z/X/C/V: lease Dock/Storage/Factory/Market");
            ui.label("R: release one lease of last selected slot type");
            ui.label("G / D / F: trigger Stage A risk events");

            if !messages.entries.is_empty() {
                ui.separator();
                ui.heading("Events");
                for message in messages.entries.iter().rev() {
                    ui.monospace(message);
                }
            }
        });
    Ok(())
}

fn slot_type_label(slot_type: SlotType) -> &'static str {
    match slot_type {
        SlotType::Dock => "Dock",
        SlotType::Storage => "Storage",
        SlotType::Factory => "Factory",
        SlotType::Market => "Market",
    }
}
