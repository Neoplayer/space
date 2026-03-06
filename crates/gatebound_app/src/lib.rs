#![forbid(unsafe_code)]

use bevy::prelude::*;
use bevy::window::WindowResolution;
use bevy_egui::{EguiPlugin, EguiPrimaryContextPass};
use gatebound_domain::RuntimeConfig;
use gatebound_sim::{config::load_runtime_config, Simulation};
use std::path::Path;

pub mod input;
pub mod render;
pub mod runtime;
pub mod ui;

use input::camera::{
    apply_zoom_controls, camera_mode_input_system, escape_to_galaxy_system,
    station_select_input_system, sync_camera_transform, CameraUiState,
};
use render::world::{draw_world_gizmos, setup_camera, update_ship_motion_cache, ShipMotionCache};
use runtime::sim::{
    apply_time_controls, drive_simulation, handle_lease_hotkeys, handle_panel_hotkeys,
    handle_risk_hotkeys, sync_selected_station, sync_selected_system, ContractsFilterState,
    LeaseSelection, SelectedShip, SelectedStation, SelectedSystem, SimClock, SimResource,
    StationUiState, UiKpiTracker, UiPanelState,
};
use ui::hud::{draw_hud_panel, HudMessages};

pub fn run() {
    let config = load_runtime_config(Path::new("assets/config/stage_a"))
        .unwrap_or_else(|_| RuntimeConfig::default());
    let simulation = Simulation::new(config.clone(), config.galaxy.seed);

    App::new()
        .insert_resource(ClearColor(Color::srgb(0.03, 0.04, 0.06)))
        .insert_resource(SimClock::default())
        .insert_resource(SimResource::new(simulation))
        .insert_resource(CameraUiState::default())
        .insert_resource(ShipMotionCache::default())
        .insert_resource(LeaseSelection::default())
        .insert_resource(UiPanelState::default())
        .insert_resource(ContractsFilterState::default())
        .insert_resource(SelectedShip::default())
        .insert_resource(SelectedSystem::default())
        .insert_resource(SelectedStation::default())
        .insert_resource(StationUiState::default())
        .insert_resource(UiKpiTracker::default())
        .insert_resource(HudMessages::default())
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Gatebound Stage A UI Slice".to_string(),
                resolution: WindowResolution::new(1280, 720),
                ..Window::default()
            }),
            ..WindowPlugin::default()
        }))
        .add_plugins(EguiPlugin::default())
        .add_systems(Startup, setup_camera)
        .add_systems(
            Update,
            (
                apply_time_controls,
                escape_to_galaxy_system,
                camera_mode_input_system,
                station_select_input_system,
                apply_zoom_controls,
                sync_selected_system,
                sync_selected_station,
                handle_panel_hotkeys,
                handle_risk_hotkeys,
                handle_lease_hotkeys,
                drive_simulation,
                update_ship_motion_cache,
                sync_camera_transform,
                draw_world_gizmos,
            )
                .chain(),
        )
        .add_systems(EguiPrimaryContextPass, draw_hud_panel)
        .run();
}

#[cfg(test)]
mod app_tests;
