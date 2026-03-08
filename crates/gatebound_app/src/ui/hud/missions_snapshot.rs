use gatebound_domain::{Commodity, Mission, MissionId, MissionOffer, ShipId, StationId};
use gatebound_sim::Simulation;

use crate::features::missions::MissionModalSelection;

use super::labels::mission_status_label;
use super::snapshot::{station_ref_snapshot, StationRefSnapshot};

#[derive(Debug, Clone, PartialEq)]
pub struct MissionSummarySnapshot {
    pub summary_line: String,
    pub route_line: String,
    pub reward: f64,
    pub gate_jumps: usize,
    pub origin: StationRefSnapshot,
    pub destination: StationRefSnapshot,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MissionOfferRowSnapshot {
    pub offer_id: u64,
    pub commodity: Commodity,
    pub quantity: f64,
    pub penalty: f64,
    pub summary: MissionSummarySnapshot,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ActiveMissionRowSnapshot {
    pub mission_id: MissionId,
    pub commodity: Commodity,
    pub quantity: f64,
    pub reward: f64,
    pub penalty: f64,
    pub status_label: String,
    pub summary: MissionSummarySnapshot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissionModalKind {
    Offer,
    Active,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MissionModalSnapshot {
    pub selection: MissionModalSelection,
    pub kind: MissionModalKind,
    pub title: String,
    pub commodity: Commodity,
    pub quantity: f64,
    pub reward: f64,
    pub penalty: f64,
    pub gate_jumps: usize,
    pub summary: MissionSummarySnapshot,
    pub status_label: Option<String>,
    pub destination_storage_amount: Option<f64>,
    pub required_quantity: Option<f64>,
    pub can_accept: bool,
    pub can_complete: bool,
    pub can_cancel: bool,
    pub complete_disabled_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StationMissionsSnapshot {
    pub docked: bool,
    pub offers: Vec<MissionOfferRowSnapshot>,
}

fn build_mission_summary_snapshot(
    simulation: &Simulation,
    reward: f64,
    origin_station_id: StationId,
    destination_station_id: StationId,
    gate_jumps: usize,
) -> Option<MissionSummarySnapshot> {
    let origin = station_ref_snapshot(simulation, origin_station_id)?;
    let destination = station_ref_snapshot(simulation, destination_station_id)?;
    let route_line = format!(
        "{} / {} -> {} / {}",
        origin.station_name, origin.system_name, destination.station_name, destination.system_name
    );
    Some(MissionSummarySnapshot {
        summary_line: format!("+{reward:.1} cr • {gate_jumps} jumps • {route_line}"),
        route_line,
        reward,
        gate_jumps,
        origin,
        destination,
    })
}

fn build_offer_row_snapshot(
    simulation: &Simulation,
    offer: &MissionOffer,
) -> Option<MissionOfferRowSnapshot> {
    let summary = build_mission_summary_snapshot(
        simulation,
        offer.reward,
        offer.origin_station,
        offer.destination_station,
        offer.route_gate_ids.len(),
    )?;
    Some(MissionOfferRowSnapshot {
        offer_id: offer.id,
        commodity: offer.commodity,
        quantity: offer.quantity,
        penalty: offer.penalty,
        summary,
    })
}

pub(super) fn build_active_mission_row_snapshot(
    simulation: &Simulation,
    mission: &Mission,
) -> Option<ActiveMissionRowSnapshot> {
    let summary = build_mission_summary_snapshot(
        simulation,
        mission.reward,
        mission.origin_station,
        mission.destination_station,
        mission.route_gate_ids.len(),
    )?;
    Some(ActiveMissionRowSnapshot {
        mission_id: mission.id,
        commodity: mission.commodity,
        quantity: mission.quantity,
        reward: mission.reward,
        penalty: mission.penalty,
        status_label: mission_status_label(mission.status).to_string(),
        summary,
    })
}

pub(super) fn build_station_missions_snapshot(
    simulation: &Simulation,
    view: &gatebound_sim::StationMissionView,
) -> StationMissionsSnapshot {
    let mut offers = view
        .offers
        .iter()
        .filter_map(|row| build_offer_row_snapshot(simulation, &row.offer))
        .collect::<Vec<_>>();
    offers.sort_by(|left, right| {
        right
            .summary
            .reward
            .total_cmp(&left.summary.reward)
            .then_with(|| left.offer_id.cmp(&right.offer_id))
    });
    StationMissionsSnapshot {
        docked: view.docked,
        offers,
    }
}

pub(super) fn build_mission_modal_snapshot(
    simulation: &Simulation,
    selection: MissionModalSelection,
    missions_board: &gatebound_sim::MissionsBoardView,
    selected_ship_id: Option<ShipId>,
) -> Option<MissionModalSnapshot> {
    match selection {
        MissionModalSelection::Offer(offer_id) => {
            let offer = missions_board
                .offers
                .iter()
                .find(|offer| offer.offer.id == offer_id)?
                .offer
                .clone();
            let summary = build_mission_summary_snapshot(
                simulation,
                offer.reward,
                offer.origin_station,
                offer.destination_station,
                offer.route_gate_ids.len(),
            )?;
            Some(MissionModalSnapshot {
                selection,
                kind: MissionModalKind::Offer,
                title: format!("Mission Offer #{}", offer.id),
                commodity: offer.commodity,
                quantity: offer.quantity,
                reward: offer.reward,
                penalty: offer.penalty,
                gate_jumps: offer.route_gate_ids.len(),
                summary,
                status_label: None,
                destination_storage_amount: None,
                required_quantity: None,
                can_accept: true,
                can_complete: false,
                can_cancel: false,
                complete_disabled_reason: None,
            })
        }
        MissionModalSelection::Active(mission_id) => {
            let detail = missions_board
                .active_missions
                .iter()
                .find(|detail| detail.mission.id == mission_id)?;
            let mission = &detail.mission;
            let summary = build_mission_summary_snapshot(
                simulation,
                mission.reward,
                mission.origin_station,
                mission.destination_station,
                mission.route_gate_ids.len(),
            )?;
            let can_complete = selected_ship_id.is_some_and(|ship_id| {
                simulation.is_ship_docked_at(ship_id, mission.destination_station)
            }) && detail.destination_storage_amount + 1e-9 >= mission.quantity;
            let complete_disabled_reason = if can_complete {
                None
            } else if selected_ship_id.is_none() {
                Some("Select a player ship to complete the mission.".to_string())
            } else if !selected_ship_id.is_some_and(|ship_id| {
                simulation.is_ship_docked_at(ship_id, mission.destination_station)
            }) {
                Some("Dock the selected ship at the destination station.".to_string())
            } else {
                Some("Not enough cargo in destination storage yet.".to_string())
            };
            Some(MissionModalSnapshot {
                selection,
                kind: MissionModalKind::Active,
                title: format!("Mission #{}", mission.id.0),
                commodity: mission.commodity,
                quantity: mission.quantity,
                reward: mission.reward,
                penalty: mission.penalty,
                gate_jumps: mission.route_gate_ids.len(),
                summary,
                status_label: Some(mission_status_label(mission.status).to_string()),
                destination_storage_amount: Some(detail.destination_storage_amount),
                required_quantity: Some(mission.quantity),
                can_accept: false,
                can_complete,
                can_cancel: true,
                complete_disabled_reason,
            })
        }
    }
}
