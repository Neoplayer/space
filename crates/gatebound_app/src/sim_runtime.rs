use bevy::prelude::*;
use gatebound_core::{
    CompanyId, ContractOffer, CycleReport, LeaseError, RiskEvent, ShipId, Simulation, SlotType,
    SystemId, TickReport,
};

use crate::hud::HudMessages;
use crate::view_mode::{CameraMode, CameraUiState};

#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct UiPanelState {
    pub contracts: bool,
    pub fleet: bool,
    pub markets: bool,
    pub assets: bool,
    pub policies: bool,
}

impl Default for UiPanelState {
    fn default() -> Self {
        Self {
            contracts: true,
            fleet: true,
            markets: true,
            assets: true,
            policies: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OfferSortMode {
    MarginDesc,
    RiskAsc,
    EtaAsc,
}

#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct ContractsFilterState {
    pub min_margin: f64,
    pub max_risk: f64,
    pub max_eta: u32,
    pub sort_mode: OfferSortMode,
}

impl Default for ContractsFilterState {
    fn default() -> Self {
        Self {
            min_margin: 0.0,
            max_risk: 2.0,
            max_eta: 240,
            sort_mode: OfferSortMode::MarginDesc,
        }
    }
}

#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SelectedShip {
    pub ship_id: Option<ShipId>,
}

#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectedSystem {
    pub system_id: SystemId,
}

impl Default for SelectedSystem {
    fn default() -> Self {
        Self {
            system_id: SystemId(0),
        }
    }
}

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

#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct LeaseSelection {
    pub slot_type: SlotType,
}

