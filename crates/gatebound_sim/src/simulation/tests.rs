use super::*;
use std::{fs, time::Instant};

fn stage_a_config() -> RuntimeConfig {
    crate::config::load_runtime_config(Path::new("../../assets/config/stage_a"))
        .expect("stage_a config should load")
}

fn station_for_system(sim: &Simulation, system_id: SystemId) -> StationId {
    sim.world
        .first_station(system_id)
        .expect("system station should exist")
}

#[test]
fn generation_respects_cluster_and_connectivity_and_degree() {
    let cfg = stage_a_config();
    let sim = Simulation::new(cfg.clone(), cfg.galaxy.seed);
    let systems = sim.world.system_count();
    assert!(
        systems >= usize::from(cfg.galaxy.cluster_system_min)
            && systems <= usize::from(cfg.galaxy.cluster_system_max),
        "system count out of stage A bounds"
    );
    assert!(sim.world.is_connected(), "world graph must be connected");

    let degrees = sim.world.degree_map();
    for degree in degrees.values() {
        assert!(
            *degree >= usize::from(cfg.galaxy.min_degree)
                && *degree <= usize::from(cfg.galaxy.max_degree),
            "node degree outside configured bounds"
        );
    }
}

#[test]
fn gate_nodes_are_placed_on_system_boundary() {
    let sim = Simulation::new(stage_a_config(), 7);
    for system in &sim.world.systems {
        for gate in &system.gate_nodes {
            let dx = gate.x - system.x;
            let dy = gate.y - system.y;
            let distance = (dx * dx + dy * dy).sqrt();
            let eps = 1e-6;
            assert!(
                (distance - system.radius).abs() < eps,
                "gate must lie on system boundary"
            );
        }
    }
}

#[test]
fn routing_supports_multihop_and_respects_max_hops() {
    let cfg = stage_a_config();
    let mut sim = Simulation::new(cfg, 3);
    let from = SystemId(0);
    let to = SystemId(sim.world.system_count() - 1);
    let ship_id = ShipId(0);

    if let Some(ship) = sim.ships.get_mut(&ship_id) {
        ship.location = from;
        ship.policy.max_hops = 16;
    }

    let route = sim
        .route_for_ship(ship_id, to)
        .expect("route should exist across connected graph");
    assert!(!route.segments.is_empty(), "route should contain hops");

    if let Some(ship) = sim.ships.get_mut(&ship_id) {
        ship.policy.max_hops = 1;
    }
    let maybe_too_short = sim.route_for_ship(ship_id, to);
    if from != to {
        assert!(
            maybe_too_short.is_none()
                || maybe_too_short
                    .expect("value checked")
                    .segments
                    .iter()
                    .filter(|segment| segment.kind == SegmentKind::Warp)
                    .count()
                    <= 1,
            "max_hops must constrain routing"
        );
    }
}

#[test]
fn reroute_happens_when_edge_blocked() {
    let cfg = stage_a_config();
    let mut sim = Simulation::new(cfg, 9);
    if sim.world.edges.len() < 2 {
        // Skip tiny graph edge-case while keeping the test deterministic.
        return;
    }

    let ship_id = ShipId(0);
    let destination = SystemId(sim.world.system_count() - 1);
    if let Some(ship) = sim.ships.get_mut(&ship_id) {
        ship.location = SystemId(0);
        ship.policy.waypoints = vec![SystemId(0), destination];
        ship.policy.max_hops = 16;
    }

    let baseline = sim
        .route_for_ship(ship_id, destination)
        .expect("baseline route should exist");
    let blocked_edge = baseline.segments[0]
        .edge
        .expect("warp segment should have edge id");
    sim.set_edge_blocked_until(blocked_edge, sim.tick + 1_000);

    let rerouted = sim.route_for_ship(ship_id, destination);
    assert!(rerouted.is_some(), "reroute path should exist");
    assert_ne!(
        rerouted
            .expect("checked")
            .segments
            .first()
            .and_then(|s| s.edge),
        Some(blocked_edge),
        "first hop should avoid blocked edge"
    );
}

#[test]
fn delivery_penalty_curve_applies_without_hard_fail() {
    let mut cfg = stage_a_config();
    cfg.pressure.sla_penalty_curve = vec![1.0, 2.0, 3.0, 4.0];
    let mut sim = Simulation::new(cfg, 11);
    let start_capital = sim.capital;

    if let Some(contract) = sim.contracts.get_mut(&ContractId(0)) {
        contract.deadline_tick = 1;
        contract.assigned_ship = Some(ShipId(0));
        contract.destination = SystemId(sim.world.system_count() - 1);
    }
    if let Some(ship) = sim.ships.get_mut(&ShipId(0)) {
        ship.location = SystemId(0);
        ship.policy.waypoints = vec![SystemId(0)];
    }

    for _ in 0..5 {
        sim.step_tick();
    }

    let after_first_fail = sim.capital;
    assert!(
        after_first_fail < start_capital,
        "penalty should reduce capital"
    );

    // No hard run fail: simulation continues ticking.
    let tick_before = sim.tick;
    sim.step_tick();
    assert!(
        sim.tick > tick_before,
        "simulation should continue after SLA fail"
    );
}

#[test]
fn supply_contract_tracks_cycle_shortfall_and_progressive_penalty() {
    let mut cfg = stage_a_config();
    cfg.pressure.sla_penalty_curve = vec![1.0, 1.5, 2.0];
    let mut sim = Simulation::new(cfg, 13);
    let cid = sim.create_supply_contract(SystemId(0), SystemId(1), 20.0, 3);
    let initial_capital = sim.capital;

    for _ in 0..(sim.config.time.cycle_ticks * 2) {
        sim.step_tick();
    }

    let contract = sim
        .contracts
        .get(&cid)
        .expect("supply contract should exist");
    assert!(contract.missed_cycles >= 1, "supply misses must accumulate");
    assert!(
        sim.capital < initial_capital,
        "misses should apply penalties"
    );
}

#[test]
fn price_update_respects_delta_cap_and_floor_ceiling() {
    let cfg = stage_a_config();
    let mut sim = Simulation::new(cfg, 17);
    let sid = station_for_system(&sim, SystemId(0));
    let book = sim.markets.get_mut(&sid).expect("market should exist");
    let fuel = book
        .goods
        .get_mut(&Commodity::Fuel)
        .expect("fuel should exist");
    fuel.stock = 0.0;
    fuel.target_stock = 100.0;
    fuel.cycle_inflow = 0.0;
    fuel.cycle_outflow = 1000.0;
    let before = fuel.price;

    sim.update_market_prices();

    let after = sim
        .markets
        .get(&sid)
        .expect("market should exist")
        .goods
        .get(&Commodity::Fuel)
        .expect("fuel should exist")
        .price;

    let expected_max = before * (1.0 + sim.config.market.delta_cap);
    assert!(after <= expected_max + 1e-8, "delta cap must clamp rise");

    let floor = base_price_for(Commodity::Fuel) * sim.config.market.floor_mult;
    let ceil = base_price_for(Commodity::Fuel) * sim.config.market.ceiling_mult;
    assert!(
        after >= floor && after <= ceil,
        "price must stay in floor/ceiling"
    );
}

