#![forbid(unsafe_code)]

mod contracts;
mod economy;
mod leases;
mod lifecycle;
mod milestones;
mod movement;
mod npc;
mod queries;
mod recovery;
mod risk;
mod routing;
mod state;
#[cfg(feature = "test-support")]
mod test_api;
#[cfg(test)]
mod tests;
mod trading;

use std::collections::{BTreeMap, VecDeque};
use std::fmt::{Display, Formatter};
use std::path::Path;

use crate::views::*;
use gatebound_domain::*;

pub use lifecycle::SnapshotError;
pub use state::Simulation;
use state::ActiveModifier;

fn seed_stage_a_companies() -> BTreeMap<CompanyId, Company> {
    let mut companies = BTreeMap::new();
    companies.insert(
        CompanyId(0),
        Company {
            id: CompanyId(0),
            name: "Player Logistics".to_string(),
            archetype: CompanyArchetype::Player,
        },
    );
    companies.insert(
        CompanyId(1),
        Company {
            id: CompanyId(1),
            name: "Haulers Alpha".to_string(),
            archetype: CompanyArchetype::Hauler,
        },
    );
    companies.insert(
        CompanyId(2),
        Company {
            id: CompanyId(2),
            name: "Haulers Beta".to_string(),
            archetype: CompanyArchetype::Hauler,
        },
    );
    companies.insert(
        CompanyId(3),
        Company {
            id: CompanyId(3),
            name: "Miner Guild".to_string(),
            archetype: CompanyArchetype::Miner,
        },
    );
    companies.insert(
        CompanyId(4),
        Company {
            id: CompanyId(4),
            name: "Industrial Combine".to_string(),
            archetype: CompanyArchetype::Industrial,
        },
    );
    companies
}

fn seed_stage_a_ships(world: &World) -> BTreeMap<ShipId, Ship> {
    let mut ships = BTreeMap::new();
    if world.system_count() == 0 {
        return ships;
    }
    let sid = |idx: usize| SystemId(idx % world.system_count());
    let player_location = sid(0);
    ships.insert(
        ShipId(0),
        Ship {
            id: ShipId(0),
            company_id: CompanyId(0),
            role: ShipRole::PlayerContract,
            location: player_location,
            current_station: world.first_station(player_location),
            eta_ticks_remaining: 0,
            sub_light_speed: 18.0,
            cargo_capacity: 18.0,
            cargo: None,
            trade_order_id: None,
            movement_queue: VecDeque::new(),
            segment_eta_remaining: 0,
            segment_progress_total: 0,
            current_segment_kind: None,
            active_contract: None,
            route_cursor: 0,
            policy: AutopilotPolicy {
                waypoints: vec![sid(0), sid(1)],
                ..AutopilotPolicy::default()
            },
            planned_path: Vec::new(),
            current_target: None,
            last_gate_arrival: None,
            last_risk_score: 0.0,
            reroutes: 0,
        },
    );

    let npc_companies = [CompanyId(1), CompanyId(2), CompanyId(3), CompanyId(4)];
    for idx in 0..60 {
        let ship_id = ShipId(idx + 1);
        let company_id = npc_companies[idx % npc_companies.len()];
        let location = sid(idx);
        ships.insert(
            ship_id,
            Ship {
                id: ship_id,
                company_id,
                role: ShipRole::NpcTrade,
                location,
                current_station: world.first_station(location),
                eta_ticks_remaining: 0,
                sub_light_speed: 18.0,
                cargo_capacity: 18.0,
                cargo: None,
                trade_order_id: None,
                movement_queue: VecDeque::new(),
                segment_eta_remaining: 0,
                segment_progress_total: 0,
                current_segment_kind: None,
                active_contract: None,
                route_cursor: 0,
                policy: AutopilotPolicy {
                    max_hops: 6,
                    waypoints: Vec::new(),
                    ..AutopilotPolicy::default()
                },
                planned_path: Vec::new(),
                current_target: None,
                last_gate_arrival: None,
                last_risk_score: 0.0,
                reroutes: 0,
            },
        );
    }
    ships
}

fn base_price_for(commodity: Commodity) -> f64 {
    match commodity {
        Commodity::Ore => 8.0,
        Commodity::Ice => 6.0,
        Commodity::Gas => 7.5,
        Commodity::Metal => 14.0,
        Commodity::Fuel => 16.0,
        Commodity::Parts => 25.0,
        Commodity::Electronics => 34.0,
    }
}
