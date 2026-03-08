use bevy::prelude::*;
use gatebound_domain::{Commodity, StationId, SystemId};
use gatebound_sim::Simulation;

#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarketsUiState {
    pub detail_station_id: Option<StationId>,
    pub focused_commodity: Commodity,
    pub seeded_from_world_selection: bool,
}

impl Default for MarketsUiState {
    fn default() -> Self {
        Self {
            detail_station_id: None,
            focused_commodity: Commodity::Fuel,
            seeded_from_world_selection: false,
        }
    }
}

pub fn seed_markets_ui_state(
    state: &mut MarketsUiState,
    simulation: &Simulation,
    selected_system_id: SystemId,
    selected_station_id: Option<StationId>,
) {
    if state.seeded_from_world_selection && state.detail_station_id.is_some() {
        return;
    }

    let topology = simulation.camera_topology_view();
    let fallback_station = selected_station_id.or_else(|| {
        topology
            .systems
            .iter()
            .find(|system| system.system_id == selected_system_id)
            .and_then(|system| system.stations.first().map(|station| station.station_id))
    });

    state.detail_station_id = fallback_station;
    state.seeded_from_world_selection = true;
}

pub struct MarketsFeaturePlugin;

impl Plugin for MarketsFeaturePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MarketsUiState>();
    }
}
