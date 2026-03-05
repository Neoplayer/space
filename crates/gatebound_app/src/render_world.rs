use bevy::prelude::*;
use bevy_egui::PrimaryEguiContext;
use gatebound_core::{
    Commodity, RouteSegment, SegmentKind, Ship, ShipId, Simulation, StationId, SystemId,
};
use std::collections::BTreeMap;

use crate::sim_runtime::SimResource;
use crate::view_mode::{CameraMode, CameraUiState};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShipMotionState {
    pub from: Vec2,
    pub to: Vec2,
    pub total_ticks: u32,
}

#[derive(Resource, Debug, Clone, Default)]
pub struct ShipMotionCache {
    pub segments: BTreeMap<ShipId, ShipMotionState>,
}

impl ShipMotionCache {
    pub fn progress_ratio(total_ticks: u32, eta_ticks_remaining: u32) -> f32 {
        if total_ticks == 0 {
            return 1.0;
        }
        let progress = 1.0 - eta_ticks_remaining as f32 / total_ticks as f32;
        progress.clamp(0.0, 1.0)
    }
}

pub fn setup_camera(mut commands: Commands) {
    commands.spawn((Camera2d, PrimaryEguiContext));
}

pub fn update_ship_motion_cache(mut cache: ResMut<ShipMotionCache>, sim: Res<SimResource>) {
    let mut stale: Vec<ShipId> = cache.segments.keys().copied().collect();

    for (ship_id, ship) in &sim.simulation.ships {
        stale.retain(|candidate| candidate != ship_id);

        if ship.segment_eta_remaining == 0
            || ship.current_segment_kind != Some(SegmentKind::InSystem)
        {
            cache.segments.remove(ship_id);
            continue;
        }

        let Some(segment) = ship.movement_queue.front() else {
            cache.segments.remove(ship_id);
            continue;
        };
        if segment.kind != SegmentKind::InSystem {
            cache.segments.remove(ship_id);
            continue;
        }

        let from = segment_from_point(&sim.simulation, ship, segment);
        let to = if let Some(anchor_id) = segment.to_anchor {
            station_position(&sim.simulation, anchor_id).unwrap_or_else(|| {
                segment_endpoint(&sim.simulation, segment.to, None, segment.edge)
            })
        } else {
            segment_endpoint(&sim.simulation, segment.to, None, segment.edge)
        };

        let replace = cache
            .segments
            .get(ship_id)
            .map(|existing| {
                existing.to != to
                    || existing.from != from
                    || ship.segment_progress_total > existing.total_ticks
            })
            .unwrap_or(true);

        if replace {
            cache.segments.insert(
                *ship_id,
                ShipMotionState {
                    from,
                    to,
                    total_ticks: ship.segment_progress_total.max(1),
                },
            );
        }
    }

    for ship_id in stale {
        cache.segments.remove(&ship_id);
    }
}

