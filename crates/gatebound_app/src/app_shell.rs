use bevy::prelude::*;
use bevy_egui::EguiPrimaryContextPass;
use gatebound_sim::Simulation;

use crate::features::finance::FinanceFeaturePlugin;
use crate::features::markets::MarketsFeaturePlugin;
use crate::features::missions::MissionsFeaturePlugin;
use crate::features::ships::ShipsFeaturePlugin;
use crate::features::stations::StationsFeaturePlugin;
use crate::input::camera::{
    apply_zoom_controls, camera_mode_input_system, galaxy_pan_input_system,
    ship_context_input_system, station_select_input_system, sync_camera_transform, CameraUiState,
};
use crate::render::world::{
    draw_world_gizmos, setup_camera, update_ship_motion_cache, ShipMotionCache,
};
use crate::runtime::save::{save_menu_hotkey_system, SaveMenuState, SaveStorage};
use crate::runtime::sim::{
    apply_time_controls, drive_simulation, handle_panel_hotkeys, handle_risk_hotkeys,
    sync_selected_station, sync_selected_system, SelectedShip, SelectedStation, SelectedSystem,
    SimClock, SimResource, TrackedShip, UiKpiTracker, UiPanelState,
};
use crate::ui::hud::{draw_hud_panel, HudMessages};

pub(crate) fn configure_app_shell(app: &mut App, simulation: Simulation) {
    app.add_plugins((
        FinanceFeaturePlugin,
        MarketsFeaturePlugin,
        MissionsFeaturePlugin,
        ShipsFeaturePlugin,
        StationsFeaturePlugin,
    ))
    .insert_resource(ClearColor(Color::srgb(0.03, 0.04, 0.06)))
    .insert_resource(SimClock::default())
    .insert_resource(SimResource::new(simulation))
    .insert_resource(CameraUiState::default())
    .insert_resource(ShipMotionCache::default())
    .insert_resource(UiPanelState::default())
    .insert_resource(SelectedShip::default())
    .insert_resource(SelectedSystem::default())
    .insert_resource(SelectedStation::default())
    .insert_resource(TrackedShip::default())
    .insert_resource(UiKpiTracker::default())
    .insert_resource(HudMessages::default())
    .insert_resource(SaveStorage::default())
    .insert_resource(SaveMenuState::default())
    .add_systems(Startup, setup_camera)
    .add_systems(
        Update,
        (
            save_menu_hotkey_system,
            apply_time_controls,
            camera_mode_input_system,
            station_select_input_system,
            apply_zoom_controls,
            galaxy_pan_input_system,
            sync_selected_system,
            sync_selected_station,
            handle_panel_hotkeys,
            handle_risk_hotkeys,
            drive_simulation,
            update_ship_motion_cache,
            ship_context_input_system,
            sync_camera_transform,
            draw_world_gizmos,
        )
            .chain(),
    )
    .add_systems(EguiPrimaryContextPass, draw_hud_panel);
}

#[derive(Debug, Clone)]
pub(crate) struct GateboundAppShellPlugin {
    seed: u64,
    simulation_config: gatebound_domain::RuntimeConfig,
}

impl GateboundAppShellPlugin {
    pub(crate) fn new(simulation_config: gatebound_domain::RuntimeConfig, seed: u64) -> Self {
        Self {
            seed,
            simulation_config,
        }
    }
}

impl Plugin for GateboundAppShellPlugin {
    fn build(&self, app: &mut App) {
        configure_app_shell(
            app,
            Simulation::new(self.simulation_config.clone(), self.seed),
        );
    }
}
