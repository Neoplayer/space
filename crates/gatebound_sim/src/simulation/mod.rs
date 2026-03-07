#![forbid(unsafe_code)]

mod economy;
mod finance;
mod lifecycle;
mod milestones;
mod missions;
mod movement;
mod npc;
mod queries;
mod risk;
mod routing;
mod state;
mod storage;
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
use state::ActiveModifier;
pub use state::Simulation;

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
    companies.insert(
        CompanyId(5),
        Company {
            id: CompanyId(5),
            name: "Frontier Exchange".to_string(),
            archetype: CompanyArchetype::Hauler,
        },
    );
    companies.insert(
        CompanyId(6),
        Company {
            id: CompanyId(6),
            name: "Orbital Freight".to_string(),
            archetype: CompanyArchetype::Hauler,
        },
    );
    companies
}

fn seed_stage_a_npc_company_runtimes(
    config: &RuntimeConfig,
) -> BTreeMap<CompanyId, NpcCompanyRuntime> {
    config
        .pressure
        .npc_company_starting_balances
        .iter()
        .enumerate()
        .map(|(index, balance)| {
            let company_id = index + 1;
            (
                CompanyId(company_id),
                NpcCompanyRuntime {
                    company_id: CompanyId(company_id),
                    balance: *balance,
                    next_plan_tick: company_id as u64,
                    last_realized_profit: 0.0,
                },
            )
        })
        .collect()
}

fn stage_a_route_hop_limit(world: &World) -> usize {
    world.system_count().saturating_sub(1).max(1)
}

fn stage_a_station_systems(world: &World) -> Vec<SystemId> {
    world.systems_with_stations()
}

fn stage_a_player_cluster_systems(world: &World) -> Vec<SystemId> {
    let cluster_systems = world.systems_with_stations_in_cluster(ClusterId(0));
    if cluster_systems.is_empty() {
        stage_a_station_systems(world)
    } else {
        cluster_systems
    }
}

fn stage_a_starter_route(world: &World) -> Option<[(SystemId, StationId); 2]> {
    let mut systems = stage_a_player_cluster_systems(world);
    if systems.len() < 2 {
        systems = stage_a_station_systems(world);
    }
    let origin = *systems.first()?;
    let destination = systems
        .iter()
        .copied()
        .find(|system_id| *system_id != origin)?;
    Some([
        (origin, world.first_station(origin)?),
        (destination, world.first_station(destination)?),
    ])
}