#[test]
fn fuel_shock_increases_fuel_price_index() {
    let cfg = stage_a_config();
    let mut sim = Simulation::new(cfg, 19);
    let sid = station_for_system(&sim, SystemId(0));
    let before = sim
        .markets
        .get(&sid)
        .expect("market should exist")
        .goods
        .get(&Commodity::Fuel)
        .expect("fuel should exist")
        .price;

    sim.apply_event(RiskEvent::FuelShock {
        production_factor: 0.3,
        duration_ticks: sim.config.time.cycle_ticks,
    });

    for _ in 0..sim.config.time.cycle_ticks {
        sim.step_tick();
    }

    let after = sim
        .markets
        .get(&sid)
        .expect("market should exist")
        .goods
        .get(&Commodity::Fuel)
        .expect("fuel should exist")
        .price;
    assert!(after > before, "fuel shock should push fuel price upward");
}

#[test]
fn congestion_changes_eta_and_risk() {
    let cfg = stage_a_config();
    let mut sim = Simulation::new(cfg, 23);
    if sim.world.system_count() < 2 {
        return;
    }
    let ship = ShipId(0);
    let destination = SystemId(1);
    let baseline = sim
        .route_for_ship(ship, destination)
        .expect("baseline route should exist");
    let edge = baseline.segments[0].edge.expect("must have edge");

    sim.apply_event(RiskEvent::GateCongestion {
        edge,
        capacity_factor: 0.2,
        duration_ticks: 200,
    });
    sim.step_tick();

    let after = sim
        .route_for_ship(ship, destination)
        .expect("route should still exist");

    assert!(
        after.eta_ticks >= baseline.eta_ticks,
        "congestion should not decrease eta"
    );
    assert!(
        after.risk_score >= baseline.risk_score,
        "congestion should not decrease risk"
    );
}

#[test]
fn autopilot_loop_and_policy_change_affect_route() {
    let cfg = stage_a_config();
    let mut sim = Simulation::new(cfg, 29);
    if sim.world.system_count() < 3 {
        return;
    }

    let ship_id = ShipId(0);
    let last = SystemId(sim.world.system_count() - 1);

    if let Some(ship) = sim.ships.get_mut(&ship_id) {
        ship.policy.waypoints = vec![SystemId(0), SystemId(1), last];
        ship.policy.max_hops = 16;
        ship.location = SystemId(0);
        ship.route_cursor = 0;
    }

    for _ in 0..200 {
        sim.step_tick();
    }

    let cursor_after_loop = sim
        .ships
        .get(&ship_id)
        .expect("ship should exist")
        .route_cursor;
    assert!(
        cursor_after_loop < 3,
        "loop cursor must remain in waypoint bounds"
    );

    let route_before = sim
        .route_for_ship(ship_id, last)
        .expect("route before policy change");

    if let Some(ship) = sim.ships.get_mut(&ship_id) {
        ship.policy.max_hops = 1;
    }

    let route_after = sim.route_for_ship(ship_id, last);
    assert!(
        route_after.as_ref().is_none_or(|route| {
            route
                .segments
                .iter()
                .filter(|segment| segment.kind == SegmentKind::Warp)
                .count()
                <= 1
        }),
        "policy max_hops must constrain route selection"
    );

    assert!(
        route_before
            .segments
            .iter()
            .filter(|segment| segment.kind == SegmentKind::Warp)
            .count()
            >= route_after.as_ref().map_or(0, |route| {
                route
                    .segments
                    .iter()
                    .filter(|segment| segment.kind == SegmentKind::Warp)
                    .count()
            }),
        "stricter policy should not increase route complexity"
    );
}

#[test]
fn deterministic_seed_tick_run_produces_same_hash_and_reports() {
    let cfg = stage_a_config();
    let mut a = Simulation::new(cfg.clone(), 31);
    let mut b = Simulation::new(cfg, 31);

    let mut reports_a = Vec::new();
    let mut reports_b = Vec::new();

    for _ in 0..120 {
        reports_a.push(a.step_tick());
        reports_b.push(b.step_tick());
    }

    assert_eq!(reports_a, reports_b, "tick reports should be deterministic");
    assert_eq!(
        a.snapshot_hash(),
        b.snapshot_hash(),
        "snapshot hash should match"
    );
}

#[test]
fn gate_warp_segment_has_zero_eta_and_keeps_queue_delay() {
    let sim = Simulation::new(stage_a_config(), 301);
    let origin_station = sim
        .world
        .first_station(SystemId(0))
        .expect("origin station should exist");
    let destination_station = sim
        .world
        .first_station(SystemId(1))
        .expect("destination station should exist");
    let route = sim
        .build_station_route(
            origin_station,
            destination_station,
            AutopilotPolicy::default(),
        )
        .expect("station route should exist");

    assert!(
        route
            .segments
            .iter()
            .any(|segment| segment.kind == SegmentKind::Warp && segment.eta_ticks == 0),
        "warp segments must be teleport with zero eta"
    );
    assert!(
        route
            .segments
            .iter()
            .any(|segment| segment.kind == SegmentKind::GateQueue),
        "gate queue stage must remain in route"
    );
}

#[test]
fn warp_completion_sets_last_gate_arrival() {
    let mut sim = Simulation::new(stage_a_config(), 303);
    sim.ships.retain(|id, _| *id == ShipId(0));
    let Some(edge) = sim.world.edges.first().cloned() else {
        return;
    };
    let ship_id = ShipId(0);
    if let Some(ship) = sim.ships.get_mut(&ship_id) {
        ship.location = edge.a;
        ship.movement_queue = VecDeque::from([RouteSegment {
            from: edge.a,
            to: edge.b,
            from_anchor: None,
            to_anchor: None,
            edge: Some(edge.id),
            kind: SegmentKind::Warp,
            eta_ticks: 0,
            risk: 0.0,
        }]);
        ship.segment_eta_remaining = 0;
        ship.segment_progress_total = 0;
        ship.current_segment_kind = None;
        ship.current_target = None;
        ship.last_gate_arrival = None;
    }
    sim.start_next_movement_segment(ship_id, 1.0);
    let ship = sim.ships.get(&ship_id).expect("ship should exist");
    assert_eq!(ship.location, edge.b);
    assert_eq!(ship.last_gate_arrival, Some(edge.id));
}

