use bevy::prelude::*;
use gatebound_domain::MissionId;

#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MissionsPanelState {
    pub selected_mission_id: Option<MissionId>,
    pub modal_selection: Option<MissionModalSelection>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissionModalSelection {
    Offer(u64),
    Active(MissionId),
}

pub fn open_mission_offer(missions_panel: &mut MissionsPanelState, offer_id: u64) {
    missions_panel.modal_selection = Some(MissionModalSelection::Offer(offer_id));
}

pub fn open_active_mission(missions_panel: &mut MissionsPanelState, mission_id: MissionId) {
    missions_panel.selected_mission_id = Some(mission_id);
    missions_panel.modal_selection = Some(MissionModalSelection::Active(mission_id));
}

pub struct MissionsFeaturePlugin;

impl Plugin for MissionsFeaturePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MissionsPanelState>();
    }
}
