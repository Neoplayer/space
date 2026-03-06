use bevy::prelude::*;
use bevy_egui::PrimaryEguiContext;
use gatebound_domain::{Commodity, GateId, RouteSegment, SegmentKind, ShipId, StationId, SystemId};
use gatebound_sim::{RenderShipView, WorldRenderSnapshot};
use std::collections::BTreeMap;

use crate::input::camera::{CameraMode, CameraUiState};
use crate::runtime::sim::{SelectedShip, SelectedStation, SimResource, TrackedShip};

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
    let snapshot = sim.simulation.world_render_snapshot();
    let mut stale: Vec<ShipId> = cache.segments.keys().copied().collect();

    for ship in &snapshot.ships {
        stale.retain(|candidate| *candidate != ship.ship_id);

        if ship.segment_eta_remaining == 0
            || ship.current_segment_kind != Some(SegmentKind::InSystem)
        {
            cache.segments.remove(&ship.ship_id);
            continue;
        }

        let Some(segment) = ship.front_segment.as_ref() else {
            cache.segments.remove(&ship.ship_id);
            continue;
        };
        if segment.kind != SegmentKind::InSystem {
            cache.segments.remove(&ship.ship_id);
            continue;
        }

        let from = segment_from_point(&snapshot, ship, segment);
        let to = if let Some(anchor_id) = segment.to_anchor {
            station_anchor_position(&snapshot, anchor_id)
                .unwrap_or_else(|| segment_endpoint(&snapshot, segment.to, None, segment.edge))
        } else {
            segment_endpoint(&snapshot, segment.to, None, segment.edge)
        };

        let replace = cache
            .segments
            .get(&ship.ship_id)
            .map(|existing| {
                existing.to != to
                    || existing.from != from
                    || ship.segment_progress_total > existing.total_ticks
            })
            .unwrap_or(true);

        if replace {
            cache.segments.insert(
                ship.ship_id,
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
    selected_station: Res<SelectedStation>,
    selected_ship: Res<SelectedShip>,
    tracked_ship: Res<TrackedShip>,
) {
    let snapshot = sim.simulation.world_render_snapshot();
    let player_destination_station = selected_ship
        .ship_id
        .and_then(|ship_id| snapshot.ships.iter().find(|ship| ship.ship_id == ship_id))
        .and_then(|ship| ship.front_segment.as_ref())
        .and_then(|segment| segment.to_anchor);

    for edge in &snapshot.edges {
        let from = system_position(&snapshot, edge.from_system);
        let to = system_position(&snapshot, edge.to_system);
        let pressure = (edge.load / edge.effective_capacity).clamp(0.0, 2.0) as f32;
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

    for system in &snapshot.systems {
        let center = Vec2::new(system.x as f32, system.y as f32);
        let system_open = system_objects_visible_in_current_view(ui_state.mode, system.system_id);
        let color = match ui_state.mode {
            CameraMode::System(selected) if selected == system.system_id => {
                Color::srgb(0.40, 0.80, 1.0)
            }
            _ => Color::srgb(0.20, 0.55, 0.95),
        };
        gizmos.circle_2d(center, system.radius as f32 * 0.18, color);

        if system_open {
            let dock_pressure = system.dock_congestion;
            let fuel_stress = system.fuel_stress;
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

            for station in &system.stations {
                gizmos.circle_2d(
                    Vec2::new(station.x as f32, station.y as f32),
                    3.8,
                    Color::srgba(0.78, 0.90, 1.0, 0.95),
                );
                if selected_station.station_id == Some(station.station_id) {
                    gizmos.circle_2d(
                        Vec2::new(station.x as f32, station.y as f32),
                        6.2,
                        Color::srgba(0.95, 0.95, 0.35, 0.95),
                    );
                }
                if player_destination_station == Some(station.station_id) {
                    gizmos.circle_2d(
                        Vec2::new(station.x as f32, station.y as f32),
                        8.0,
                        Color::srgba(0.35, 1.0, 0.6, 0.8),
                    );
                }
            }
            let tick_phase = (snapshot.tick % 120) as f32 / 120.0;
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

    for ship in &snapshot.ships {
        if !ship_is_visible_in_current_view(ui_state.mode, ship.location) {
            continue;
        }
        let position = ship_position(
            &snapshot,
            ship.ship_id,
            ship.location,
            ship.segment_eta_remaining,
            &cache,
        );
        gizmos.circle_2d(position, 4.0, company_color(ship.company_id.0));
        if tracked_ship.ship_id == Some(ship.ship_id) {
            gizmos.circle_2d(position, 7.2, Color::srgba(0.95, 0.95, 0.35, 0.95));
        }
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

pub(crate) fn ship_position(
    snapshot: &WorldRenderSnapshot,
    ship_id: ShipId,
    fallback_system: SystemId,
    eta_ticks_remaining: u32,
    cache: &ShipMotionCache,
) -> Vec2 {
    if let Some(segment) = cache.segments.get(&ship_id) {
        let t = ShipMotionCache::progress_ratio(segment.total_ticks, eta_ticks_remaining);
        return segment.from.lerp(segment.to, t);
    }
    system_position(snapshot, fallback_system)
}

pub(crate) fn pick_visible_ship(
    snapshot: &WorldRenderSnapshot,
    mode: CameraMode,
    world_position: Vec2,
    cache: &ShipMotionCache,
) -> Option<ShipId> {
    snapshot
        .ships
        .iter()
        .filter(|ship| ship_is_visible_in_current_view(mode, ship.location))
        .filter_map(|ship| {
            let position = ship_position(
                snapshot,
                ship.ship_id,
                ship.location,
                ship.segment_eta_remaining,
                cache,
            );
            let dx = world_position.x - position.x;
            let dy = world_position.y - position.y;
            let distance_sq = dx * dx + dy * dy;
            (distance_sq <= 8.0_f32.powi(2)).then_some((distance_sq, ship.ship_id))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, ship_id)| ship_id)
}

pub(crate) fn segment_from_point(
    snapshot: &WorldRenderSnapshot,
    ship: &RenderShipView,
    segment: &RouteSegment,
) -> Vec2 {
    if let Some(station_id) = segment.from_anchor {
        if let Some(position) = station_anchor_position(snapshot, station_id) {
            return position;
        }
    }
    if let Some(gate_id) = ship.last_gate_arrival {
        if let Some(position) = gate_position(snapshot, segment.from, gate_id) {
            return position;
        }
    }
    segment_endpoint(snapshot, segment.from, None, segment.edge)
}

fn segment_endpoint(
    snapshot: &WorldRenderSnapshot,
    system_id: SystemId,
    station_id: Option<StationId>,
    edge: Option<GateId>,
) -> Vec2 {
    if let Some(station_id) = station_id {
        if let Some(position) = station_anchor_position(snapshot, station_id) {
            return position;
        }
    }
    if let Some(gate_id) = edge {
        if let Some(position) = gate_position(snapshot, system_id, gate_id) {
            return position;
        }
    }
    system_position(snapshot, system_id)
}

fn system_position(snapshot: &WorldRenderSnapshot, system_id: SystemId) -> Vec2 {
    snapshot
        .systems
        .iter()
        .find(|system| system.system_id == system_id)
        .map(|system| Vec2::new(system.x as f32, system.y as f32))
        .unwrap_or(Vec2::ZERO)
}

fn station_anchor_position(snapshot: &WorldRenderSnapshot, station_id: StationId) -> Option<Vec2> {
    snapshot
        .systems
        .iter()
        .flat_map(|system| system.stations.iter())
        .find(|station| station.station_id == station_id)
        .map(|station| Vec2::new(station.x as f32, station.y as f32))
}

fn gate_position(
    snapshot: &WorldRenderSnapshot,
    system_id: SystemId,
    gate_id: GateId,
) -> Option<Vec2> {
    snapshot
        .systems
        .iter()
        .find(|system| system.system_id == system_id)
        .and_then(|system| {
            system
                .gate_nodes
                .iter()
                .find(|gate| gate.gate_id == gate_id)
                .map(|gate| Vec2::new(gate.x as f32, gate.y as f32))
        })
}

pub(crate) fn company_color(company_id: usize) -> Color {
    const COMPANY_PALETTE: [(f32, f32, f32); 7] = [
        (0.94, 0.94, 0.32),
        (0.35, 0.84, 1.0),
        (0.96, 0.55, 0.22),
        (0.52, 0.95, 0.52),
        (0.92, 0.48, 0.78),
        (1.0, 0.72, 0.22),
        (0.42, 0.78, 0.96),
    ];
    let (r, g, b) = COMPANY_PALETTE[company_id % COMPANY_PALETTE.len()];
    Color::srgb(r, g, b)
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