#[test]
fn last_gate_arrival_cleared_on_new_station_route() {
    let mut sim = Simulation::new(stage_a_config(), 305);
    sim.ships.retain(|id, _| *id == ShipId(0));
    if sim.world.system_count() < 2 {
        return;
    }
    let ship_id = ShipId(0);
    let fallback_gate = sim.world.edges.first().map(|edge| edge.id);
    if let Some(ship) = sim.ships.get_mut(&ship_id) {
        ship.active_contract = None;
        ship.location = SystemId(0);
        ship.movement_queue.clear();
        ship.segment_eta_remaining = 0;
        ship.segment_progress_total = 0;
        ship.current_segment_kind = None;
        ship.current_target = None;
        ship.last_gate_arrival = fallback_gate;
    }
    let destination_station = station_for_system(&sim, SystemId(1));

    sim.command_fly_to_station(ship_id, destination_station)
        .expect("player command must start route");

    let ship = sim.ships.get(&ship_id).expect("ship should exist");
    assert_eq!(ship.last_gate_arrival, None);
}

#[test]
fn station_route_contains_in_system_segments_between_gates_and_stations() {
    let sim = Simulation::new(stage_a_config(), 307);
    let origin_station = sim
        .world
        .first_station(SystemId(0))
        .expect("origin station should exist");
    let destination_station = sim
        .world
        .first_station(SystemId(sim.world.system_count().saturating_sub(1)))
        .expect("destination station should exist");
    let route = sim
        .build_station_route(
            origin_station,
            destination_station,
            AutopilotPolicy::default(),
        )
        .expect("station route should exist");

    assert!(
        route
            .segments
            .first()
            .is_some_and(|segment| segment.kind == SegmentKind::InSystem),
        "route must start with in-system movement from station"
    );
    assert!(
        route
            .segments
            .last()
            .is_some_and(|segment| segment.kind == SegmentKind::InSystem),
        "route must end with in-system movement to destination station"
    );
}

#[test]
fn in_system_eta_uses_distance_over_sub_light_speed() {
    let sim = Simulation::new(stage_a_config(), 311);
    let system_id = SystemId(0);
    let stations = sim
        .world
        .stations_by_system
        .get(&system_id)
        .cloned()
        .expect("stations should exist");
    if stations.len() < 2 {
        return;
    }
    let from_station = stations[0];
    let to_station = stations[1];
    let from = sim
        .world
        .stations
        .iter()
        .find(|station| station.id == from_station)
        .expect("from station exists");
    let to = sim
        .world
        .stations
        .iter()
        .find(|station| station.id == to_station)
        .expect("to station exists");
    let speed = 9.0;
    let dx = to.x - from.x;
    let dy = to.y - from.y;
    let expected = ((dx * dx + dy * dy).sqrt() / speed).ceil().max(1.0) as u32;
    let route = sim
        .build_station_route_with_speed(from_station, to_station, AutopilotPolicy::default(), speed)
        .expect("route should exist");
    let in_system = route
        .segments
        .iter()
        .find(|segment| segment.kind == SegmentKind::InSystem)
        .expect("in-system segment should exist");
    assert_eq!(in_system.eta_ticks, expected);
}

#[test]
fn multi_hop_route_follows_station_gate_gate_station_pattern() {
    let sim = Simulation::new(stage_a_config(), 313);
    if sim.world.system_count() < 3 {
        return;
    }
    let mut route_with_hops = None;
    'search: for from_idx in 0..sim.world.system_count() {
        for to_idx in 0..sim.world.system_count() {
            if from_idx == to_idx {
                continue;
            }
            let Some(route) = sim.build_station_route(
                sim.world
                    .first_station(SystemId(from_idx))
                    .expect("station exists"),
                sim.world
                    .first_station(SystemId(to_idx))
                    .expect("station exists"),
                AutopilotPolicy {
                    max_hops: 16,
                    ..AutopilotPolicy::default()
                },
            ) else {
                continue;
            };
            let warp_count = route
                .segments
                .iter()
                .filter(|segment| segment.kind == SegmentKind::Warp)
                .count();
            if warp_count >= 2 {
                route_with_hops = Some(route);
                break 'search;
            }
        }
    }
    let Some(route) = route_with_hops else {
        return;
    };
    for segment in &route.segments {
        if segment.kind == SegmentKind::Warp {
            assert_eq!(segment.eta_ticks, 0);
        }
    }
}

#[test]
fn delivery_requires_explicit_pickup_and_dropoff_actions() {
    let mut cfg = stage_a_config();
    cfg.pressure.gate_fee_per_jump = 0.0;
    let mut sim = Simulation::new(cfg, 317);
    sim.ships.retain(|id, _| *id == ShipId(0));
    let destination_system = if sim.world.system_count() > 1 {
        SystemId(1)
    } else {
        SystemId(0)
    };
    let destination_station = sim
        .world
        .stations_by_system
        .get(&destination_system)
        .and_then(|stations| stations.last().copied())
        .unwrap_or_else(|| {
            sim.world
                .first_station(destination_system)
                .unwrap_or(StationId(0))
        });
    if let Some(contract) = sim.contracts.get_mut(&ContractId(0)) {
        contract.completed = false;
        contract.failed = false;
        contract.destination = destination_system;
        contract.destination_station = destination_station;
        contract.assigned_ship = Some(ShipId(0));
        contract.deadline_tick = 10_000;
        contract.progress = ContractProgress::AwaitPickup;
        contract.loaded_amount = 0.0;
        contract.delivered_amount = 0.0;
    }
    if let Some(ship) = sim.ships.get_mut(&ShipId(0)) {
        ship.location = SystemId(0);
        ship.current_station = sim.world.first_station(SystemId(0));
        ship.active_contract = Some(ContractId(0));
        ship.policy.max_hops = 16;
    }

    for _ in 0..40 {
        sim.step_tick();
    }
    assert!(
        !sim.contracts
            .get(&ContractId(0))
            .expect("contract should exist")
            .completed,
        "delivery must not complete without explicit load/unload"
    );

    let load_amount = sim
        .contracts
        .get(&ContractId(0))
        .map(|contract| contract.quantity)
        .unwrap_or(0.0);
    sim.player_contract_load(ShipId(0), ContractId(0), load_amount)
        .expect("load should work at origin station");

    let origin_station = sim
        .ships
        .get(&ShipId(0))
        .and_then(|ship| ship.current_station)
        .expect("ship should stay at origin station after load");
    if origin_station != destination_station {
        for _ in 0..40 {
            sim.step_tick();
        }
        assert_eq!(
            sim.ships
                .get(&ShipId(0))
                .and_then(|ship| ship.current_station),
            Some(origin_station),
            "player ship must stay idle until explicit fly command"
        );
    }

    sim.command_fly_to_station(ShipId(0), destination_station)
        .expect("player flight command should start route");

    for _ in 0..200 {
        sim.step_tick();
        if sim
            .ships
            .get(&ShipId(0))
            .is_some_and(|ship| ship.current_station == Some(destination_station))
        {
            break;
        }
    }

    sim.player_contract_unload(ShipId(0), ContractId(0), load_amount)
        .expect("unload should complete contract");
    assert!(
        sim.contracts
            .get(&ContractId(0))
            .expect("contract should exist")
            .completed,
        "delivery should complete after explicit unload"
    );
}

