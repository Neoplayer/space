use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use gatebound_domain::{ShipId, StationId, SystemId};
use gatebound_sim::CameraTopologyView;

use crate::render::world::{pick_visible_ship, ShipMotionCache};
use crate::runtime::sim::{SelectedStation, ShipUiState, SimResource, StationUiState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraMode {
    Galaxy,
    System(SystemId),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClickTracker {
    pub last_system: Option<SystemId>,
    pub last_click_seconds: f64,
}

impl Default for ClickTracker {
    fn default() -> Self {
        Self {
            last_system: None,
            last_click_seconds: -10.0,
        }
    }
}

#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct CameraUiState {
    pub mode: CameraMode,
    pub zoom_level: f32,
    pub zoom_min: f32,
    pub zoom_max: f32,
    pub double_click_window_seconds: f64,
}

impl Default for CameraUiState {
    fn default() -> Self {
        Self {
            mode: CameraMode::Galaxy,
            zoom_level: 1.0,
            zoom_min: 0.3,
            zoom_max: 4.0,
            double_click_window_seconds: 0.35,
        }
    }
}

pub fn apply_system_click(
    mode: &mut CameraMode,
    tracker: &mut ClickTracker,
    system_id: SystemId,
    now_seconds: f64,
) -> bool {
    let is_double_click =
        tracker.last_system == Some(system_id) && now_seconds - tracker.last_click_seconds <= 0.35;

    tracker.last_system = Some(system_id);
    tracker.last_click_seconds = now_seconds;

    if is_double_click {
        *mode = CameraMode::System(system_id);
        return true;
    }

    false
}

pub fn apply_escape(mode: &mut CameraMode, escape_pressed: bool) {
    if escape_pressed {
        *mode = CameraMode::Galaxy;
    }
}

pub fn escape_to_galaxy_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut ui_state: ResMut<CameraUiState>,
) {
    apply_escape(&mut ui_state.mode, keys.just_pressed(KeyCode::Escape));
}

pub fn clamp_zoom(current_zoom: f32, delta: f32, min_zoom: f32, max_zoom: f32) -> f32 {
    (current_zoom - delta * 0.15).clamp(min_zoom, max_zoom)
}

pub fn camera_mode_input_system(
    time: Res<Time>,
    buttons: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
    sim: Res<SimResource>,
    mut ui_state: ResMut<CameraUiState>,
    mut tracker: Local<ClickTracker>,
) {
    if !buttons.just_pressed(MouseButton::Left) {
        return;
    }
    if ui_state.mode != CameraMode::Galaxy {
        return;
    }

    let Ok(window) = windows.single() else {
        return;
    };
    let Some(cursor_position) = window.cursor_position() else {
        return;
    };

    let Ok((camera, camera_transform)) = camera_query.single() else {
        return;
    };
    let Ok(world_position) = camera.viewport_to_world_2d(camera_transform, cursor_position) else {
        return;
    };

    let topology = sim.simulation.camera_topology_view();
    if let Some(system_id) = pick_system(&topology, world_position) {
        let double_clicked = apply_system_click(
            &mut ui_state.mode,
            &mut tracker,
            system_id,
            time.elapsed_secs_f64(),
        );
        if double_clicked {
            ui_state.zoom_level = 0.8;
        }
    }
}

pub fn station_select_input_system(
    buttons: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
    sim: Res<SimResource>,
    ui_state: Res<CameraUiState>,
    mut selected_station: ResMut<SelectedStation>,
    mut station_ui: ResMut<StationUiState>,
) {
    let left_click = buttons.just_pressed(MouseButton::Left);
    let right_click = buttons.just_pressed(MouseButton::Right);
    if !(left_click || right_click) {
        return;
    }
    let CameraMode::System(system_id) = ui_state.mode else {
        return;
    };
    let Ok(window) = windows.single() else {
        return;
    };
    let Some(cursor_position) = window.cursor_position() else {
        return;
    };
    let Ok((camera, camera_transform)) = camera_query.single() else {
        return;
    };
    let Ok(world_position) = camera.viewport_to_world_2d(camera_transform, cursor_position) else {
        return;
    };
    let topology = sim.simulation.camera_topology_view();
    if let Some(station_id) = pick_station(&topology, system_id, world_position) {
        selected_station.station_id = Some(station_id);
        if right_click {
            apply_station_context_open(&mut station_ui, station_id);
        }
    } else if right_click {
        station_ui.context_menu_open = false;
        station_ui.context_station_id = None;
    }
}

