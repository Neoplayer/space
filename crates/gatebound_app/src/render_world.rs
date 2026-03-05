use bevy::prelude::*;
use bevy_egui::PrimaryEguiContext;
use gatebound_core::{ShipId, Simulation, SystemId};
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

        if ship.eta_ticks_remaining == 0 {
            cache.segments.remove(ship_id);
            continue;
        }

        let Some(target_system) = ship.current_target else {
            cache.segments.remove(ship_id);
            continue;
        };

        let from = system_position(&sim.simulation, ship.location);
        let to = system_position(&sim.simulation, target_system);

        let replace = cache
            .segments
            .get(ship_id)
            .map(|existing| {
                existing.to != to
                    || existing.from != from
                    || ship.eta_ticks_remaining > existing.total_ticks
            })
            .unwrap_or(true);

        if replace {
            cache.segments.insert(
                *ship_id,
                ShipMotionState {
                    from,
                    to,
                    total_ticks: ship.eta_ticks_remaining.max(1),
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
        gizmos.line_2d(from, to, Color::srgba(0.35, 0.45, 0.65, 0.85));
    }

    for system in &simulation.world.systems {
        let center = Vec2::new(system.x as f32, system.y as f32);
        let color = match ui_state.mode {
            CameraMode::System(selected) if selected == system.id => Color::srgb(0.40, 0.80, 1.0),
            _ => Color::srgb(0.20, 0.55, 0.95),
        };
        gizmos.circle_2d(center, system.radius as f32 * 0.18, color);

        if matches!(ui_state.mode, CameraMode::System(selected) if selected == system.id) {
            gizmos.circle_2d(
                center,
                system.radius as f32,
                Color::srgba(0.45, 0.75, 1.0, 0.55),
            );
            for gate in &system.gate_nodes {
                gizmos.circle_2d(
                    Vec2::new(gate.x as f32, gate.y as f32),
                    6.0,
                    Color::srgb(0.95, 0.65, 0.15),
                );
            }
        }
    }

    for (ship_id, ship) in &simulation.ships {
        let position = ship_position(
            simulation,
            ship_id,
            ship.location,
            ship.eta_ticks_remaining,
            &cache,
        );
        gizmos.circle_2d(position, 4.0, Color::srgb(0.94, 0.94, 0.32));
    }
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

fn system_position(simulation: &Simulation, system_id: SystemId) -> Vec2 {
    simulation
        .world
        .systems
        .iter()
        .find(|system| system.id == system_id)
        .map(|system| Vec2::new(system.x as f32, system.y as f32))
        .unwrap_or(Vec2::ZERO)
}
