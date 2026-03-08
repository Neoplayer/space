#![forbid(unsafe_code)]

use bevy::prelude::*;
use bevy::window::WindowResolution;
use bevy_egui::EguiPlugin;
use gatebound_domain::RuntimeConfig;
use gatebound_sim::config::load_runtime_config;
use std::path::Path;

mod app_shell;
pub mod features;
pub mod input;
pub mod render;
pub mod runtime;
pub mod ui;

use app_shell::GateboundAppShellPlugin;

pub fn run() {
    let config = load_runtime_config(Path::new("assets/config/stage_a"))
        .unwrap_or_else(|_| RuntimeConfig::default());

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Gatebound Stage A UI Slice".to_string(),
                resolution: WindowResolution::new(1280, 720),
                fit_canvas_to_parent: true,
                ..Window::default()
            }),
            ..WindowPlugin::default()
        }))
        .add_plugins(EguiPlugin::default())
        .add_plugins(GateboundAppShellPlugin::new(
            config.clone(),
            config.galaxy.seed,
        ))
        .run();
}

#[cfg(test)]
mod app_tests;