#[test]
fn gate_fee_and_traversal_count_apply_on_teleport_segment() {
    let mut cfg = stage_a_config();
    cfg.pressure.gate_fee_per_jump = 4.0;
    let mut sim = Simulation::new(cfg, 331);
    sim.ships.retain(|id, _| *id == ShipId(0));
    if sim.world.system_count() < 2 {
        return;
    }
    if let Some(ship) = sim.ships.get_mut(&ShipId(0)) {
        ship.active_contract = None;
        ship.location = SystemId(0);
    }
    let destination_station = station_for_system(&sim, SystemId(1));
    sim.command_fly_to_station(ShipId(0), destination_station)
        .expect("player command should build route");
    let route = sim
        .route_for_ship(ShipId(0), SystemId(1))
        .expect("route should exist");
    assert!(route
        .segments
        .iter()
        .any(|segment| segment.kind == SegmentKind::Warp && segment.eta_ticks == 0));

    let capital_before = sim.capital;
    let traversal_before = sim
        .gate_traversals_cycle
        .values()
        .flat_map(|by_company| by_company.values())
        .copied()
        .sum::<u32>();
    for _ in 0..120 {
        sim.step_tick();
        let traversal_after = sim
            .gate_traversals_cycle
            .values()
            .flat_map(|by_company| by_company.values())
            .copied()
            .sum::<u32>();
        if traversal_after > traversal_before {
            break;
        }
    }
    let traversal_after = sim
        .gate_traversals_cycle
        .values()
        .flat_map(|by_company| by_company.values())
        .copied()
        .sum::<u32>();
    assert!(traversal_after > traversal_before);
    assert!(
        capital_before - sim.capital >= 4.0,
        "gate fee should be charged on warp teleport segment start"
    );
}

#[test]
fn snapshot_round_trip_restores_future_ticks() {
    let cfg = stage_a_config();
    let mut base = Simulation::new(cfg.clone(), 37);
    for _ in 0..45 {
        base.step_tick();
    }

    let tmp = std::env::temp_dir().join("gatebound_stage_a_snapshot.json");
    base.save_snapshot(&tmp).expect("snapshot save should pass");

    let mut loaded = Simulation::load_snapshot(&tmp, cfg).expect("snapshot load should pass");

    let mut base_reports = Vec::new();
    let mut loaded_reports = Vec::new();
    for _ in 0..60 {
        base_reports.push(base.step_tick());
        loaded_reports.push(loaded.step_tick());
    }

    assert_eq!(base_reports.len(), loaded_reports.len());
    for (base_report, loaded_report) in base_reports.iter().zip(loaded_reports.iter()) {
        assert_eq!(base_report.tick, loaded_report.tick);
        assert_eq!(base_report.cycle, loaded_report.cycle);
        assert_eq!(base_report.active_ships, loaded_report.active_ships);
        assert_eq!(base_report.active_contracts, loaded_report.active_contracts);
        assert!(
            (base_report.total_queue_delay as i64 - loaded_report.total_queue_delay as i64).abs()
                <= 8,
            "queue delay should remain close after snapshot reload"
        );
        assert!(
            (base_report.avg_price_index - loaded_report.avg_price_index).abs() < 1e-6,
            "price index should stay stable after snapshot reload"
        );
    }
}

#[test]
fn snapshot_round_trip_preserves_station_and_ship_segment_state() {
    let cfg = stage_a_config();
    let mut sim = Simulation::new(cfg.clone(), 337);
    sim.ships.retain(|id, _| *id == ShipId(0));
    if sim.world.system_count() < 2 {
        return;
    }
    let gate_id = sim.world.edges.first().map(|edge| edge.id);
    if let Some(ship) = sim.ships.get_mut(&ShipId(0)) {
        ship.active_contract = None;
        ship.location = SystemId(0);
        ship.current_station = sim.world.first_station(SystemId(0));
        ship.segment_eta_remaining = 0;
        ship.segment_progress_total = 0;
        ship.current_segment_kind = None;
        ship.movement_queue.clear();
        ship.last_gate_arrival = gate_id;
    }
    let destination_station = station_for_system(&sim, SystemId(1));
    sim.command_fly_to_station(ShipId(0), destination_station)
        .expect("player command should build route before snapshot");
    sim.step_tick();
    let ship_before = sim
        .ships
        .get(&ShipId(0))
        .cloned()
        .expect("ship should exist");

    let tmp = std::env::temp_dir().join("gatebound_stage_a_snapshot_station_ship.json");
    sim.save_snapshot(&tmp).expect("snapshot save should pass");
    let loaded = Simulation::load_snapshot(&tmp, cfg).expect("snapshot load should pass");
    let ship_after = loaded
        .ships
        .get(&ShipId(0))
        .expect("loaded ship should exist");

    assert_eq!(loaded.world.stations, sim.world.stations);
    assert_eq!(
        loaded.world.stations_by_system,
        sim.world.stations_by_system
    );
    assert_eq!(ship_after.movement_queue, ship_before.movement_queue);
    assert_eq!(
        ship_after.current_segment_kind,
        ship_before.current_segment_kind
    );
    assert_eq!(
        ship_after.segment_eta_remaining,
        ship_before.segment_eta_remaining
    );
    assert_eq!(ship_after.last_gate_arrival, ship_before.last_gate_arrival);
    let loaded_contract = loaded
        .contracts
        .get(&ContractId(0))
        .expect("contract should exist");
    let base_contract = sim
        .contracts
        .get(&ContractId(0))
        .expect("contract should exist");
    assert_eq!(loaded_contract.origin_station, base_contract.origin_station);
    assert_eq!(
        loaded_contract.destination_station,
        base_contract.destination_station
    );
}

#[test]
fn snapshot_round_trip_preserves_last_gate_arrival() {
    let cfg = stage_a_config();
    let mut sim = Simulation::new(cfg.clone(), 341);
    let Some(gate_id) = sim.world.edges.first().map(|edge| edge.id) else {
        return;
    };
    if let Some(ship) = sim.ships.get_mut(&ShipId(0)) {
        ship.last_gate_arrival = Some(gate_id);
    }

    let tmp = std::env::temp_dir().join("gatebound_stage_a_snapshot_last_gate_arrival.json");
    sim.save_snapshot(&tmp).expect("snapshot save should pass");
    let loaded = Simulation::load_snapshot(&tmp, cfg).expect("snapshot load should pass");
    let loaded_ship = loaded.ships.get(&ShipId(0)).expect("ship should exist");
    assert_eq!(loaded_ship.last_gate_arrival, Some(gate_id));
}

#[test]
fn stage_a_scope_guards_are_locked() {
    let cfg = stage_a_config();
    assert_eq!(cfg.time.cycle_ticks, 60, "cycle must be 60 ticks");
    assert_eq!(
        cfg.time.start_year, 3500,
        "calendar must start in year 3500"
    );
    assert_eq!(
        cfg.time.months_per_year, 12,
        "calendar must use a 12 month year"
    );

    let sim = Simulation::new(cfg, 41);
    for contract in sim.contracts.values() {
        assert!(
            matches!(
                contract.kind,
                ContractTypeStageA::Delivery | ContractTypeStageA::Supply
            ),
            "stage A must contain delivery/supply only"
        );
    }
}