impl Default for LeaseSelection {
    fn default() -> Self {
        Self {
            slot_type: SlotType::Dock,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeaseHotkey {
    Dock,
    Storage,
    Factory,
    Market,
    Release,
}

pub fn hotkey_to_lease(ch: char) -> Option<LeaseHotkey> {
    match ch.to_ascii_lowercase() {
        'z' => Some(LeaseHotkey::Dock),
        'x' => Some(LeaseHotkey::Storage),
        'c' => Some(LeaseHotkey::Factory),
        'v' => Some(LeaseHotkey::Market),
        'r' => Some(LeaseHotkey::Release),
        _ => None,
    }
}

pub fn selected_system_from_camera(mode: CameraMode) -> SystemId {
    match mode {
        CameraMode::System(system_id) => system_id,
        CameraMode::Galaxy => SystemId(0),
    }
}

pub fn player_ship_ids(simulation: &Simulation) -> Vec<ShipId> {
    let mut ids = simulation
        .ships
        .values()
        .filter(|ship| ship.company_id == CompanyId(0))
        .map(|ship| ship.id)
        .collect::<Vec<_>>();
    ids.sort_by_key(|id| id.0);
    ids
}

pub fn cycle_selected_ship(
    current: Option<ShipId>,
    ship_ids: &[ShipId],
    forward: bool,
) -> Option<ShipId> {
    if ship_ids.is_empty() {
        return None;
    }
    let current_idx = current
        .and_then(|id| ship_ids.iter().position(|candidate| *candidate == id))
        .unwrap_or(0);
    let next_idx = if forward {
        (current_idx + 1) % ship_ids.len()
    } else if current_idx == 0 {
        ship_ids.len() - 1
    } else {
        current_idx - 1
    };
    Some(ship_ids[next_idx])
}

pub fn apply_offer_filters(
    mut offers: Vec<ContractOffer>,
    filters: ContractsFilterState,
) -> Vec<ContractOffer> {
    offers.retain(|offer| {
        offer.margin_estimate >= filters.min_margin
            && offer.risk_score <= filters.max_risk
            && offer.eta_ticks <= filters.max_eta
    });
    match filters.sort_mode {
        OfferSortMode::MarginDesc => {
            offers.sort_by(|a, b| b.margin_estimate.total_cmp(&a.margin_estimate))
        }
        OfferSortMode::RiskAsc => offers.sort_by(|a, b| a.risk_score.total_cmp(&b.risk_score)),
        OfferSortMode::EtaAsc => offers.sort_by_key(|offer| offer.eta_ticks),
    }
    offers
}

pub fn panel_hotkey_to_index(ch: char) -> Option<u8> {
    match ch {
        '1' => Some(1),
        '2' => Some(2),
        '3' => Some(3),
        '4' => Some(4),
        '5' => Some(5),
        _ => None,
    }
}

pub fn apply_panel_toggle(panels: &mut UiPanelState, index: u8) {
    match index {
        1 => panels.contracts = !panels.contracts,
        2 => panels.fleet = !panels.fleet,
        3 => panels.markets = !panels.markets,
        4 => panels.assets = !panels.assets,
        5 => panels.policies = !panels.policies,
        _ => {}
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

pub fn sync_selected_system(camera: Res<CameraUiState>, mut selected: ResMut<SelectedSystem>) {
    selected.system_id = selected_system_from_camera(camera.mode);
}

pub fn handle_panel_hotkeys(
    keys: Res<ButtonInput<KeyCode>>,
    mut panels: ResMut<UiPanelState>,
    mut selected_ship: ResMut<SelectedShip>,
    sim: Res<SimResource>,
) {
    if keys.just_pressed(KeyCode::F1) {
        apply_panel_toggle(&mut panels, 1);
    }
    if keys.just_pressed(KeyCode::F2) {
        apply_panel_toggle(&mut panels, 2);
    }
    if keys.just_pressed(KeyCode::F3) {
        apply_panel_toggle(&mut panels, 3);
    }
    if keys.just_pressed(KeyCode::F4) {
        apply_panel_toggle(&mut panels, 4);
    }
    if keys.just_pressed(KeyCode::F5) {
        apply_panel_toggle(&mut panels, 5);
    }

    let ship_ids = player_ship_ids(&sim.simulation);
    if selected_ship.ship_id.is_none() {
        selected_ship.ship_id = ship_ids.first().copied();
    }
    if keys.just_pressed(KeyCode::BracketRight) {
        selected_ship.ship_id = cycle_selected_ship(selected_ship.ship_id, &ship_ids, true);
    }
    if keys.just_pressed(KeyCode::BracketLeft) {
        selected_ship.ship_id = cycle_selected_ship(selected_ship.ship_id, &ship_ids, false);
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

pub fn handle_lease_hotkeys(
    keys: Res<ButtonInput<KeyCode>>,
    selected_system: Res<SelectedSystem>,
    mut selection: ResMut<LeaseSelection>,
    mut sim: ResMut<SimResource>,
    mut messages: ResMut<HudMessages>,
) {
    let action = if keys.just_pressed(KeyCode::KeyZ) {
        hotkey_to_lease('z')
    } else if keys.just_pressed(KeyCode::KeyX) {
        hotkey_to_lease('x')
    } else if keys.just_pressed(KeyCode::KeyC) {
        hotkey_to_lease('c')
    } else if keys.just_pressed(KeyCode::KeyV) {
        hotkey_to_lease('v')
    } else if keys.just_pressed(KeyCode::KeyR) {
        hotkey_to_lease('r')
    } else {
        None
    };

    let Some(action) = action else {
        return;
    };

    let selected_system = selected_system.system_id;
    const DEFAULT_LEASE_CYCLES: u32 = 3;

    match action {
        LeaseHotkey::Dock => lease_slot(
            &mut sim.simulation,
            &mut selection,
            &mut messages,
            selected_system,
            SlotType::Dock,
            DEFAULT_LEASE_CYCLES,
        ),
        LeaseHotkey::Storage => lease_slot(
            &mut sim.simulation,
            &mut selection,
            &mut messages,
            selected_system,
            SlotType::Storage,
            DEFAULT_LEASE_CYCLES,
        ),
        LeaseHotkey::Factory => lease_slot(
            &mut sim.simulation,
            &mut selection,
            &mut messages,
            selected_system,
            SlotType::Factory,
            DEFAULT_LEASE_CYCLES,
        ),
        LeaseHotkey::Market => lease_slot(
            &mut sim.simulation,
            &mut selection,
            &mut messages,
            selected_system,
            SlotType::Market,
            DEFAULT_LEASE_CYCLES,
        ),
        LeaseHotkey::Release => {
            let released = sim
                .simulation
                .release_one_slot(selected_system, selection.slot_type);
            if released {
                messages.push(format!(
                    "Released {:?} slot lease in system {}",
                    selection.slot_type, selected_system.0
                ));
            } else {
                messages.push(format!(
                    "No {:?} lease to release in system {}",
                    selection.slot_type, selected_system.0
                ));
            }
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

fn lease_slot(
    simulation: &mut Simulation,
    selection: &mut LeaseSelection,
    messages: &mut HudMessages,
    system_id: SystemId,
    slot_type: SlotType,
    cycles: u32,
) {
    selection.slot_type = slot_type;
    match simulation.lease_slot(system_id, slot_type, cycles) {
        Ok(()) => {
            messages.push(format!(
                "Leased {:?} slot in system {} for {} cycles",
                slot_type, system_id.0, cycles
            ));
        }
        Err(err) => {
            let reason = match err {
                LeaseError::NoCapacity => "no capacity",
                LeaseError::InvalidCycles => "invalid cycles",
                LeaseError::UnknownSystem => "unknown system",
            };
            messages.push(format!(
                "Lease {:?} in system {} failed: {}",
                slot_type, system_id.0, reason
            ));
        }
    }
}
