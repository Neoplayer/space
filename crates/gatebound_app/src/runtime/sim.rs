use bevy::prelude::*;
use gatebound_domain::{Commodity, CycleReport, ShipId, StationId, SystemId, TickReport};
use gatebound_sim::Simulation;
use std::collections::VecDeque;

use crate::features::stations::{open_station_card, StationUiState};
use crate::input::camera::{CameraMode, CameraUiState};
use crate::runtime::save::SaveMenuState;
use crate::ui::hud::HudMessages;

#[derive(Resource, Debug, Clone, Copy, PartialEq, Default)]
pub struct UiPanelState {
    pub missions: bool,
    pub fleet: bool,
    pub markets: bool,
    pub assets: bool,
    pub policies: bool,
    pub station_ops: bool,
    pub corporations: bool,
    pub systems: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PanelButtonSpec {
    pub index: u8,
    pub label: &'static str,
    pub hotkey: &'static str,
}

const PANEL_BUTTON_SPECS: [PanelButtonSpec; 8] = [
    PanelButtonSpec {
        index: 1,
        label: "Missions",
        hotkey: "F1",
    },
    PanelButtonSpec {
        index: 2,
        label: "MyShip",
        hotkey: "F2",
    },
    PanelButtonSpec {
        index: 3,
        label: "Markets",
        hotkey: "F3",
    },
    PanelButtonSpec {
        index: 4,
        label: "Finance",
        hotkey: "F4",
    },
    PanelButtonSpec {
        index: 5,
        label: "Policies",
        hotkey: "F5",
    },
    PanelButtonSpec {
        index: 6,
        label: "Station",
        hotkey: "F6",
    },
    PanelButtonSpec {
        index: 7,
        label: "Corps",
        hotkey: "F7",
    },
    PanelButtonSpec {
        index: 8,
        label: "Systems",
        hotkey: "F8",
    },
];

pub fn panel_button_specs() -> &'static [PanelButtonSpec; 8] {
    &PANEL_BUTTON_SPECS
}

pub fn panel_is_open(panels: &UiPanelState, index: u8) -> bool {
    match index {
        1 => panels.missions,
        2 => panels.fleet,
        3 => panels.markets,
        4 => panels.assets,
        5 => panels.policies,
        6 => panels.station_ops,
        7 => panels.corporations,
        8 => panels.systems,
        _ => false,
    }
}

#[derive(Resource, Debug, Clone, PartialEq)]
pub struct UiKpiTracker {
    pub manual_action_ticks: VecDeque<u64>,
    pub policy_edit_ticks: VecDeque<u64>,
    pub manual_actions_per_min: f64,
    pub policy_edits_per_min: f64,
    pub avg_route_hops_player: f64,
}

impl Default for UiKpiTracker {
    fn default() -> Self {
        Self {
            manual_action_ticks: VecDeque::new(),
            policy_edit_ticks: VecDeque::new(),
            manual_actions_per_min: 0.0,
            policy_edits_per_min: 0.0,
            avg_route_hops_player: 0.0,
        }
    }
}

impl UiKpiTracker {
    pub fn record_manual_action(&mut self, tick: u64) {
        self.manual_action_ticks.push_back(tick);
    }

    pub fn record_policy_edit(&mut self, tick: u64) {
        self.policy_edit_ticks.push_back(tick);
    }

    pub fn update(&mut self, simulation: &Simulation) {
        let window = u64::from(simulation.time_settings_view().cycle_ticks.max(1));
        let min_tick = simulation.tick().saturating_sub(window);
        while self
            .manual_action_ticks
            .front()
            .is_some_and(|tick| *tick < min_tick)
        {
            self.manual_action_ticks.pop_front();
        }
        while self
            .policy_edit_ticks
            .front()
            .is_some_and(|tick| *tick < min_tick)
        {
            self.policy_edit_ticks.pop_front();
        }

        self.manual_actions_per_min = self.manual_action_ticks.len() as f64;
        self.policy_edits_per_min = self.policy_edit_ticks.len() as f64;
        self.avg_route_hops_player = simulation.fleet_panel_view().avg_route_hops_player;
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

#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SelectedStation {
    pub station_id: Option<StationId>,
}

#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TrackedShip {
    pub ship_id: Option<ShipId>,
}

#[derive(Resource, Debug, Clone)]
pub struct SimResource {
    pub simulation: Simulation,
    pub last_tick_report: TickReport,
    pub last_cycle_report: CycleReport,
}