#[test]
fn runtime_config_defaults_include_calendar_settings() {
    let cfg = RuntimeConfig::default();
    assert_eq!(cfg.time.start_year, 3500);
    assert_eq!(cfg.time.months_per_year, 12);
}

#[test]
fn runtime_config_rejects_zero_months_per_year() {
    let mut cfg = RuntimeConfig::default();
    cfg.time.months_per_year = 0;

    let err = cfg
        .validate()
        .expect_err("zero months_per_year must be rejected");
    assert_eq!(err.to_string(), "months_per_year must be > 0");
}

#[test]
fn market_intel_local_is_fresh_remote_is_stale() {
    let sim = Simulation::new(stage_a_config(), 43);
    let local = sim
        .market_intel(SystemId(0), true)
        .expect("local intel should be available");
    assert_eq!(local.staleness_ticks, 0);
    assert!((local.confidence - 1.0).abs() < 1e-9);

    let remote = sim
        .market_intel(SystemId(0), false)
        .expect("remote intel should be available");
    assert!(remote.staleness_ticks > 0);
    assert!(remote.confidence < 1.0);
}

#[test]
fn idle_ticks_do_not_change_capital_without_transactions() {
    let mut sim = Simulation::new(stage_a_config(), 71);
    let start_capital = sim.capital;

    if let Some(contract) = sim.contracts.get_mut(&ContractId(0)) {
        contract.assigned_ship = None;
        contract.completed = true;
    }
    if let Some(ship) = sim.ships.get_mut(&ShipId(0)) {
        ship.active_contract = None;
        ship.policy.waypoints = vec![ship.location];
    }

    for _ in 0..6_100 {
        sim.step_tick();
    }

    assert!(
        (sim.capital - start_capital).abs() < 1e-6,
        "capital should stay flat while no finance transaction occurs"
    );
}

#[test]
fn negative_capital_does_not_trigger_emergency_debt_on_cycle() {
    let mut sim = Simulation::new(stage_a_config(), 73);
    sim.capital = -30.0;
    let debt_before = sim.outstanding_debt;

    sim.step_cycle();

    assert!(
        (sim.capital + 30.0).abs() < 1e-6,
        "cycle processing should not auto-fix capital"
    );
    assert!(
        (sim.outstanding_debt - debt_before).abs() < 1e-6,
        "cycle processing should not auto-create debt"
    );
}

#[test]
fn negative_capital_does_not_reduce_reputation_or_raise_rate() {
    let mut sim = Simulation::new(stage_a_config(), 79);
    let base_rate = sim.current_loan_interest_rate;
    let base_rep = sim.reputation;

    sim.capital = -1.0;
    sim.step_cycle();

    assert!(
        (sim.current_loan_interest_rate - base_rate).abs() < 1e-9,
        "negative capital alone should not change interest rate"
    );
    assert!(
        (sim.reputation - base_rep).abs() < 1e-9,
        "negative capital alone should not change reputation"
    );
}

fn annuity_payment(principal: f64, monthly_rate: f64, months: u32) -> f64 {
    if months == 0 {
        return 0.0;
    }
    if monthly_rate.abs() < 1e-12 {
        return principal / f64::from(months);
    }
    principal * monthly_rate / (1.0 - (1.0 + monthly_rate).powf(-f64::from(months)))
}

#[test]
fn loan_offers_are_fixed_and_taking_credit_credits_capital() {
    let mut sim = Simulation::new(stage_a_config(), 83);
    let offers = sim.loan_offers();
    assert_eq!(offers.len(), 3, "three fixed loan offers expected");

    let starter = offers
        .iter()
        .find(|offer| offer.id == LoanOfferId::Starter)
        .expect("starter offer should exist");
    assert!((starter.principal - 100.0).abs() < 1e-9);
    assert!((starter.monthly_interest_rate - 0.02).abs() < 1e-9);
    assert_eq!(starter.term_months, 3);

    let start_capital = sim.capital;
    sim.take_credit(LoanOfferId::Starter)
        .expect("taking starter credit should work");

    assert!((sim.capital - (start_capital + 100.0)).abs() < 1e-9);
    assert!((sim.outstanding_debt() - 100.0).abs() < 1e-9);
    assert!((sim.current_loan_interest_rate() - 0.02).abs() < 1e-9);
    let active = sim
        .finance_panel_view()
        .active_loan
        .expect("active loan should exist after taking credit");
    assert_eq!(active.remaining_months, 3);
    assert!(
        (active.next_payment - annuity_payment(100.0, 0.02, 3)).abs() < 1e-6,
        "starter annuity should match fixed schedule"
    );
}

#[test]
fn month_end_applies_interest_and_scheduled_payment() {
    let mut sim = Simulation::new(stage_a_config(), 89);
    sim.take_credit(LoanOfferId::Starter)
        .expect("taking starter credit should work");
    let initial_payment = annuity_payment(100.0, 0.02, 3);
    let capital_before = sim.capital;

    sim.step_month();

    let expected_principal = 100.0 * 1.02 - initial_payment;
    let active = sim
        .finance_panel_view()
        .active_loan
        .expect("loan should remain active after first month");
    assert!((sim.capital - (capital_before - initial_payment)).abs() < 1e-6);
    assert!((active.principal_remaining - expected_principal).abs() < 1e-6);
    assert_eq!(active.remaining_months, 2);
    assert!(
        (active.next_payment - annuity_payment(expected_principal, 0.02, 2)).abs() < 1e-6,
        "payment should be recomputed from remaining balance"
    );
}

#[test]
fn partial_and_full_repayment_recompute_schedule_and_close_loan() {
    let mut sim = Simulation::new(stage_a_config(), 97);
    sim.take_credit(LoanOfferId::Growth)
        .expect("taking growth credit should work");

    sim.repay_credit(50.0)
        .expect("partial repayment should work");

    let active = sim
        .finance_panel_view()
        .active_loan
        .expect("loan should remain active after partial repayment");
    assert!((active.principal_remaining - 200.0).abs() < 1e-6);
    assert_eq!(active.remaining_months, 6);
    assert!(
        (active.next_payment - annuity_payment(200.0, 0.03, 6)).abs() < 1e-6,
        "partial repayment should recompute annuity"
    );

    sim.repay_credit(10_000.0)
        .expect("full repayment should allow overpay clamp");
    assert!(sim.finance_panel_view().active_loan.is_none());
    assert!((sim.outstanding_debt()).abs() < 1e-9);
}

