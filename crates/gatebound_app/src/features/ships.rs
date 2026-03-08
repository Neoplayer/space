use bevy::prelude::*;
use gatebound_domain::ShipId;

use crate::runtime::sim::SelectedShip;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShipCardTab {
    Overview,
    Cargo,
    Modules,
    Technical,
}

#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShipUiState {
    pub context_ship_id: Option<ShipId>,
    pub card_ship_id: Option<ShipId>,
    pub context_menu_open: bool,
    pub card_open: bool,
    pub card_tab: ShipCardTab,
}

impl Default for ShipUiState {
    fn default() -> Self {
        Self {
            context_ship_id: None,
            card_ship_id: None,
            context_menu_open: false,
            card_open: false,
            card_tab: ShipCardTab::Overview,
        }
    }
}

pub fn apply_ship_context_open(state: &mut ShipUiState, ship_id: ShipId) {
    state.context_ship_id = Some(ship_id);
    state.context_menu_open = true;
}

pub fn open_ship_card(state: &mut ShipUiState, ship_id: ShipId) {
    state.card_open = true;
    state.card_ship_id = Some(ship_id);
    state.context_ship_id = Some(ship_id);
    state.card_tab = ShipCardTab::Overview;
}

pub fn open_system_ship_inspector_selection(
    selected_ship: &mut SelectedShip,
    ship_ui: &mut ShipUiState,
    ship_id: ShipId,
) {
    selected_ship.ship_id = Some(ship_id);
    open_ship_card(ship_ui, ship_id);
}

pub struct ShipsFeaturePlugin;

impl Plugin for ShipsFeaturePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ShipUiState>();
    }
}