pub fn draw_world_gizmos(
    mut gizmos: Gizmos,
    sim: Res<SimResource>,
    ui_state: Res<CameraUiState>,
    cache: Res<ShipMotionCache>,
) {
    let simulation = &sim.simulation;

    for edge in &simulation.world.edges {
        let from = system_position(simulation, edge.a);
        let to = system_position(simulation, edge.b);
        let load = simulation
            .gate_queue_load
            .get(&edge.id)
            .copied()
            .unwrap_or(0.0);
        let effective_capacity = (edge.base_capacity * edge.capacity_factor).max(1.0);
        let pressure = (load / effective_capacity).clamp(0.0, 2.0) as f32;
        let color = Color::srgba(0.25 + pressure * 0.35, 0.40, 0.65 - pressure * 0.25, 0.90);
        gizmos.line_2d(from, to, color);
        if pressure > 0.15 {
            let midpoint = from.lerp(to, 0.5);
            gizmos.circle_2d(
                midpoint,
                1.0 + pressure * 2.5,
                Color::srgba(1.0, 0.45, 0.15, 0.55),
            );
        }
    }

    for system in &simulation.world.systems {
        let center = Vec2::new(system.x as f32, system.y as f32);
        let system_open = system_objects_visible_in_current_view(ui_state.mode, system.id);
        let color = match ui_state.mode {
            CameraMode::System(selected) if selected == system.id => Color::srgb(0.40, 0.80, 1.0),
            _ => Color::srgb(0.20, 0.55, 0.95),
        };
        gizmos.circle_2d(center, system.radius as f32 * 0.18, color);

        if system_open {
            let dock_pressure = dock_congestion_index(simulation, system.id);
            let fuel_stress = fuel_stress_index(simulation, system.id);
            if dock_pressure > 0.15 {
                gizmos.circle_2d(
                    center,
                    system.radius as f32 * (0.24 + dock_pressure * 0.08),
                    Color::srgba(1.0, 0.72, 0.18, 0.4 + dock_pressure * 0.2),
                );
            }
            if fuel_stress > 0.20 {
                gizmos.circle_2d(
                    center,
                    system.radius as f32 * (0.30 + fuel_stress * 0.12),
                    Color::srgba(1.0, 0.2, 0.2, 0.35 + fuel_stress * 0.25),
                );
            }

            for station_id in simulation
                .world
                .stations_by_system
                .get(&system.id)
                .into_iter()
                .flatten()
            {
                if let Some(station) = simulation
                    .world
                    .stations
                    .iter()
                    .find(|anchor| anchor.id == *station_id)
                {
                    gizmos.circle_2d(
                        Vec2::new(station.x as f32, station.y as f32),
                        3.8,
                        Color::srgba(0.78, 0.90, 1.0, 0.95),
                    );
                }
            }
            let tick_phase = (simulation.tick % 120) as f32 / 120.0;
            let pulse = 0.85 + 0.25 * (tick_phase * std::f32::consts::TAU).sin().abs();
            gizmos.circle_2d(
                center,
                system.radius as f32,
                Color::srgba(0.45, 0.75, 1.0, 0.55),
            );
            gizmos.circle_2d(
                center,
                system.radius as f32 * 0.12,
                Color::srgba(1.0, 0.85, 0.25, 0.8),
            );
            for orbit in [0.26_f32, 0.42_f32, 0.58_f32] {
                gizmos.circle_2d(
                    center,
                    system.radius as f32 * orbit,
                    Color::srgba(0.7, 0.8, 1.0, 0.16),
                );
            }
            for gate in &system.gate_nodes {
                gizmos.circle_2d(
                    Vec2::new(gate.x as f32, gate.y as f32),
                    6.0,
                    Color::srgb(0.95, 0.65, 0.15),
                );
                gizmos.circle_2d(
                    Vec2::new(gate.x as f32, gate.y as f32),
                    8.0 * pulse,
                    Color::srgba(1.0, 0.7, 0.2, 0.3),
                );
            }
        }
    }

    for (ship_id, ship) in &simulation.ships {
        if !ship_is_visible_in_current_view(ui_state.mode, ship.location) {
            continue;
        }
        let position = ship_position(
            simulation,
            ship_id,
            ship.location,
            ship.segment_eta_remaining,
            &cache,
        );
        gizmos.circle_2d(position, 4.0, company_color(ship.company_id.0));
        if let Some(cargo) = ship.cargo {
            gizmos.circle_2d(
                position + Vec2::new(3.0, -3.0),
                1.8,
                cargo_color(cargo.commodity),
            );
        }
    }
}

pub(crate) fn ship_is_visible_in_current_view(mode: CameraMode, ship_location: SystemId) -> bool {
    match mode {
        CameraMode::Galaxy => false,
        CameraMode::System(selected_system) => ship_location == selected_system,
    }
}

pub(crate) fn system_objects_visible_in_current_view(
    mode: CameraMode,
    system_id: SystemId,
) -> bool {
    matches!(mode, CameraMode::System(selected_system) if selected_system == system_id)
}