impl SimResource {
    pub fn new(simulation: Simulation) -> Self {
        let overview = simulation.hud_overview_view();
        Self {
            last_tick_report: TickReport {
                tick: 0,
                cycle: 0,
                active_ships: overview.active_ships,
                active_missions: overview.active_missions,
                total_queue_delay: 0,
                avg_price_index: overview.avg_price_index,
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

pub fn selected_system_from_camera(mode: CameraMode) -> SystemId {
    match mode {
        CameraMode::System(system_id) => system_id,
        CameraMode::Galaxy => SystemId(0),
    }
}

pub fn player_ship_ids(simulation: &Simulation) -> Vec<ShipId> {
    simulation.fleet_panel_view().player_ship_ids
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

pub fn panel_hotkey_to_index(ch: char) -> Option<u8> {
    match ch {
        '1' => Some(1),
        '2' => Some(2),
        '3' => Some(3),
        '4' => Some(4),
        '5' => Some(5),
        '6' => Some(6),
        '7' => Some(7),
        '8' => Some(8),
        _ => None,
    }
}

pub fn apply_panel_toggle(panels: &mut UiPanelState, index: u8) {
    match index {
        1 => panels.missions = !panels.missions,
        2 => panels.fleet = !panels.fleet,
        3 => panels.markets = !panels.markets,
        4 => panels.assets = !panels.assets,
        5 => panels.policies = !panels.policies,
        6 => panels.station_ops = !panels.station_ops,
        7 => panels.corporations = !panels.corporations,
        8 => panels.systems = !panels.systems,
        _ => {}
    }
}

pub fn open_system_view(mode: &mut CameraMode, system_id: SystemId) {
    *mode = CameraMode::System(system_id);
}

pub fn preferred_trade_commodity(
    simulation: &Simulation,
    ship_id: Option<ShipId>,
    station_id: StationId,
    fallback: Commodity,
) -> Commodity {
    ship_id
        .and_then(|selected_ship_id| simulation.station_trade_view(selected_ship_id, station_id))
        .and_then(|view| {
            view.cargo_lots
                .into_iter()
                .filter(|cargo| cargo.source == gatebound_domain::CargoSource::Spot)
                .max_by(|left, right| left.amount.total_cmp(&right.amount))
                .map(|cargo| cargo.commodity)
        })
        .unwrap_or(fallback)
}

pub fn track_ship(
    tracked_ship: &mut TrackedShip,
    camera: &mut CameraUiState,
    simulation: &Simulation,
    ship_id: ShipId,
) -> Option<SystemId> {
    let system_id = simulation.ship_card_view(ship_id)?.location;
    tracked_ship.ship_id = Some(ship_id);
    open_system_view(&mut camera.mode, system_id);
    Some(system_id)
}

pub fn toggle_pause(clock: &mut SimClock) {
    clock.paused = !clock.paused;
}

pub fn set_time_speed(clock: &mut SimClock, speed_multiplier: u32) {
    clock.speed_multiplier = speed_multiplier.max(1);
}

pub fn apply_time_controls(
    keys: Res<ButtonInput<KeyCode>>,
    save_menu: Res<SaveMenuState>,
    mut clock: ResMut<SimClock>,
) {
    if save_menu.open {
        return;
    }

    if keys.just_pressed(KeyCode::Space) {
        toggle_pause(&mut clock);
    }

    if keys.just_pressed(KeyCode::Digit1) {
        set_time_speed(&mut clock, 1);
    }
    if keys.just_pressed(KeyCode::Digit2) {
        set_time_speed(&mut clock, 2);
    }
    if keys.just_pressed(KeyCode::Digit4) {
        set_time_speed(&mut clock, 4);
    }
}

pub fn sync_selected_system(camera: Res<CameraUiState>, mut selected: ResMut<SelectedSystem>) {
    selected.system_id = selected_system_from_camera(camera.mode);
}

pub fn sync_selected_station(
    sim: Res<SimResource>,
    selected_system: Res<SelectedSystem>,
    mut selected_station: ResMut<SelectedStation>,
) {
    let system_id = selected_system.system_id;
    let topology = sim.simulation.camera_topology_view();
    let current_system = topology
        .systems
        .iter()
        .find(|system| system.system_id == system_id);
    let in_system = |station_id: StationId| {
        current_system.is_some_and(|system| {
            system
                .stations
                .iter()
                .any(|station| station.station_id == station_id)
        })
    };
    if selected_station.station_id.is_some_and(in_system) {
        return;
    }
    selected_station.station_id = current_system
        .and_then(|system| system.stations.first())
        .map(|station| station.station_id);
}

#[allow(clippy::too_many_arguments)]
pub fn handle_panel_hotkeys(
    keys: Res<ButtonInput<KeyCode>>,
    save_menu: Res<SaveMenuState>,
    mut panels: ResMut<UiPanelState>,
    mut selected_ship: ResMut<SelectedShip>,
    selected_station: Res<SelectedStation>,
    sim: Res<SimResource>,
    mut station_ui: ResMut<StationUiState>,
    mut kpi: ResMut<UiKpiTracker>,
) {
    if save_menu.open {
        return;
    }

    let mut manual_action = false;
    if keys.just_pressed(KeyCode::F1) {
        apply_panel_toggle(&mut panels, 1);
        manual_action = true;
    }
    if keys.just_pressed(KeyCode::F2) {
        apply_panel_toggle(&mut panels, 2);
        manual_action = true;
    }
    if keys.just_pressed(KeyCode::F3) {
        apply_panel_toggle(&mut panels, 3);
        manual_action = true;
    }
    if keys.just_pressed(KeyCode::F4) {
        apply_panel_toggle(&mut panels, 4);
        manual_action = true;
    }
    if keys.just_pressed(KeyCode::F5) {
        apply_panel_toggle(&mut panels, 5);
        manual_action = true;
    }
    if keys.just_pressed(KeyCode::F6) {
        apply_panel_toggle(&mut panels, 6);
        station_ui.station_panel_open = panels.station_ops;
        if panels.station_ops {
            if selected_ship.ship_id.is_none() {
                selected_ship.ship_id = player_ship_ids(&sim.simulation).first().copied();
            }
            if let Some(station_id) = selected_station.station_id.or(station_ui.card_station_id) {
                let preferred = preferred_trade_commodity(
                    &sim.simulation,
                    selected_ship.ship_id,
                    station_id,
                    station_ui.trade_commodity,
                );
                open_station_card(&mut station_ui, station_id, Some(preferred));
            }
        }
        manual_action = true;
    }
    if keys.just_pressed(KeyCode::F7) {
        apply_panel_toggle(&mut panels, 7);
        manual_action = true;
    }
    if keys.just_pressed(KeyCode::F8) {
        apply_panel_toggle(&mut panels, 8);
        manual_action = true;
    }

    let ship_ids = player_ship_ids(&sim.simulation);
    if selected_ship.ship_id.is_none() {
        selected_ship.ship_id = ship_ids.first().copied();
    }
    if keys.just_pressed(KeyCode::BracketRight) {
        selected_ship.ship_id = cycle_selected_ship(selected_ship.ship_id, &ship_ids, true);
        manual_action = true;
    }
    if keys.just_pressed(KeyCode::BracketLeft) {
        selected_ship.ship_id = cycle_selected_ship(selected_ship.ship_id, &ship_ids, false);
        manual_action = true;
    }

    if manual_action {
        kpi.record_manual_action(sim.simulation.tick());
    }
}

pub fn handle_risk_hotkeys(
    keys: Res<ButtonInput<KeyCode>>,
    save_menu: Res<SaveMenuState>,
    mut sim: ResMut<SimResource>,
    mut messages: ResMut<HudMessages>,
    mut kpi: ResMut<UiKpiTracker>,
) {
    if save_menu.open {
        return;
    }

    let cycle_ticks = sim.simulation.time_settings_view().cycle_ticks;
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
    kpi.record_manual_action(sim.simulation.tick());

    match action {
        RiskHotkey::GateCongestion => {
            if let Some(edge) = sim
                .simulation
                .camera_topology_view()
                .gate_ids
                .first()
                .copied()
            {
                sim.simulation
                    .inject_gate_congestion(edge, 0.5, cycle_ticks * 5);
                messages.push(format!(
                    "Risk event: Gate congestion on edge {} (capacity x0.5)",
                    edge.0
                ));
            }
        }
        RiskHotkey::DockCongestion => {
            sim.simulation.inject_dock_congestion(3.0, cycle_ticks * 4);
            messages.push("Risk event: Dock congestion (delay x3.0)".to_string());
        }
        RiskHotkey::FuelShock => {
            sim.simulation.inject_fuel_shock(0.5, cycle_ticks * 6);
            messages.push("Risk event: Fuel shock (production x0.5)".to_string());
        }
    }
}

pub fn drive_simulation(
    time: Res<Time>,
    mut clock: ResMut<SimClock>,
    mut sim: ResMut<SimResource>,
    mut kpi: ResMut<UiKpiTracker>,
) {
    let tick_seconds = sim.simulation.time_settings_view().tick_seconds;
    let ticks = consume_ticks(&mut clock, time.delta_secs_f64(), tick_seconds);

    for _ in 0..ticks {
        let prev_cycle = sim.simulation.cycle();
        sim.last_tick_report = sim.simulation.step_tick();
        if sim.simulation.cycle() != prev_cycle {
            sim.last_cycle_report = sim.simulation.cycle_report();
        }
    }
    kpi.update(&sim.simulation);
}

pub fn derive_cycle_report(simulation: &Simulation) -> CycleReport {
    simulation.cycle_report()
}