pub fn apply_station_context_open(state: &mut StationUiState, station_id: StationId) {
    state.context_station_id = Some(station_id);
    state.context_menu_open = true;
}

pub fn ship_context_input_system(
    buttons: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
    sim: Res<SimResource>,
    cache: Res<ShipMotionCache>,
    ui_state: Res<CameraUiState>,
    mut ship_ui: ResMut<ShipUiState>,
) {
    if !buttons.just_pressed(MouseButton::Right) {
        return;
    }
    let CameraMode::System(_) = ui_state.mode else {
        return;
    };
    let Ok(window) = windows.single() else {
        return;
    };
    let Some(cursor_position) = window.cursor_position() else {
        return;
    };
    let Ok((camera, camera_transform)) = camera_query.single() else {
        return;
    };
    let Ok(world_position) = camera.viewport_to_world_2d(camera_transform, cursor_position) else {
        return;
    };

    let snapshot = sim.simulation.world_render_snapshot();
    if let Some(ship_id) = pick_visible_ship(&snapshot, ui_state.mode, world_position, &cache) {
        apply_ship_context_open(&mut ship_ui, ship_id);
    } else {
        ship_ui.context_menu_open = false;
        ship_ui.context_ship_id = None;
    }
}

pub fn apply_ship_context_open(state: &mut ShipUiState, ship_id: ShipId) {
    state.context_ship_id = Some(ship_id);
    state.context_menu_open = true;
}

pub fn apply_zoom_controls(
    keys: Res<ButtonInput<KeyCode>>,
    mut wheel_events: MessageReader<MouseWheel>,
    mut ui_state: ResMut<CameraUiState>,
) {
    let mut delta = 0.0_f32;

    for event in wheel_events.read() {
        delta += event.y;
    }

    if keys.pressed(KeyCode::Equal) || keys.pressed(KeyCode::NumpadAdd) {
        delta += 1.0;
    }
    if keys.pressed(KeyCode::Minus) || keys.pressed(KeyCode::NumpadSubtract) {
        delta -= 1.0;
    }

    if delta.abs() > f32::EPSILON {
        ui_state.zoom_level = clamp_zoom(
            ui_state.zoom_level,
            delta,
            ui_state.zoom_min,
            ui_state.zoom_max,
        );
    }
}

pub fn sync_camera_transform(
    sim: Res<SimResource>,
    ui_state: Res<CameraUiState>,
    mut camera_query: Query<(&mut Transform, &mut Projection), With<Camera2d>>,
) {
    let Ok((mut transform, mut projection)) = camera_query.single_mut() else {
        return;
    };

    if let Projection::Orthographic(orthographic) = &mut *projection {
        orthographic.scale = ui_state.zoom_level;
    }

    match ui_state.mode {
        CameraMode::Galaxy => {
            transform.translation.x = 0.0;
            transform.translation.y = 0.0;
        }
        CameraMode::System(system_id) => {
            let topology = sim.simulation.camera_topology_view();
            if let Some(system) = topology
                .systems
                .iter()
                .find(|system| system.system_id == system_id)
            {
                transform.translation.x = system.x as f32;
                transform.translation.y = system.y as f32;
            }
        }
    }
}

fn pick_system(topology: &CameraTopologyView, world_position: Vec2) -> Option<SystemId> {
    topology
        .systems
        .iter()
        .find(|system| {
            let dx = world_position.x - system.x as f32;
            let dy = world_position.y - system.y as f32;
            let distance_sq = dx * dx + dy * dy;
            distance_sq <= (system.radius as f32 * 0.4).powi(2)
        })
        .map(|system| system.system_id)
}

fn pick_station(
    topology: &CameraTopologyView,
    system_id: SystemId,
    world_position: Vec2,
) -> Option<StationId> {
    topology
        .systems
        .iter()
        .find(|system| system.system_id == system_id)
        .into_iter()
        .flat_map(|system| system.stations.iter())
        .find(|station| {
            let dx = world_position.x - station.x as f32;
            let dy = world_position.y - station.y as f32;
            let distance_sq = dx * dx + dy * dy;
            distance_sq <= 9.0_f32.powi(2)
        })
        .map(|station| station.station_id)
}