#[test]
fn snapshot_round_trip_preserves_active_loan() {
    let cfg = stage_a_config();
    let mut sim = Simulation::new(cfg.clone(), 101);
    sim.reputation = 0.66;
    sim.take_credit(LoanOfferId::Expansion)
        .expect("taking expansion credit should work");
    sim.repay_credit(120.0)
        .expect("partial repayment should work");

    let tmp = std::env::temp_dir().join("gatebound_stage_a_snapshot_financials.json");
    sim.save_snapshot(&tmp).expect("snapshot save should pass");
    let loaded = Simulation::load_snapshot(&tmp, cfg).expect("snapshot load should pass");

    assert!((loaded.reputation - sim.reputation).abs() < 1e-9);
    assert_eq!(
        loaded.finance_panel_view().active_loan,
        sim.finance_panel_view().active_loan
    );
}

#[test]
fn snapshot_save_writes_v2_json_envelope() {
    let cfg = stage_a_config();
    let sim = Simulation::new(cfg.clone(), 97);
    let tmp = std::env::temp_dir().join("gatebound_stage_a_snapshot_v2.json");

    crate::snapshot::save_snapshot(&sim, &tmp).expect("snapshot save should pass");
    let payload = fs::read_to_string(&tmp).expect("snapshot file should exist");

    assert!(
        payload.contains("\"version\": 2"),
        "snapshot payload should use v2 envelope"
    );
    assert!(
        payload.contains("\"state\""),
        "snapshot payload should embed typed state"
    );

    let loaded = crate::snapshot::load_snapshot(&tmp, cfg).expect("snapshot load should pass");
    assert_eq!(loaded.snapshot_hash(), sim.snapshot_hash());
}

#[test]
fn legacy_snapshot_versions_are_rejected() {
    let cfg = stage_a_config();
    let tmp = std::env::temp_dir().join("gatebound_stage_a_snapshot_legacy.json");
    fs::write(&tmp, "{\"version\":3,\"state\":\"legacy\"}\n")
        .expect("legacy snapshot fixture write should pass");

    let err = crate::snapshot::load_snapshot(&tmp, cfg).expect_err("legacy payload must fail");
    assert!(
        err.to_string().contains("unsupported snapshot version"),
        "legacy versions should be rejected explicitly"
    );
}

#[test]
fn offer_generation_reflects_market_imbalance_and_risk() {
    let mut sim = Simulation::new(stage_a_config(), 101);
    sim.refresh_contract_offers();
    let baseline = sim
        .contract_offers
        .values()
        .next()
        .expect("offer must exist")
        .quantity;

    let station_id = station_for_system(&sim, SystemId(1));
    if let Some(market) = sim.markets.get_mut(&station_id) {
        for state in market.goods.values_mut() {
            state.stock = 10.0;
            state.target_stock = 200.0;
            state.cycle_outflow = 70.0;
            state.cycle_inflow = 10.0;
        }
    }
    sim.refresh_contract_offers();
    let stressed = sim
        .contract_offers
        .values()
        .next()
        .expect("offer must exist")
        .quantity;
    assert!(
        stressed >= baseline,
        "higher imbalance should increase offer size"
    );
}

#[test]
fn accept_offer_creates_contract_and_removes_offer() {
    let mut sim = Simulation::new(stage_a_config(), 103);
    if let Some(ship) = sim.ships.get_mut(&ShipId(0)) {
        ship.active_contract = None;
    }
    if let Some(contract) = sim.contracts.get_mut(&ContractId(0)) {
        contract.completed = true;
    }
    sim.refresh_contract_offers();
    let offer_id = *sim
        .contract_offers
        .keys()
        .next()
        .expect("offer must exist for acceptance");
    let cid = sim
        .accept_contract_offer(offer_id, ShipId(0))
        .expect("offer acceptance should pass");
    assert!(sim.contracts.contains_key(&cid));
    assert!(
        !sim.contract_offers.contains_key(&offer_id),
        "accepted offer should be removed"
    );
}

#[test]
fn offer_expiration_and_refresh_work_by_cycle() {
    let mut cfg = stage_a_config();
    cfg.pressure.offer_refresh_cycles = 1;
    cfg.pressure.offer_ttl_cycles = 1;
    let mut sim = Simulation::new(cfg, 107);
    sim.refresh_contract_offers();
    let first_offer_ids = sim.contract_offers.keys().copied().collect::<Vec<_>>();

    sim.step_cycle();
    sim.step_cycle();

    let has_old = first_offer_ids
        .iter()
        .any(|offer_id| sim.contract_offers.contains_key(offer_id));
    assert!(!has_old, "expired offers should be replaced on refresh");
}

#[test]
fn gate_fee_is_charged_per_warp_segment() {
    let mut cfg = stage_a_config();
    cfg.pressure.gate_fee_per_jump = 3.5;
    let mut sim = Simulation::new(cfg, 109);
    sim.ships.retain(|ship_id, _| *ship_id == ShipId(0));
    if sim.world.system_count() < 2 {
        return;
    }
    if let Some(ship) = sim.ships.get_mut(&ShipId(0)) {
        ship.active_contract = None;
        ship.location = SystemId(0);
    }
    let destination_station = station_for_system(&sim, SystemId(1));
    sim.command_fly_to_station(ShipId(0), destination_station)
        .expect("player command should start warp route");
    let before = sim.capital;
    for _ in 0..32 {
        sim.step_tick();
        if before - sim.capital >= 3.5 {
            break;
        }
    }
    assert!(
        before - sim.capital >= 3.5,
        "gate fee should be charged when warp segment starts"
    );
}

#[test]
fn market_fee_applies_to_payouts() {
    let mut cfg = stage_a_config();
    cfg.pressure.gate_fee_per_jump = 0.0;
    cfg.pressure.market_fee_rate = 0.2;
    let mut sim = Simulation::new(cfg, 113);
    sim.ships.retain(|ship_id, _| *ship_id == ShipId(0));
    let destination_station = station_for_system(&sim, SystemId(0));
    if let Some(contract) = sim.contracts.get_mut(&ContractId(0)) {
        contract.completed = false;
        contract.failed = false;
        contract.destination = SystemId(0);
        contract.destination_station = destination_station;
        contract.assigned_ship = Some(ShipId(0));
        contract.payout = 100.0;
        contract.deadline_tick = 1_000;
        contract.progress = ContractProgress::InTransit;
        contract.quantity = 10.0;
        contract.loaded_amount = 10.0;
        contract.delivered_amount = 0.0;
    }
    if let Some(ship) = sim.ships.get_mut(&ShipId(0)) {
        ship.location = SystemId(0);
        ship.current_station = Some(destination_station);
        ship.eta_ticks_remaining = 0;
        ship.segment_eta_remaining = 0;
        ship.current_segment_kind = None;
        ship.movement_queue.clear();
        ship.active_contract = Some(ContractId(0));
        ship.cargo = Some(CargoLoad {
            commodity: Commodity::Fuel,
            amount: 10.0,
            source: CargoSource::Contract {
                contract_id: ContractId(0),
            },
        });
    }

    let before = sim.capital;
    sim.player_contract_unload(ShipId(0), ContractId(0), 10.0)
        .expect("explicit unload should settle payout");
    let delta = sim.capital - before;
    assert!(
        (delta - 80.0).abs() < 1e-6,
        "payout should include market fee deduction"
    );
}