fn ship_position(
    simulation: &Simulation,
    ship_id: &ShipId,
    fallback_system: SystemId,
    eta_ticks_remaining: u32,
    cache: &ShipMotionCache,
) -> Vec2 {
    if let Some(segment) = cache.segments.get(ship_id) {
        let t = ShipMotionCache::progress_ratio(segment.total_ticks, eta_ticks_remaining);
        return segment.from.lerp(segment.to, t);
    }
    system_position(simulation, fallback_system)
}

pub(crate) fn segment_from_point(
    simulation: &Simulation,
    ship: &Ship,
    segment: &RouteSegment,
) -> Vec2 {
    if let Some(station_id) = segment.from_anchor {
        if let Some(position) = station_position(simulation, station_id) {
            return position;
        }
    }
    if let Some(gate_id) = ship.last_gate_arrival {
        if let Some((x, y)) = simulation.world.gate_coords(segment.from, gate_id) {
            return Vec2::new(x as f32, y as f32);
        }
    }
    segment_endpoint(simulation, segment.from, None, segment.edge)
}

fn segment_endpoint(
    simulation: &Simulation,
    system_id: SystemId,
    station_id: Option<StationId>,
    edge: Option<gatebound_core::GateId>,
) -> Vec2 {
    if let Some(station_id) = station_id {
        if let Some(position) = station_position(simulation, station_id) {
            return position;
        }
    }
    if let Some(gate_id) = edge {
        if let Some((x, y)) = simulation.world.gate_coords(system_id, gate_id) {
            return Vec2::new(x as f32, y as f32);
        }
    }
    system_position(simulation, system_id)
}

fn system_position(simulation: &Simulation, system_id: SystemId) -> Vec2 {
    simulation
        .world
        .systems
        .iter()
        .find(|system| system.id == system_id)
        .map(|system| Vec2::new(system.x as f32, system.y as f32))
        .unwrap_or(Vec2::ZERO)
}

fn station_position(simulation: &Simulation, station_id: StationId) -> Option<Vec2> {
    simulation
        .station_position(station_id)
        .map(|(x, y)| Vec2::new(x as f32, y as f32))
}

fn dock_congestion_index(simulation: &Simulation, system_id: SystemId) -> f32 {
    let inbound = simulation
        .ships
        .values()
        .filter(|ship| ship.current_target == Some(system_id) && ship.eta_ticks_remaining > 0)
        .count() as f32;
    (inbound / 6.0).clamp(0.0, 1.0)
}

fn fuel_stress_index(simulation: &Simulation, system_id: SystemId) -> f32 {
    let Some(stations) = simulation.world.stations_by_system.get(&system_id) else {
        return 0.0;
    };
    let mut stock = 0.0;
    let mut target = 0.0;
    for station_id in stations {
        if let Some(fuel) = simulation
            .markets
            .get(station_id)
            .and_then(|market| market.goods.get(&Commodity::Fuel))
        {
            stock += fuel.stock;
            target += fuel.target_stock;
        }
    }
    if target <= 0.0 {
        return 0.0;
    }
    let ratio = (stock / target).clamp(0.0, 1.0);
    (1.0 - ratio as f32).clamp(0.0, 1.0)
}

fn company_color(company_id: usize) -> Color {
    match company_id % 5 {
        0 => Color::srgb(0.94, 0.94, 0.32),
        1 => Color::srgb(0.35, 0.84, 1.0),
        2 => Color::srgb(0.96, 0.55, 0.22),
        3 => Color::srgb(0.52, 0.95, 0.52),
        _ => Color::srgb(0.92, 0.48, 0.78),
    }
}

fn cargo_color(commodity: Commodity) -> Color {
    match commodity {
        Commodity::Ore => Color::srgb(0.58, 0.58, 0.62),
        Commodity::Ice => Color::srgb(0.68, 0.88, 1.0),
        Commodity::Gas => Color::srgb(0.64, 0.84, 0.72),
        Commodity::Metal => Color::srgb(0.78, 0.74, 0.68),
        Commodity::Fuel => Color::srgb(1.0, 0.72, 0.22),
        Commodity::Parts => Color::srgb(0.84, 0.68, 0.94),
        Commodity::Electronics => Color::srgb(0.45, 1.0, 0.8),
    }
}