fn stage_a_ship_metadata(
    ship_id: ShipId,
    company_id: CompanyId,
    role: ShipRole,
) -> (ShipDescriptor, Vec<ShipModule>, ShipTechnicalState) {
    let class = match (role, company_id.0) {
        (ShipRole::Player, _) => ShipClass::Courier,
        (_, 1 | 2 | 5 | 6) => ShipClass::Hauler,
        (_, 3) => ShipClass::Miner,
        _ => ShipClass::Industrial,
    };

    let registry_prefix = match class {
        ShipClass::Courier => "Swift",
        ShipClass::Hauler => "Bulk",
        ShipClass::Miner => "Drill",
        ShipClass::Industrial => "Forge",
    };
    let descriptor = ShipDescriptor {
        name: format!("{registry_prefix}-{:03}", ship_id.0),
        class,
        description: match class {
            ShipClass::Courier => {
                "Fast-response mission hull configured for dispatch runs, inspections, and operator oversight."
            }
            ShipClass::Hauler => {
                "General freight workhorse balancing cargo throughput, dock cadence, and dependable route turnover."
            }
            ShipClass::Miner => {
                "Ore-biased industrial tug optimized for dense bulk loads, extraction support, and rugged handling."
            }
            ShipClass::Industrial => {
                "Heavy utility platform tuned for refined parts, maintenance cargo, and infrastructure supply chains."
            }
        }
        .to_string(),
    };

    let wear_seed = (ship_id.0 % 5) as f64;
    let status_for = |offset: usize| match (ship_id.0 + offset) % 3 {
        0 => ShipModuleStatus::Optimal,
        1 => ShipModuleStatus::Serviceable,
        _ => ShipModuleStatus::Worn,
    };
    let modules = match class {
        ShipClass::Courier => vec![
            ShipModule {
                slot: ShipModuleSlot::Command,
                name: "Courier Flight Deck".to_string(),
                status: status_for(0),
                details:
                    "Dispatch bridge with rapid traffic clearances and mission telemetry uplinks."
                        .to_string(),
            },
            ShipModule {
                slot: ShipModuleSlot::Drive,
                name: "Sprint Torch Drive".to_string(),
                status: status_for(1),
                details: "High-response sub-light package tuned for quick orbital transfers."
                    .to_string(),
            },
            ShipModule {
                slot: ShipModuleSlot::Cargo,
                name: "Priority Cargo Spine".to_string(),
                status: status_for(2),
                details: "Compact sealed hold for high-value lots and time-sensitive consignments."
                    .to_string(),
            },
            ShipModule {
                slot: ShipModuleSlot::Utility,
                name: "Traffic Sensor Mast".to_string(),
                status: status_for(3),
                details: "Dock approach optics and route beacon sync package.".to_string(),
            },
        ],
        ShipClass::Hauler => vec![
            ShipModule {
                slot: ShipModuleSlot::Command,
                name: "Freight Bridge".to_string(),
                status: status_for(0),
                details: "Long-shift command deck optimized for recurring station loops."
                    .to_string(),
            },
            ShipModule {
                slot: ShipModuleSlot::Drive,
                name: "Load-Line Drive".to_string(),
                status: status_for(1),
                details:
                    "Stable thrust package that favors loaded acceleration and predictable docking."
                        .to_string(),
            },
            ShipModule {
                slot: ShipModuleSlot::Cargo,
                name: "Expandable Cargo Lattice".to_string(),
                status: status_for(2),
                details: "Bulk pallet frame with reinforced tie-downs for repeated freight turns."
                    .to_string(),
            },
            ShipModule {
                slot: ShipModuleSlot::Utility,
                name: "Docking Collar Array".to_string(),
                status: status_for(3),
                details: "Multi-ring docking interface for high-frequency berth changes."
                    .to_string(),
            },
        ],
        ShipClass::Miner => vec![
            ShipModule {
                slot: ShipModuleSlot::Command,
                name: "Extraction Control Pod".to_string(),
                status: status_for(0),
                details: "Hazard-aware command suite for rough industrial routing.".to_string(),
            },
            ShipModule {
                slot: ShipModuleSlot::Drive,
                name: "Torque Tug Drive".to_string(),
                status: status_for(1),
                details: "Heavy vectoring assembly built to push dense ore loads through orbit."
                    .to_string(),
            },
            ShipModule {
                slot: ShipModuleSlot::Cargo,
                name: "Ore Clamp Hold".to_string(),
                status: status_for(2),
                details: "Armored bay with dust shielding and raw-mass anchor points.".to_string(),
            },
            ShipModule {
                slot: ShipModuleSlot::Utility,
                name: "Survey Lidar Rack".to_string(),
                status: status_for(3),
                details: "Industrial-grade scan head for debris and extraction support."
                    .to_string(),
            },
        ],
        ShipClass::Industrial => vec![
            ShipModule {
                slot: ShipModuleSlot::Command,
                name: "Yard Operations Core".to_string(),
                status: status_for(0),
                details: "Command stack tuned for maintenance runs and structured logistics."
                    .to_string(),
            },
            ShipModule {
                slot: ShipModuleSlot::Drive,
                name: "Service Vector Drive".to_string(),
                status: status_for(1),
                details: "Balanced propulsion package prioritizing safe approach windows."
                    .to_string(),
            },
            ShipModule {
                slot: ShipModuleSlot::Cargo,
                name: "Parts Gantry Hold".to_string(),
                status: status_for(2),
                details: "Segmented bay for refined parts, tooling pallets, and repair kits."
                    .to_string(),
            },
            ShipModule {
                slot: ShipModuleSlot::Utility,
                name: "Maintenance Drone Rail".to_string(),
                status: status_for(3),
                details: "Automated service carriage for quick turnaround support.".to_string(),
            },
        ],
    };

    let technical_state = ShipTechnicalState {
        hull: (92.0 - wear_seed * 2.5).max(58.0),
        drive: (89.0 - wear_seed * 2.0).max(55.0),
        reactor: (90.0 - wear_seed * 1.8).max(57.0),
        sensors: (87.0 - wear_seed * 2.2).max(52.0),
        cargo_bay: (91.0 - wear_seed * 1.7).max(60.0),
        maintenance_note: match class {
            ShipClass::Courier => "Flight systems remain sharp; next service window reserved after current dispatch cycle.",
            ShipClass::Hauler => "Cargo rails show routine wear from constant dock turns; service crew marked it for standard inspection.",
            ShipClass::Miner => "Dust load is within tolerance, but extraction seals need periodic recalibration.",
            ShipClass::Industrial => "Utility frame is stable; maintenance log notes deferred gantry polishing after the next yard call.",
        }
        .to_string(),
    };

    (descriptor, modules, technical_state)
}

