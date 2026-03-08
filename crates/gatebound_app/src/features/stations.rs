use bevy::prelude::*;
use gatebound_domain::{Commodity, StationId};

use crate::runtime::sim::{SelectedStation, UiPanelState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StationCardTab {
    Info,
    Trade,
    Storage,
    Missions,
}

#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct StationUiState {
    pub context_station_id: Option<StationId>,
    pub card_station_id: Option<StationId>,
    pub context_menu_open: bool,
    pub station_panel_open: bool,
    pub card_tab: StationCardTab,
    pub trade_commodity: Commodity,
    pub trade_quantity: f64,
    pub storage_commodity: Commodity,
    pub storage_quantity: f64,
}

impl Default for StationUiState {
    fn default() -> Self {
        Self {
            context_station_id: None,
            card_station_id: None,
            context_menu_open: false,
            station_panel_open: false,
            card_tab: StationCardTab::Info,
            trade_commodity: Commodity::Fuel,
            trade_quantity: 5.0,
            storage_commodity: Commodity::Fuel,
            storage_quantity: 5.0,
        }
    }
}

pub fn apply_station_context_open(state: &mut StationUiState, station_id: StationId) {
    state.context_station_id = Some(station_id);
    state.context_menu_open = true;
}

pub fn open_station_card(
    state: &mut StationUiState,
    station_id: StationId,
    preferred_commodity: Option<Commodity>,
) {
    state.station_panel_open = true;
    state.card_station_id = Some(station_id);
    state.context_station_id = Some(station_id);
    state.card_tab = StationCardTab::Info;
    if let Some(commodity) = preferred_commodity {
        state.trade_commodity = commodity;
        state.storage_commodity = commodity;
    }
}

pub fn open_system_station_inspector_selection(
    selected_station: &mut SelectedStation,
    panels: &mut UiPanelState,
    station_ui: &mut StationUiState,
    station_id: StationId,
    preferred_commodity: Option<Commodity>,
) {
    selected_station.station_id = Some(station_id);
    panels.station_ops = true;
    open_station_card(station_ui, station_id, preferred_commodity);
}

pub struct StationsFeaturePlugin;

impl Plugin for StationsFeaturePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<StationUiState>();
    }
}