#[test]
fn market_depth_caps_effective_supply_delivery() {
    let mut cfg = stage_a_config();
    cfg.pressure.market_depth_per_cycle = 5.0;
    let mut sim = Simulation::new(cfg, 127);
    let cid = sim.create_supply_contract(SystemId(0), SystemId(1), 10.0, 3);
    if let Some(contract) = sim.contracts.get_mut(&cid) {
        contract.delivered_amount = 10.0;
        contract.per_cycle = 10.0;
        contract.payout = 40.0;
        contract.penalty = 12.0;
    }
    let before = sim.capital;
    sim.step_cycle();
    assert!(
        sim.capital < before,
        "depth cap should turn apparent full delivery into shortfall penalty"
    );
}

#[test]
fn explicit_supply_unload_drives_cycle_payout() {
    let mut cfg = stage_a_config();
    cfg.pressure.gate_fee_per_jump = 0.0;
    cfg.pressure.market_fee_rate = 0.0;
    let mut sim = Simulation::new(cfg, 129);
    sim.ships.retain(|ship_id, _| *ship_id == ShipId(0));
    let cid = sim.create_supply_contract(SystemId(0), SystemId(1), 5.0, 3);
    let destination_station = station_for_system(&sim, SystemId(1));
    if let Some(contract) = sim.contracts.get_mut(&cid) {
        contract.assigned_ship = Some(ShipId(0));
        contract.progress = ContractProgress::InTransit;
        contract.payout = 40.0;
        contract.penalty = 10.0;
        contract.loaded_amount = 5.0;
        contract.delivered_cycle_amount = 0.0;
    }
    if let Some(ship) = sim.ships.get_mut(&ShipId(0)) {
        ship.current_station = Some(destination_station);
        ship.location = SystemId(1);
        ship.eta_ticks_remaining = 0;
        ship.segment_eta_remaining = 0;
        ship.current_segment_kind = None;
        ship.movement_queue.clear();
        ship.active_contract = Some(cid);
        ship.cargo = Some(CargoLoad {
            commodity: Commodity::Fuel,
            amount: 5.0,
            source: CargoSource::Contract { contract_id: cid },
        });
    }

    sim.player_contract_unload(ShipId(0), cid, 5.0)
        .expect("supply unload should succeed");
    let before = sim.capital;
    sim.step_cycle();
    assert!(
        sim.capital > before,
        "cycle payout should require explicit unload contribution"
    );
}

#[test]
fn player_trade_enforces_docked_capacity_and_contract_lock() {
    let mut cfg = stage_a_config();
    cfg.pressure.market_fee_rate = 0.1;
    let mut sim = Simulation::new(cfg, 133);
    sim.ships.retain(|ship_id, _| *ship_id == ShipId(0));
    let station_id = station_for_system(&sim, SystemId(0));
    let other_station = station_for_system(&sim, SystemId(1));
    let fuel_price = sim
        .markets
        .get(&station_id)
        .and_then(|book| book.goods.get(&Commodity::Fuel))
        .map(|state| state.price)
        .unwrap_or(0.0);
    if let Some(ship) = sim.ships.get_mut(&ShipId(0)) {
        ship.current_station = Some(station_id);
        ship.location = SystemId(0);
        ship.eta_ticks_remaining = 0;
        ship.segment_eta_remaining = 0;
        ship.current_segment_kind = None;
        ship.movement_queue.clear();
        ship.active_contract = None;
    }

    assert_eq!(
        sim.player_buy(ShipId(0), other_station, Commodity::Fuel, 1.0),
        Err(TradeError::NotDocked)
    );

    let before_buy = sim.capital;
    let buy = sim
        .player_buy(ShipId(0), station_id, Commodity::Fuel, 10.0)
        .expect("buy should work");
    assert!(
        (buy.net_cash_delta + 10.0 * fuel_price * 1.1).abs() < 1e-6,
        "buy should apply fee to total cost"
    );
    assert!(sim.capital < before_buy);

    let before_sell = sim.capital;
    let sell = sim
        .player_sell(ShipId(0), station_id, Commodity::Fuel, 5.0)
        .expect("sell should work");
    assert!(
        (sell.net_cash_delta - 5.0 * fuel_price * 0.9).abs() < 1e-6,
        "sell should apply fee to proceeds"
    );
    assert!(sim.capital > before_sell);

    if let Some(ship) = sim.ships.get_mut(&ShipId(0)) {
        ship.cargo = Some(CargoLoad {
            commodity: Commodity::Fuel,
            amount: 2.0,
            source: CargoSource::Contract {
                contract_id: ContractId(99),
            },
        });
    }
    assert_eq!(
        sim.player_sell(ShipId(0), station_id, Commodity::Fuel, 1.0),
        Err(TradeError::ContractCargoLocked)
    );
}

#[test]
fn snapshot_load_normalizes_to_single_player_ship() {
    let cfg = stage_a_config();
    let mut sim = Simulation::new(cfg.clone(), 1377);
    let extra_npc_id = sim
        .ships
        .iter()
        .find(|(_, ship)| ship.company_id != CompanyId(0))
        .map(|(ship_id, _)| *ship_id)
        .expect("npc ship should exist");
    if let Some(ship) = sim.ships.get_mut(&extra_npc_id) {
        ship.company_id = CompanyId(0);
        ship.role = ShipRole::PlayerContract;
    }

    let tmp = std::env::temp_dir().join("gatebound_stage_a_snapshot_player_norm.json");
    sim.save_snapshot(&tmp).expect("snapshot save should pass");
    let loaded = Simulation::load_snapshot(&tmp, cfg).expect("snapshot load should pass");
    assert_eq!(
        loaded
            .ships
            .values()
            .filter(|ship| ship.company_id == CompanyId(0))
            .count(),
        1,
        "load must normalize to a single player ship"
    );
    assert_eq!(
        loaded
            .ships
            .values()
            .filter(|ship| ship.role == ShipRole::NpcTrade)
            .count(),
        60,
        "npc fleet size should stay stable"
    );
}

#[test]
fn npc_stage_a_baseline_roster_is_created() {
    let sim = Simulation::new(stage_a_config(), 131);
    assert_eq!(sim.companies.len(), 5);
    assert_eq!(
        sim.ships.len(),
        61,
        "roster must include 1 player + 60 npc cargo ships"
    );
    assert_eq!(
        sim.ships
            .values()
            .filter(|ship| ship.role == ShipRole::PlayerContract)
            .count(),
        1
    );
    assert_eq!(
        sim.ships
            .values()
            .filter(|ship| ship.role == ShipRole::NpcTrade)
            .count(),
        60
    );
    assert!(
        sim.ships
            .values()
            .filter(|ship| ship.role == ShipRole::NpcTrade)
            .all(|ship| (ship.cargo_capacity - 18.0).abs() < 1e-9),
        "npc cargo ships must use stage A capacity"
    );
    assert!(sim
        .companies
        .values()
        .any(|company| company.archetype == CompanyArchetype::Hauler));
    assert!(sim
        .companies
        .values()
        .any(|company| company.archetype == CompanyArchetype::Miner));
    assert!(sim
        .companies
        .values()
        .any(|company| company.archetype == CompanyArchetype::Industrial));
}