fn seed_stage_a_ships(world: &World) -> BTreeMap<ShipId, Ship> {
    let mut ships = BTreeMap::new();
    let spawn_systems = stage_a_station_systems(world);
    if spawn_systems.is_empty() {
        return ships;
    }
    let player_route = stage_a_starter_route(world)
        .map(|route| route.map(|(system_id, _)| system_id))
        .unwrap_or([spawn_systems[0], spawn_systems[0]]);
    let player_location = player_route[0];
    let hop_limit = stage_a_route_hop_limit(world);
    let (descriptor, modules, technical_state) =
        stage_a_ship_metadata(ShipId(0), CompanyId(0), ShipRole::Player);
    ships.insert(
        ShipId(0),
        Ship {
            id: ShipId(0),
            company_id: CompanyId(0),
            role: ShipRole::Player,
            location: player_location,
            current_station: world.first_station(player_location),
            eta_ticks_remaining: 0,
            sub_light_speed: 18.0,
            cargo_capacity: 18.0,
            cargo: CargoManifest::default(),
            trade_order_id: None,
            movement_queue: VecDeque::new(),
            segment_eta_remaining: 0,
            segment_progress_total: 0,
            current_segment_kind: None,
            route_cursor: 0,
            policy: AutopilotPolicy {
                max_hops: hop_limit,
                waypoints: vec![player_route[0], player_route[1]],
                ..AutopilotPolicy::default()
            },
            planned_path: Vec::new(),
            current_target: None,
            last_gate_arrival: None,
            last_risk_score: 0.0,
            reroutes: 0,
            descriptor,
            modules,
            technical_state,
        },
    );

    for idx in 0..60 {
        let ship_id = ShipId(idx + 1);
        let company_id = CompanyId(idx / 10 + 1);
        let location = spawn_systems[idx % spawn_systems.len()];
        let (descriptor, modules, technical_state) =
            stage_a_ship_metadata(ship_id, company_id, ShipRole::NpcTrade);
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
                cargo: CargoManifest::default(),
                trade_order_id: None,
                movement_queue: VecDeque::new(),
                segment_eta_remaining: 0,
                segment_progress_total: 0,
                current_segment_kind: None,
                route_cursor: 0,
                policy: AutopilotPolicy {
                    max_hops: hop_limit,
                    waypoints: Vec::new(),
                    ..AutopilotPolicy::default()
                },
                planned_path: Vec::new(),
                current_target: None,
                last_gate_arrival: None,
                last_risk_score: 0.0,
                reroutes: 0,
                descriptor,
                modules,
                technical_state,
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