#[test]
fn throughput_window_computes_player_share() {
    let mut sim = Simulation::new(stage_a_config(), 137);
    let gate = sim.world.edges.first().expect("edge exists").id;
    let mut cycle_map = BTreeMap::new();
    cycle_map.insert(
        gate,
        BTreeMap::from([(CompanyId(0), 3_u32), (CompanyId(1), 1_u32)]),
    );
    sim.gate_traversals_window.clear();
    sim.gate_traversals_window.push_back(cycle_map);

    let snapshot = sim
        .gate_throughput_view()
        .into_iter()
        .find(|entry| entry.gate_id == gate)
        .expect("gate throughput should exist");
    assert!((snapshot.player_share - 0.75).abs() < 1e-9);
}

#[test]
fn milestones_complete_when_targets_reached() {
    let mut cfg = stage_a_config();
    cfg.pressure.milestone_capital_target = 100.0;
    cfg.pressure.milestone_throughput_target_share = 0.2;
    cfg.pressure.milestone_reputation_target = 0.4;
    let mut sim = Simulation::new(cfg, 149);
    sim.capital = 500.0;
    sim.reputation = 0.9;
    let gate = sim.world.edges.first().expect("edge exists").id;
    sim.gate_traversals_cycle.insert(
        gate,
        BTreeMap::from([(CompanyId(0), 2_u32), (CompanyId(1), 1_u32)]),
    );
    sim.step_cycle();
    assert!(
        sim.milestones.iter().all(|milestone| milestone.completed),
        "all milestones should complete once thresholds are crossed"
    );
}

#[test]
fn market_share_milestone_completes_on_window_share() {
    let mut cfg = stage_a_config();
    cfg.pressure.milestone_market_share_target = 0.5;
    let mut sim = Simulation::new(cfg, 211);
    let gate = sim.world.edges.first().expect("edge exists").id;
    sim.gate_traversals_window.clear();
    sim.gate_traversals_window.push_back(BTreeMap::from([(
        gate,
        BTreeMap::from([(CompanyId(0), 6_u32), (CompanyId(1), 2_u32)]),
    )]));
    sim.update_milestones();
    let market_share = sim
        .milestones
        .iter()
        .find(|milestone| milestone.id == MilestoneId::MarketShare)
        .expect("market share milestone exists");
    assert!(market_share.completed);
    assert!(market_share.current >= 0.5);
}

#[test]
fn offer_generation_populates_route_gates_problem_and_profit_per_ton() {
    let mut sim = Simulation::new(stage_a_config(), 223);
    sim.refresh_contract_offers();
    let offer = sim
        .contract_offers
        .values()
        .next()
        .expect("offer should exist");
    assert!(offer.profit_per_ton.is_finite());
    assert!(offer.profit_per_ton.abs() < 1_000.0);
    assert!(matches!(
        offer.problem_tag,
        OfferProblemTag::HighRisk
            | OfferProblemTag::CongestedRoute
            | OfferProblemTag::LowMargin
            | OfferProblemTag::FuelVolatility
    ));
}

#[test]
fn premium_offer_requires_reputation_threshold() {
    let mut cfg = stage_a_config();
    cfg.pressure.premium_offer_reputation_min = 0.9;
    let mut sim = Simulation::new(cfg, 227);
    sim.reputation = 0.5;
    sim.refresh_contract_offers();
    assert!(
        sim.contract_offers.values().all(|offer| !offer.premium),
        "low reputation should suppress premium offers"
    );
    sim.reputation = 0.95;
    sim.refresh_contract_offers();
    assert!(
        sim.contract_offers.values().all(|offer| offer.premium),
        "high reputation should enable premium offers"
    );
}

#[test]
fn fleet_status_exposes_job_queue_and_kpis() {
    let mut sim = Simulation::new(stage_a_config(), 229);
    let ship_id = ShipId(0);
    sim.ship_idle_ticks_cycle.insert(ship_id, 5);
    sim.ship_delay_ticks_cycle.insert(ship_id, 12);
    sim.ship_runs_completed.insert(ship_id, 3);
    sim.ship_profit_earned.insert(ship_id, 90.0);
    if let Some(ship) = sim.ships.get_mut(&ship_id) {
        ship.planned_path = vec![SystemId(1), SystemId(2)];
        ship.active_contract = Some(ContractId(0));
    }

    let row = sim
        .fleet_status()
        .into_iter()
        .find(|row| row.ship_id == ship_id)
        .expect("ship row should exist");
    assert_eq!(row.idle_ticks_cycle, 5);
    assert!(row.avg_delay_ticks_cycle > 0.0);
    assert!(row.profit_per_run > 0.0);
    assert!(!row.job_queue.is_empty());
}

#[test]
fn market_insights_produce_trend_forecast_and_factors() {
    let mut sim = Simulation::new(stage_a_config(), 233);
    let station_id = station_for_system(&sim, SystemId(0));
    if let Some(book) = sim.markets.get_mut(&station_id) {
        if let Some(fuel) = book.goods.get_mut(&Commodity::Fuel) {
            fuel.stock = 40.0;
            fuel.target_stock = 100.0;
            fuel.cycle_outflow = 15.0;
            fuel.cycle_inflow = 5.0;
        }
    }
    sim.capture_previous_cycle_prices();
    sim.update_market_prices();
    let rows = sim.market_insights(station_id);
    assert!(!rows.is_empty());
    let fuel_row = rows
        .iter()
        .find(|row| row.commodity == Commodity::Fuel)
        .expect("fuel row should exist");
    assert!(fuel_row.forecast_next.is_finite());
    assert!(fuel_row.imbalance_factor.is_finite());
    assert!(fuel_row.congestion_factor.is_finite());
}

#[test]
fn benchmark_cluster_tick_latency_reports_percentiles() {
    let cfg = stage_a_config();
    let mut sim = Simulation::new(cfg, 47);

    let mut samples = Vec::new();
    for _ in 0..200 {
        let start = Instant::now();
        sim.step_tick();
        samples.push(start.elapsed().as_micros() as u64);
    }

    samples.sort_unstable();
    let p95_idx = ((samples.len() as f64) * 0.95).floor() as usize;
    let p99_idx = ((samples.len() as f64) * 0.99).floor() as usize;
    let p95 = samples[p95_idx.min(samples.len() - 1)];
    let p99 = samples[p99_idx.min(samples.len() - 1)];

    // We keep this generous to avoid flaky CI; this is a reporting gate.
    assert!(p95 < 200_000, "p95 tick latency should stay sane");
    assert!(p99 < 300_000, "p99 tick latency should stay sane");
}
