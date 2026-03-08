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

fn all_station_ids(sim: &Simulation) -> Vec<StationId> {
    sim.world
        .stations
        .iter()
        .map(|station| station.id)
        .collect()
}

fn station_profile(sim: &Simulation, station_id: StationId) -> StationProfile {
    sim.world
        .stations
        .iter()
        .find(|station| station.id == station_id)
        .map(|station| station.profile)
        .expect("station should exist")
}

fn baseline_population(profile: StationProfile) -> f64 {
    match profile {
        StationProfile::Civilian => 12_000.0,
        StationProfile::Industrial => 9_000.0,
        StationProfile::Research => 7_500.0,
    }
}

fn essential_commodities(profile: StationProfile) -> &'static [Commodity] {
    match profile {
        StationProfile::Civilian => {
            &[Commodity::Ice, Commodity::Fuel, Commodity::Electronics]
        }
        StationProfile::Industrial => {
            &[Commodity::Ore, Commodity::Metal, Commodity::Parts, Commodity::Fuel]
        }
        StationProfile::Research => {
            &[Commodity::Electronics, Commodity::Parts, Commodity::Fuel]
        }
    }
}

fn expected_target_stock_after_population_change(
    profile: StationProfile,
    population_multiplier: f64,
) -> f64 {
    let population = baseline_population(profile) * population_multiplier;
    let ratio = population / baseline_population(profile);
    100.0 * ratio.powf(1.15)
}

fn reset_markets_to_nominal(sim: &mut Simulation) {
    for book in sim.markets.values_mut() {
        for state in book.goods.values_mut() {
            state.stock = 100.0;
            state.target_stock = 100.0;
            state.price = state.base_price;
            state.cycle_inflow = 0.0;
            state.cycle_outflow = 0.0;
        }
    }
}

fn last_station_system(sim: &Simulation) -> SystemId {
    sim.world
        .systems_with_stations()
        .last()
        .copied()
        .expect("world should contain a station-bearing system")
}

fn route_hop_limit(sim: &Simulation) -> usize {
    sim.world.system_count().saturating_sub(1).max(1)
}

fn ordered_system_pair(a: SystemId, b: SystemId) -> (SystemId, SystemId) {
    if a.0 < b.0 {
        (a, b)
    } else {
        (b, a)
    }
}

fn cluster_centroid(world: &World, cluster_id: ClusterId) -> (f64, f64) {
    let members = world
        .clusters
        .iter()
        .find(|cluster| cluster.id == cluster_id)
        .expect("cluster should exist");
    let mut sum_x = 0.0;
    let mut sum_y = 0.0;
    for system_id in &members.system_ids {
        let system = world
            .systems
            .iter()
            .find(|system| system.id == *system_id)
            .expect("cluster member system should exist");
        sum_x += system.x;
        sum_y += system.y;
    }
    let count = members.system_ids.len() as f64;
    (sum_x / count, sum_y / count)
}

fn positions_are_contiguous_on_ring(mut positions: Vec<usize>, ring_len: usize) -> bool {
    positions.sort_unstable();
    if positions.len() <= 1 || ring_len <= 1 {
        return true;
    }

    let mut large_gaps = 0_usize;
    for idx in 0..positions.len() {
        let current = positions[idx];
        let next = positions[(idx + 1) % positions.len()];
        let gap = if idx + 1 == positions.len() {
            (next + ring_len).saturating_sub(current + 1)
        } else {
            next.saturating_sub(current + 1)
        };
        if gap > 0 {
            large_gaps += 1;
        }
    }

    large_gaps <= 1
}

fn expected_faction_cluster_counts(cfg: &GalaxyGenConfig, cluster_count: usize) -> Vec<usize> {
    let faction_count = cfg.factions.len();
    let mut counts = vec![0_usize; faction_count];
    if cluster_count <= faction_count {
        counts
            .iter_mut()
            .take(cluster_count)
            .for_each(|count| *count = 1);
        return counts;
    }

    counts.iter_mut().for_each(|count| *count = 1);
    let remaining = cluster_count - faction_count;
    let total_weight = cfg
        .factions
        .iter()
        .map(|faction| faction.cluster_weight)
        .sum::<f64>();
    let mut quotas = cfg
        .factions
        .iter()
        .enumerate()
        .map(|(index, faction)| {
            let raw = remaining as f64 * faction.cluster_weight / total_weight;
            let base = raw.floor() as usize;
            (index, base, raw - base as f64)
        })
        .collect::<Vec<_>>();
    let allocated = quotas.iter().map(|(_, base, _)| *base).sum::<usize>();
    let leftover = remaining - allocated;
    for (index, base, _) in &quotas {
        counts[*index] += *base;
    }
    quotas.sort_by(|left, right| {
        right
            .2
            .total_cmp(&left.2)
            .then_with(|| left.0.cmp(&right.0))
    });
    for (index, _, _) in quotas.into_iter().take(leftover) {
        counts[index] += 1;
    }
    counts
}

#[test]
fn generation_respects_cluster_and_connectivity_and_degree() {
    let cfg = stage_a_config();
    let sim = Simulation::new(cfg.clone(), cfg.galaxy.seed);
    let systems = sim.world.system_count();
    assert_eq!(systems, 25, "stage A world must always generate 25 systems");
    assert!(sim.world.is_connected(), "world graph must be connected");

    let degrees = sim.world.degree_map();
    for degree in degrees.values() {
        assert!(
            *degree >= usize::from(cfg.galaxy.min_degree)
                && *degree <= usize::from(cfg.galaxy.max_degree),
            "node degree outside configured bounds"
        );
    }

    for system in &sim.world.systems {
        let station_count = sim
            .world
            .stations_by_system
            .get(&system.id)
            .map(|stations| stations.len())
            .unwrap_or(0);
        assert!(
            station_count <= 4,
            "systems must generate between 0 and 4 stations"
        );
    }
}

#[test]
fn runtime_config_rejects_wrong_fixed_faction_count() {
    let mut cfg = RuntimeConfig::default();
    cfg.galaxy.factions.pop();

    let err = cfg
        .validate()
        .expect_err("exactly five config factions must be required");
    assert_eq!(
        err.to_string(),
        "galaxy factions must contain exactly 5 entries"
    );
}

#[test]
fn runtime_config_rejects_non_positive_faction_weights() {
    let mut cfg = RuntimeConfig::default();
    cfg.galaxy.factions[0].cluster_weight = 0.0;

    let err = cfg
        .validate()
        .expect_err("non-positive faction weights must be rejected");
    assert_eq!(err.to_string(), "galaxy faction weights must be > 0");
}

#[test]
fn load_runtime_config_rejects_invalid_faction_rgb_bytes() {
    let dir = std::env::temp_dir().join("gatebound_invalid_faction_rgb_config");
    fs::create_dir_all(&dir).expect("temp config dir should be created");

    for file_name in ["time_units.toml", "market.toml", "economy_pressure.toml"] {
        let source = Path::new("../../assets/config/stage_a").join(file_name);
        fs::write(
            dir.join(file_name),
            fs::read_to_string(source).expect("fixture file should load"),
        )
        .expect("fixture file should copy");
    }

    fs::write(
        dir.join("galaxy.toml"),
        r#"seed = 42
system_count = 25
cluster_size_min = 3
cluster_size_max = 5
station_count_min = 0
station_count_max = 4
inter_cluster_gate_min = 1
inter_cluster_gate_max = 3
min_degree = 2
max_degree = 4
system_radius = 100.0
base_gate_capacity = 8.0
base_gate_travel_ticks = 15

[[factions]]
name = "Aegis Collective"
color_rgb = [256, 169, 255]
cluster_weight = 1.3

[[factions]]
name = "Cinder Consortium"
color_rgb = [255, 122, 72]
cluster_weight = 1.1

[[factions]]
name = "Verdant League"
color_rgb = [108, 214, 112]
cluster_weight = 0.9

[[factions]]
name = "Helix Syndicate"
color_rgb = [198, 108, 255]
cluster_weight = 0.8

[[factions]]
name = "Solar Union"
color_rgb = [255, 214, 82]
cluster_weight = 1.4
"#,
    )
    .expect("invalid galaxy fixture should write");

    let err = crate::config::load_runtime_config(&dir)
        .expect_err("invalid RGB byte must fail runtime config loading");
    assert!(
        err.to_string().contains("failed to parse galaxy.toml"),
        "invalid RGB byte should be rejected during galaxy config parsing"
    );
}

#[test]
fn generation_reuses_fixed_factions_contiguously_and_picks_nearest_cluster_bridges() {
    let mut cfg = RuntimeConfig::default();
    cfg.galaxy.system_count = 30;
    cfg.galaxy.cluster_size_min = 3;
    cfg.galaxy.cluster_size_max = 4;
    cfg.galaxy.inter_cluster_gate_min = 1;
    cfg.galaxy.inter_cluster_gate_max = 1;

    let world = World::generate(&cfg.galaxy, 313);
    assert_eq!(
        world.factions.len(),
        5,
        "world must reuse only the fixed 5 factions"
    );
    assert!(
        world.clusters.len() > world.factions.len(),
        "test fixture must generate more clusters than factions"
    );
    let actual_cluster_counts = world.clusters.iter().fold(
        BTreeMap::<FactionId, usize>::new(),
        |mut counts, cluster| {
            *counts.entry(cluster.faction_id).or_insert(0) += 1;
            counts
        },
    );
    let expected_cluster_counts =
        expected_faction_cluster_counts(&cfg.galaxy, world.clusters.len());
    for (index, expected) in expected_cluster_counts.iter().copied().enumerate() {
        assert_eq!(
            actual_cluster_counts
                .get(&FactionId(index))
                .copied()
                .unwrap_or(0),
            expected,
            "faction cluster counts must follow deterministic weighted quotas"
        );
    }

    let mut cluster_order = world
        .clusters
        .iter()
        .map(|cluster| {
            let (x, y) = cluster_centroid(&world, cluster.id);
            (cluster.id, y.atan2(x))
        })
        .collect::<Vec<_>>();
    cluster_order.sort_by(|left, right| left.1.total_cmp(&right.1));

    let mut positions_by_faction = BTreeMap::<FactionId, Vec<usize>>::new();
    for (index, (cluster_id, _)) in cluster_order.iter().enumerate() {
        let cluster = world
            .clusters
            .iter()
            .find(|cluster| cluster.id == *cluster_id)
            .expect("cluster should exist");
        positions_by_faction
            .entry(cluster.faction_id)
            .or_default()
            .push(index);
    }

    assert!(
        positions_by_faction
            .values()
            .all(|positions| positions_are_contiguous_on_ring(
                positions.clone(),
                cluster_order.len()
            )),
        "reused faction clusters must stay geographically contiguous"
    );

    let mut bridge_pairs = BTreeMap::<(ClusterId, ClusterId), Vec<(SystemId, SystemId)>>::new();
    for edge in &world.edges {
        let a_cluster = world.systems[edge.a.0].cluster_id;
        let b_cluster = world.systems[edge.b.0].cluster_id;
        if a_cluster == b_cluster {
            continue;
        }
        let cluster_pair = if a_cluster.0 < b_cluster.0 {
            (a_cluster, b_cluster)
        } else {
            (b_cluster, a_cluster)
        };
        bridge_pairs
            .entry(cluster_pair)
            .or_default()
            .push(ordered_system_pair(edge.a, edge.b));
    }

    assert!(
        !bridge_pairs.is_empty(),
        "world must contain inter-cluster bridges"
    );
    for ((cluster_a, cluster_b), actual_pairs) in bridge_pairs {
        assert_eq!(
            actual_pairs.len(),
            1,
            "fixture should generate one bridge per cluster pair"
        );

        let systems_a = world
            .clusters
            .iter()
            .find(|cluster| cluster.id == cluster_a)
            .expect("cluster a should exist")
            .system_ids
            .clone();
        let systems_b = world
            .clusters
            .iter()
            .find(|cluster| cluster.id == cluster_b)
            .expect("cluster b should exist")
            .system_ids
            .clone();

        let mut candidates = Vec::new();
        for left in &systems_a {
            for right in &systems_b {
                let left_system = &world.systems[left.0];
                let right_system = &world.systems[right.0];
                let dx = left_system.x - right_system.x;
                let dy = left_system.y - right_system.y;
                candidates.push((ordered_system_pair(*left, *right), dx * dx + dy * dy));
            }
        }
        candidates.sort_by(|left, right| left.1.total_cmp(&right.1));

        assert_eq!(
            actual_pairs[0], candidates[0].0,
            "inter-cluster bridge must use the nearest system pair"
        );
    }
}

#[test]
fn world_views_expose_owner_faction_and_configured_faction_color() {
    let sim = Simulation::new(RuntimeConfig::default(), 515);
    let topology = sim.camera_topology_view();
    let snapshot = sim.world_render_snapshot();
    assert_eq!(sim.world.factions.len(), 5);
    for (configured, generated) in sim
        .config()
        .galaxy
        .factions
        .iter()
        .zip(sim.world.factions.iter())
    {
        assert_eq!(configured.name, generated.name);
        assert_eq!(configured.color_rgb, generated.color_rgb);
    }

    for system in &topology.systems {
        let world_system = sim
            .world
            .systems
            .iter()
            .find(|candidate| candidate.id == system.system_id)
            .expect("world system should exist");
        let faction = sim
            .world
            .factions
            .iter()
            .find(|faction| faction.id == world_system.owner_faction_id)
            .expect("faction should exist");
        assert_eq!(system.owner_faction_id, world_system.owner_faction_id);
        assert_eq!(system.faction_color_rgb, faction.color_rgb);
    }

    for system in &snapshot.systems {
        let world_system = sim
            .world
            .systems
            .iter()
            .find(|candidate| candidate.id == system.system_id)
            .expect("world system should exist");
        let faction = sim
            .world
            .factions
            .iter()
            .find(|faction| faction.id == world_system.owner_faction_id)
            .expect("faction should exist");
        assert_eq!(system.owner_faction_id, world_system.owner_faction_id);
        assert_eq!(system.faction_color_rgb, faction.color_rgb);
    }
}

#[test]
fn world_render_snapshot_omits_resource_signal_fields_from_system_view() {
    let sim = Simulation::new(stage_a_config(), 808);
    let snapshot = sim.world_render_snapshot();
    let debug = format!("{:?}", snapshot.systems);

    assert!(
        !debug.contains("dock_congestion"),
        "render snapshot should not expose dock_congestion in system view"
    );
    assert!(
        !debug.contains("fuel_stress"),
        "render snapshot should not expose fuel_stress in system view"
    );
}

#[test]
fn market_panel_view_does_not_fallback_to_global_station_for_unknown_system() {
    let sim = Simulation::new(stage_a_config(), 247);
    let panel = sim.market_panel_view(SystemId(999), None, Commodity::Fuel);
    assert!(
        panel.station_detail.is_none(),
        "unknown or stationless systems must not borrow station detail from another system"
    );
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
    let last = last_station_system(&sim);
    let hop_limit = route_hop_limit(&sim);

    if let Some(ship) = sim.ships.get_mut(&ship_id) {
        ship.policy.waypoints = vec![SystemId(0), SystemId(1), last];
        ship.policy.max_hops = hop_limit;
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

    if let Some(ship) = sim.ships.get_mut(&ship_id) {
        ship.location = SystemId(0);
        ship.current_station = sim.world.first_station(SystemId(0));
        ship.movement_queue.clear();
        ship.segment_eta_remaining = 0;
        ship.segment_progress_total = 0;
        ship.current_segment_kind = None;
        ship.current_target = None;
        ship.eta_ticks_remaining = 0;
    }

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
        .first_station(last_station_system(&sim))
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
fn gate_fee_and_traversal_count_apply_on_teleport_segment() {
    let mut cfg = stage_a_config();
    cfg.pressure.gate_fee_per_jump = 4.0;
    let mut sim = Simulation::new(cfg, 331);
    sim.ships.retain(|id, _| *id == ShipId(0));
    if sim.world.system_count() < 2 {
        return;
    }
    if let Some(ship) = sim.ships.get_mut(&ShipId(0)) {
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
        assert_eq!(base_report.active_missions, loaded_report.active_missions);
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
    assert!(
        sim.missions.is_empty(),
        "stage A should not seed accepted missions"
    );
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
fn runtime_config_rejects_wrong_npc_company_balance_count() {
    let mut cfg = RuntimeConfig::default();
    cfg.pressure.npc_company_starting_balances.pop();

    let err = cfg
        .validate()
        .expect_err("five npc company balances must be rejected");
    assert_eq!(
        err.to_string(),
        "npc_company_starting_balances must contain exactly 6 entries"
    );
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

    if let Some(ship) = sim.ships.get_mut(&ShipId(0)) {
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
fn snapshot_save_writes_v6_json_envelope() {
    let cfg = stage_a_config();
    let sim = Simulation::new(cfg.clone(), 97);
    let tmp = std::env::temp_dir().join("gatebound_stage_a_snapshot_v6.json");

    crate::snapshot::save_snapshot(&sim, &tmp).expect("snapshot save should pass");
    let payload = fs::read_to_string(&tmp).expect("snapshot file should exist");

    assert!(
        payload.contains("\"version\": 6"),
        "snapshot payload should use v6 envelope"
    );
    assert!(
        payload.contains("\"state\""),
        "snapshot payload should embed typed state"
    );

    let loaded = crate::snapshot::load_snapshot(&tmp, cfg).expect("snapshot load should pass");
    let loaded_state = loaded.snapshot_state();
    let sim_state = sim.snapshot_state();
    assert_eq!(
        serde_json::to_value(loaded_state).expect("loaded snapshot should serialize"),
        serde_json::to_value(sim_state).expect("snapshot state should serialize"),
    );
    assert_eq!(loaded.snapshot_hash(), sim.snapshot_hash());
}

#[test]
fn snapshot_payload_round_trip_matches_file_api() {
    let cfg = stage_a_config();
    let sim = Simulation::new(cfg.clone(), 98);
    let tmp = std::env::temp_dir().join("gatebound_stage_a_snapshot_payload.json");

    let payload = sim
        .snapshot_payload()
        .expect("snapshot payload serialization should pass");
    sim.save_snapshot(&tmp).expect("snapshot save should pass");
    let file_payload = fs::read_to_string(&tmp).expect("snapshot file should exist");

    assert_eq!(file_payload, format!("{payload}\n"));

    let loaded = Simulation::from_snapshot_payload(&payload, cfg)
        .expect("snapshot payload load should pass");
    assert_eq!(loaded.snapshot_hash(), sim.snapshot_hash());
}

#[test]
fn snapshot_round_trip_preserves_player_station_storage() {
    let cfg = stage_a_config();
    let mut sim = Simulation::new(cfg.clone(), 980);
    let ship_id = ShipId(0);
    let station_id = station_for_system(&sim, SystemId(0));
    if let Some(ship) = sim.ships.get_mut(&ship_id) {
        ship.location = SystemId(0);
        ship.current_station = Some(station_id);
        ship.eta_ticks_remaining = 0;
        ship.segment_eta_remaining = 0;
        ship.segment_progress_total = 0;
        ship.movement_queue.clear();
        ship.cargo_capacity = 18.0;
        ship.cargo = CargoManifest::from(CargoLoad {
            commodity: Commodity::Fuel,
            amount: 7.0,
            source: CargoSource::Spot,
        });
    }
    sim.player_unload_to_station_storage(ship_id, station_id, Commodity::Fuel, 4.0)
        .expect("station unload should succeed");

    let payload = sim
        .snapshot_payload()
        .expect("snapshot payload serialization should pass");
    assert!(
        payload.contains("\"player_station_storage\""),
        "snapshot payload should include station storage state"
    );

    let loaded = Simulation::from_snapshot_payload(&payload, cfg)
        .expect("snapshot payload load should pass");
    let storage = loaded
        .station_storage_view(ship_id, station_id)
        .expect("storage view should exist");
    let fuel_row = storage
        .rows
        .iter()
        .find(|row| row.commodity == Commodity::Fuel)
        .expect("fuel storage row should exist");
    assert!((fuel_row.stored_amount - 4.0).abs() < 1e-9);
    assert!((fuel_row.player_cargo - 3.0).abs() < 1e-9);
}

#[test]
fn legacy_snapshot_versions_are_rejected() {
    let cfg = stage_a_config();
    let tmp = std::env::temp_dir().join("gatebound_stage_a_snapshot_legacy.json");
    fs::write(&tmp, "{\"version\":2,\"state\":\"legacy\"}\n")
        .expect("legacy snapshot fixture write should pass");

    let err = crate::snapshot::load_snapshot(&tmp, cfg).expect_err("legacy payload must fail");
    assert!(
        err.to_string().contains("unsupported snapshot version"),
        "legacy versions should be rejected explicitly"
    );
}

#[test]
fn healthy_station_growth_increases_target_stock() {
    let mut sim = Simulation::new(stage_a_config(), 611);
    let station_id = station_for_system(&sim, SystemId(0));
    let profile = station_profile(&sim, station_id);
    reset_markets_to_nominal(&mut sim);

    for commodity in essential_commodities(profile) {
        let state = sim
            .markets
            .get_mut(&station_id)
            .and_then(|book| book.goods.get_mut(commodity))
            .expect("essential commodity should exist");
        state.stock = 100.0;
        state.target_stock = 100.0;
    }

    sim.step_cycle();

    let gas_state = sim
        .markets
        .get(&station_id)
        .and_then(|book| book.goods.get(&Commodity::Gas))
        .expect("gas market should exist");
    let expected = expected_target_stock_after_population_change(profile, 1.005);
    assert!(
        (gas_state.target_stock - expected).abs() < 1e-6,
        "healthy stations should grow population and raise target stock"
    );
}

#[test]
fn critical_shortage_shrinks_population_and_target_stock() {
    let mut sim = Simulation::new(stage_a_config(), 613);
    let station_id = station_for_system(&sim, SystemId(0));
    let profile = station_profile(&sim, station_id);
    reset_markets_to_nominal(&mut sim);

    for commodity in essential_commodities(profile) {
        let state = sim
            .markets
            .get_mut(&station_id)
            .and_then(|book| book.goods.get_mut(commodity))
            .expect("essential commodity should exist");
        state.stock = 0.0;
        state.target_stock = 100.0;
    }

    sim.step_cycle();

    let gas_state = sim
        .markets
        .get(&station_id)
        .and_then(|book| book.goods.get(&Commodity::Gas))
        .expect("gas market should exist");
    let expected = expected_target_stock_after_population_change(profile, 0.98);
    assert!(
        (gas_state.target_stock - expected).abs() < 1e-6,
        "critical shortages should shrink population and lower target stock"
    );
}

#[test]
fn neutral_band_shortage_holds_population_steady() {
    let mut sim = Simulation::new(stage_a_config(), 617);
    let station_id = station_for_system(&sim, SystemId(0));
    let profile = station_profile(&sim, station_id);
    reset_markets_to_nominal(&mut sim);

    let neutral_commodity = essential_commodities(profile)[0];
    let state = sim
        .markets
        .get_mut(&station_id)
        .and_then(|book| book.goods.get_mut(&neutral_commodity))
        .expect("essential commodity should exist");
    state.stock = 90.0;
    state.target_stock = 100.0;

    sim.step_cycle();

    let gas_state = sim
        .markets
        .get(&station_id)
        .and_then(|book| book.goods.get(&Commodity::Gas))
        .expect("gas market should exist");
    assert!(
        (gas_state.target_stock - 100.0).abs() < 1e-9,
        "stations in the neutral band should hold population steady"
    );
}

#[test]
fn snapshot_round_trip_preserves_population_progression() {
    let cfg = stage_a_config();
    let mut sim = Simulation::new(cfg.clone(), 619);
    let station_id = station_for_system(&sim, SystemId(0));
    let profile = station_profile(&sim, station_id);
    reset_markets_to_nominal(&mut sim);

    sim.step_cycle();

    let tmp = std::env::temp_dir().join("gatebound_stage_a_snapshot_population_progression.json");
    sim.save_snapshot(&tmp).expect("snapshot save should pass");
    let mut loaded = Simulation::load_snapshot(&tmp, cfg).expect("snapshot load should pass");
    for book in loaded.markets.values_mut() {
        for state in book.goods.values_mut() {
            state.stock = 500.0;
        }
    }

    loaded.step_cycle();
    let target_after_second_cycle = loaded
        .markets
        .get(&station_id)
        .and_then(|book| book.goods.get(&Commodity::Gas))
        .expect("gas market should exist")
        .target_stock;
    let expected = expected_target_stock_after_population_change(profile, 1.005 * 1.005);

    assert!(
        (target_after_second_cycle - expected).abs() < 1e-6,
        "population state should continue from the saved value after load"
    );
}

#[test]
fn mission_generation_uses_population_adjusted_target_stock() {
    let mut sim = Simulation::new(stage_a_config(), 631);
    let origin_station = station_for_system(&sim, SystemId(0));
    let destination_station = station_for_system(&sim, last_station_system(&sim));
    reset_markets_to_nominal(&mut sim);
    for book in sim.markets.values_mut() {
        for state in book.goods.values_mut() {
            state.stock = 500.0;
        }
    }

    for _ in 0..40 {
        sim.step_cycle();
    }

    let origin_gas = sim
        .markets
        .get_mut(&origin_station)
        .and_then(|book| book.goods.get_mut(&Commodity::Gas))
        .expect("origin gas state should exist");
    origin_gas.stock = 200.0;

    let destination_gas = sim
        .markets
        .get_mut(&destination_station)
        .and_then(|book| book.goods.get_mut(&Commodity::Gas))
        .expect("destination gas state should exist");
    destination_gas.stock = 90.0;

    sim.refresh_mission_offers();

    assert!(
        sim.mission_offers.values().any(|offer| {
            offer.origin_station == origin_station
                && offer.destination_station == destination_station
                && offer.commodity == Commodity::Gas
        }),
        "population-adjusted targets should create gas delivery demand"
    );
}

#[test]
fn npc_planner_uses_population_adjusted_target_stock() {
    let mut sim = Simulation::new(stage_a_config(), 641);
    sim.set_planner_mode(PlannerMode::HybridRecommended);
    let company_id = CompanyId(1);
    let station_id = station_for_system(&sim, SystemId(0));
    reset_markets_to_nominal(&mut sim);

    for _ in 0..12 {
        sim.step_cycle();
    }

    let gas_state = sim
        .markets
        .get_mut(&station_id)
        .and_then(|book| book.goods.get_mut(&Commodity::Gas))
        .expect("gas state should exist");
    gas_state.stock = 90.0;

    sim.plan_company_orders(company_id);

    let diagnostics = sim.planner_diagnostics();
    let gas_demand = diagnostics
        .demands
        .iter()
        .find(|demand| demand.station_id == station_id && demand.commodity == Commodity::Gas)
        .expect("gas demand should be present");
    assert!(
        gas_demand.required_amount > 10.5,
        "planner demand should expand with population-adjusted target stock"
    );
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
fn player_trade_enforces_docked_capacity_and_market_fees() {
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
}

#[test]
fn player_trade_allows_multiple_spot_commodities_in_hold() {
    let mut cfg = stage_a_config();
    cfg.pressure.market_fee_rate = 0.1;
    let mut sim = Simulation::new(cfg, 244);
    let ship_id = ShipId(0);
    let station_id = station_for_system(&sim, SystemId(0));

    if let Some(ship) = sim.ships.get_mut(&ship_id) {
        ship.location = SystemId(0);
        ship.current_station = Some(station_id);
        ship.eta_ticks_remaining = 0;
        ship.segment_eta_remaining = 0;
        ship.segment_progress_total = 0;
        ship.movement_queue.clear();
        ship.cargo_capacity = 18.0;
    }
    if let Some(book) = sim.markets.get_mut(&station_id) {
        if let Some(fuel) = book.goods.get_mut(&Commodity::Fuel) {
            fuel.stock = 100.0;
        }
        if let Some(ore) = book.goods.get_mut(&Commodity::Ore) {
            ore.stock = 100.0;
        }
    }

    sim.player_buy(ship_id, station_id, Commodity::Fuel, 6.0)
        .expect("fuel buy should work");
    sim.player_buy(ship_id, station_id, Commodity::Ore, 4.0)
        .expect("ore buy should also work");
    sim.player_sell(ship_id, station_id, Commodity::Fuel, 2.0)
        .expect("fuel sell should only reduce the matching lot");

    let trade = sim
        .station_trade_view(ship_id, station_id)
        .expect("trade view should exist");
    let fuel_row = trade
        .rows
        .iter()
        .find(|row| row.commodity == Commodity::Fuel)
        .expect("fuel row should exist");
    let ore_row = trade
        .rows
        .iter()
        .find(|row| row.commodity == Commodity::Ore)
        .expect("ore row should exist");

    assert!((fuel_row.player_cargo - 4.0).abs() < 1e-9);
    assert!((ore_row.player_cargo - 4.0).abs() < 1e-9);
    assert!((fuel_row.buy_cap - 10.0).abs() < 1e-9);
    assert!((ore_row.buy_cap - 10.0).abs() < 1e-9);
    assert!((fuel_row.sell_cap - 4.0).abs() < 1e-9);
    assert!((ore_row.sell_cap - 4.0).abs() < 1e-9);
}

#[test]
fn player_station_storage_transfers_spot_cargo_between_ship_and_local_station() {
    let mut sim = Simulation::new(stage_a_config(), 244);
    let ship_id = ShipId(0);
    let station_id = station_for_system(&sim, SystemId(0));
    if let Some(ship) = sim.ships.get_mut(&ship_id) {
        ship.location = SystemId(0);
        ship.current_station = Some(station_id);
        ship.eta_ticks_remaining = 0;
        ship.segment_eta_remaining = 0;
        ship.segment_progress_total = 0;
        ship.movement_queue.clear();
        ship.cargo_capacity = 18.0;
        ship.cargo = CargoManifest::from(CargoLoad {
            commodity: Commodity::Fuel,
            amount: 6.0,
            source: CargoSource::Spot,
        });
    }

    sim.player_unload_to_station_storage(ship_id, station_id, Commodity::Fuel, 4.0)
        .expect("station unload should work");

    let storage = sim
        .station_storage_view(ship_id, station_id)
        .expect("storage view should exist");
    let fuel_row = storage
        .rows
        .iter()
        .find(|row| row.commodity == Commodity::Fuel)
        .expect("fuel row should exist");
    assert!((fuel_row.stored_amount - 4.0).abs() < 1e-9);
    assert!((fuel_row.player_cargo - 2.0).abs() < 1e-9);
    assert!((fuel_row.load_cap - 4.0).abs() < 1e-9);
    assert!((fuel_row.unload_cap - 2.0).abs() < 1e-9);
    assert!(fuel_row.can_load);
    assert!(fuel_row.can_unload);

    sim.player_load_from_station_storage(ship_id, station_id, Commodity::Fuel, 1.5)
        .expect("station load should work");

    let storage = sim
        .station_storage_view(ship_id, station_id)
        .expect("storage view should exist");
    let fuel_row = storage
        .rows
        .iter()
        .find(|row| row.commodity == Commodity::Fuel)
        .expect("fuel row should still exist");
    assert!((fuel_row.stored_amount - 2.5).abs() < 1e-9);
    assert!((fuel_row.player_cargo - 3.5).abs() < 1e-9);
}

#[test]
fn player_station_storage_rejects_wrong_station_access() {
    let mut sim = Simulation::new(stage_a_config(), 246);
    let ship_id = ShipId(0);
    let station_id = station_for_system(&sim, SystemId(0));
    let other_station = station_for_system(&sim, SystemId(1));
    if let Some(ship) = sim.ships.get_mut(&ship_id) {
        ship.location = SystemId(0);
        ship.current_station = Some(station_id);
        ship.eta_ticks_remaining = 0;
        ship.segment_eta_remaining = 0;
        ship.segment_progress_total = 0;
        ship.movement_queue.clear();
        ship.cargo = CargoManifest::from(CargoLoad {
            commodity: Commodity::Fuel,
            amount: 6.0,
            source: CargoSource::Spot,
        });
    }

    assert_eq!(
        sim.player_unload_to_station_storage(ship_id, other_station, Commodity::Fuel, 2.0),
        Err(StorageTransferError::NotDocked)
    );
}

#[test]
fn player_station_storage_enforces_capacity_commodity_and_station_locality() {
    let mut sim = Simulation::new(stage_a_config(), 248);
    let ship_id = ShipId(0);
    let station_id = station_for_system(&sim, SystemId(0));
    let other_station = station_for_system(&sim, SystemId(1));
    if let Some(ship) = sim.ships.get_mut(&ship_id) {
        ship.location = SystemId(0);
        ship.current_station = Some(station_id);
        ship.eta_ticks_remaining = 0;
        ship.segment_eta_remaining = 0;
        ship.segment_progress_total = 0;
        ship.movement_queue.clear();
        ship.cargo_capacity = 18.0;
        ship.cargo = CargoManifest::from(CargoLoad {
            commodity: Commodity::Fuel,
            amount: 5.0,
            source: CargoSource::Spot,
        });
    }

    sim.player_unload_to_station_storage(ship_id, station_id, Commodity::Fuel, 5.0)
        .expect("initial unload should work");
    if let Some(ship) = sim.ships.get_mut(&ship_id) {
        ship.cargo = CargoManifest::from(CargoLoad {
            commodity: Commodity::Ore,
            amount: 17.0,
            source: CargoSource::Spot,
        });
    }

    sim.player_load_from_station_storage(ship_id, station_id, Commodity::Fuel, 1.0)
        .expect("loading a second commodity should work while capacity remains");

    if let Some(ship) = sim.ships.get_mut(&ship_id) {
        ship.cargo = CargoManifest::from(vec![
            CargoLoad {
                commodity: Commodity::Fuel,
                amount: 0.5,
                source: CargoSource::Spot,
            },
            CargoLoad {
                commodity: Commodity::Ore,
                amount: 17.0,
                source: CargoSource::Spot,
            },
        ]);
    }
    assert_eq!(
        sim.player_load_from_station_storage(ship_id, station_id, Commodity::Fuel, 1.0),
        Err(StorageTransferError::CargoCapacityExceeded)
    );
    assert_eq!(
        sim.player_load_from_station_storage(ship_id, other_station, Commodity::Fuel, 1.0),
        Err(StorageTransferError::NotDocked)
    );

    let other_storage = sim
        .station_storage_view(ship_id, other_station)
        .expect("other station storage view should exist");
    assert!(
        other_storage.rows.is_empty(),
        "storage should remain local to the station where cargo was unloaded"
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
        ship.role = ShipRole::Player;
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
    assert_eq!(sim.companies.len(), 7);
    assert_eq!(
        sim.ships.len(),
        61,
        "roster must include 1 player + 60 npc cargo ships"
    );
    assert_eq!(
        sim.ships
            .values()
            .filter(|ship| ship.role == ShipRole::Player)
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
    let mut npc_ships_by_company = BTreeMap::new();
    for ship in sim
        .ships
        .values()
        .filter(|ship| ship.role == ShipRole::NpcTrade)
    {
        *npc_ships_by_company
            .entry(ship.company_id)
            .or_insert(0_usize) += 1;
    }
    assert_eq!(npc_ships_by_company.len(), 6);
    assert!(npc_ships_by_company.values().all(|count| *count == 10));
    assert_eq!(
        sim.companies
            .get(&CompanyId(5))
            .expect("frontier exchange should exist")
            .name,
        "Frontier Exchange"
    );
    assert_eq!(
        sim.companies
            .get(&CompanyId(6))
            .expect("orbital freight should exist")
            .name,
        "Orbital Freight"
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
fn npc_company_runtime_starts_with_staggered_plan_ticks() {
    let sim = Simulation::new(stage_a_config(), 141);

    let runtimes = (1..=6)
        .map(|id| {
            sim.npc_company_runtimes
                .get(&CompanyId(id))
                .expect("npc company runtime should exist")
        })
        .collect::<Vec<_>>();

    assert_eq!(
        runtimes
            .iter()
            .map(|runtime| runtime.balance)
            .collect::<Vec<_>>(),
        vec![1400.0, 1100.0, 1800.0, 2600.0, 3200.0, 2200.0]
    );
    assert_eq!(
        runtimes
            .iter()
            .map(|runtime| runtime.next_plan_tick)
            .collect::<Vec<_>>(),
        vec![1, 2, 3, 4, 5, 6]
    );
}

#[test]
fn company_planner_prefers_positive_profit_over_short_eta() {
    let mut sim = Simulation::new(stage_a_config(), 143);
    let company_id = CompanyId(1);
    let ship_id = sim
        .ships
        .values()
        .find(|ship| ship.company_id == company_id && ship.role == ShipRole::NpcTrade)
        .map(|ship| ship.id)
        .expect("company ship should exist");
    sim.ships.retain(|id, _| *id == ShipId(0) || *id == ship_id);

    let ship = sim.ships.get(&ship_id).expect("ship should exist");
    let source_station = ship.current_station.expect("ship should start docked");
    let mut destinations = sim
        .world
        .stations
        .iter()
        .map(|station| station.id)
        .filter(|station_id| *station_id != source_station)
        .collect::<Vec<_>>();
    destinations.sort_by_key(|station_id| {
        sim.build_station_route_with_speed(
            source_station,
            *station_id,
            AutopilotPolicy {
                max_hops: route_hop_limit(&sim),
                ..AutopilotPolicy::default()
            },
            ship.sub_light_speed,
        )
        .expect("route should exist")
        .eta_ticks
    });

    let short_destination = destinations[0];
    let long_destination = destinations
        .iter()
        .rev()
        .copied()
        .find(|station_id| *station_id != short_destination)
        .expect("long destination should exist");

    for book in sim.markets.values_mut() {
        for state in book.goods.values_mut() {
            state.stock = 100.0;
            state.target_stock = 100.0;
            state.price = state.base_price;
            state.cycle_inflow = 0.0;
            state.cycle_outflow = 0.0;
        }
    }
    if let Some(state) = sim
        .markets
        .get_mut(&source_station)
        .and_then(|book| book.goods.get_mut(&Commodity::Fuel))
    {
        state.stock = 180.0;
        state.target_stock = 100.0;
        state.price = 100.0;
    }
    if let Some(state) = sim
        .markets
        .get_mut(&short_destination)
        .and_then(|book| book.goods.get_mut(&Commodity::Fuel))
    {
        state.stock = 5.0;
        state.target_stock = 100.0;
        state.price = 101.0;
    }
    if let Some(state) = sim
        .markets
        .get_mut(&long_destination)
        .and_then(|book| book.goods.get_mut(&Commodity::Fuel))
    {
        state.stock = 5.0;
        state.target_stock = 100.0;
        state.price = 160.0;
    }

    sim.plan_company_orders(company_id);

    let order_id = sim
        .ships
        .get(&ship_id)
        .and_then(|ship| ship.trade_order_id)
        .expect("planner should assign an order");
    let order = sim
        .trade_orders
        .get(&order_id)
        .expect("trade order should exist");
    assert_eq!(order.company_id, company_id);
    assert_eq!(order.source_station, source_station);
    assert_eq!(order.destination_station, long_destination);
}

#[test]
fn hybrid_planner_prioritizes_zero_stock_destination() {
    let mut sim = Simulation::new(stage_a_config(), 501);
    sim.set_planner_mode(PlannerMode::HybridRecommended);

    let company_id = CompanyId(1);
    let ship_id = sim
        .ships
        .values()
        .find(|ship| ship.company_id == company_id && ship.role == ShipRole::NpcTrade)
        .map(|ship| ship.id)
        .expect("company ship should exist");
    sim.ships.retain(|id, _| *id == ShipId(0) || *id == ship_id);

    let source_station = sim.ships[&ship_id]
        .current_station
        .expect("ship should start docked");
    let destinations = sim
        .world
        .stations
        .iter()
        .map(|station| station.id)
        .filter(|station_id| *station_id != source_station)
        .take(2)
        .collect::<Vec<_>>();
    assert_eq!(
        destinations.len(),
        2,
        "fixture should provide two destinations"
    );
    let zero_stock_destination = destinations[0];
    let profitable_destination = destinations[1];

    for book in sim.markets.values_mut() {
        for state in book.goods.values_mut() {
            state.stock = 100.0;
            state.target_stock = 100.0;
            state.price = state.base_price;
            state.cycle_inflow = 0.0;
            state.cycle_outflow = 0.0;
        }
    }
    if let Some(state) = sim
        .markets
        .get_mut(&source_station)
        .and_then(|book| book.goods.get_mut(&Commodity::Fuel))
    {
        state.stock = 180.0;
        state.target_stock = 100.0;
        state.price = 20.0;
    }
    if let Some(state) = sim
        .markets
        .get_mut(&zero_stock_destination)
        .and_then(|book| book.goods.get_mut(&Commodity::Fuel))
    {
        state.stock = 0.0;
        state.target_stock = 100.0;
        state.price = 55.0;
    }
    if let Some(state) = sim
        .markets
        .get_mut(&profitable_destination)
        .and_then(|book| book.goods.get_mut(&Commodity::Fuel))
    {
        state.stock = 25.0;
        state.target_stock = 100.0;
        state.price = 65.0;
    }

    sim.plan_company_orders(company_id);

    let order_id = sim.ships[&ship_id]
        .trade_order_id
        .expect("hybrid planner should assign an order");
    let order = sim
        .trade_orders
        .get(&order_id)
        .expect("trade order should exist");
    assert_eq!(order.destination_station, zero_stock_destination);
}

#[test]
fn hybrid_planner_reserves_lane_capacity_between_ships() {
    let mut sim = Simulation::new(stage_a_config(), 503);
    sim.set_planner_mode(PlannerMode::HybridRecommended);

    let company_id = CompanyId(1);
    let mut ship_ids = sim
        .ships
        .values()
        .filter(|ship| ship.company_id == company_id && ship.role == ShipRole::NpcTrade)
        .map(|ship| ship.id)
        .take(2)
        .collect::<Vec<_>>();
    ship_ids.sort_by_key(|ship_id| ship_id.0);
    assert_eq!(
        ship_ids.len(),
        2,
        "fixture should provide two company ships"
    );
    sim.ships
        .retain(|id, _| *id == ShipId(0) || ship_ids.contains(id));

    let source_station = sim.ships[&ship_ids[0]]
        .current_station
        .expect("ship should start docked");
    let destinations = sim
        .world
        .stations
        .iter()
        .map(|station| station.id)
        .filter(|station_id| *station_id != source_station)
        .take(2)
        .collect::<Vec<_>>();
    assert_eq!(
        destinations.len(),
        2,
        "fixture should provide two destinations"
    );
    let primary_destination = destinations[0];
    let secondary_destination = destinations[1];

    for ship_id in &ship_ids {
        if let Some(ship) = sim.ships.get_mut(ship_id) {
            ship.current_station = Some(source_station);
            ship.location = sim
                .world
                .stations
                .iter()
                .find(|station| station.id == source_station)
                .map(|station| station.system_id)
                .expect("source station should have a system");
        }
    }

    for book in sim.markets.values_mut() {
        for state in book.goods.values_mut() {
            state.stock = 100.0;
            state.target_stock = 100.0;
            state.price = state.base_price;
            state.cycle_inflow = 0.0;
            state.cycle_outflow = 0.0;
        }
    }
    if let Some(state) = sim
        .markets
        .get_mut(&source_station)
        .and_then(|book| book.goods.get_mut(&Commodity::Fuel))
    {
        state.stock = 170.0;
        state.target_stock = 100.0;
        state.price = 18.0;
    }
    if let Some(state) = sim
        .markets
        .get_mut(&primary_destination)
        .and_then(|book| book.goods.get_mut(&Commodity::Fuel))
    {
        state.stock = 0.0;
        state.target_stock = 18.0;
        state.price = 42.0;
    }
    if let Some(state) = sim
        .markets
        .get_mut(&secondary_destination)
        .and_then(|book| book.goods.get_mut(&Commodity::Fuel))
    {
        state.stock = 0.0;
        state.target_stock = 60.0;
        state.price = 40.0;
    }

    sim.plan_company_orders(company_id);

    let destinations = ship_ids
        .iter()
        .filter_map(|ship_id| sim.ships.get(ship_id).and_then(|ship| ship.trade_order_id))
        .filter_map(|order_id| sim.trade_orders.get(&order_id))
        .map(|order| order.destination_station)
        .collect::<Vec<_>>();

    assert_eq!(destinations.len(), 2, "both ships should be assigned");
    assert_eq!(
        destinations
            .iter()
            .filter(|station_id| **station_id == primary_destination)
            .count(),
        1,
        "only one ship should consume the small primary lane reservation"
    );
}

#[test]
fn planner_diagnostics_expose_reserved_and_unmatched_critical_demand() {
    let mut sim = Simulation::new(stage_a_config(), 505);
    sim.set_planner_mode(PlannerMode::HybridRecommended);

    let company_id = CompanyId(1);
    sim.ships.retain(|id, ship| {
        *id == ShipId(0) || (ship.company_id == company_id && ship.role == ShipRole::NpcTrade)
    });

    let source_station = sim
        .ships
        .values()
        .find(|ship| ship.company_id == company_id && ship.role == ShipRole::NpcTrade)
        .and_then(|ship| ship.current_station)
        .expect("company ship should start docked");
    let destination_station = sim
        .world
        .stations
        .iter()
        .map(|station| station.id)
        .find(|station_id| *station_id != source_station)
        .expect("fixture should provide a destination");

    for book in sim.markets.values_mut() {
        for state in book.goods.values_mut() {
            state.stock = 100.0;
            state.target_stock = 100.0;
            state.price = state.base_price;
            state.cycle_inflow = 0.0;
            state.cycle_outflow = 0.0;
        }
    }
    if let Some(state) = sim
        .markets
        .get_mut(&source_station)
        .and_then(|book| book.goods.get_mut(&Commodity::Fuel))
    {
        state.stock = 120.0;
        state.target_stock = 100.0;
        state.price = 20.0;
    }
    if let Some(state) = sim
        .markets
        .get_mut(&destination_station)
        .and_then(|book| book.goods.get_mut(&Commodity::Fuel))
    {
        state.stock = 0.0;
        state.target_stock = 140.0;
        state.price = 60.0;
    }

    sim.plan_company_orders(company_id);

    let diagnostics = sim.planner_diagnostics();
    assert!(
        diagnostics
            .demands
            .iter()
            .any(|demand| demand.station_id == destination_station && demand.is_critical),
        "critical destination should appear in planner demand diagnostics"
    );
    assert!(
        diagnostics.total_reserved_amount > 0.0,
        "hybrid planner should record at least one reservation"
    );
}

#[test]
fn economy_lab_snapshot_is_deterministic_for_same_seed_and_settings() {
    let mut left = Simulation::new(stage_a_config(), 507);
    let mut right = Simulation::new(stage_a_config(), 507);
    left.set_planner_mode(PlannerMode::HybridRecommended);
    right.set_planner_mode(PlannerMode::HybridRecommended);

    let settings = PlannerSettings {
        planning_interval_ticks: 6,
        dispatch_window_ticks: 12,
        ..PlannerSettings::default()
    };
    left.set_planner_settings(settings);
    right.set_planner_settings(settings);

    for _ in 0..90 {
        left.step_tick();
        right.step_tick();
    }

    assert_eq!(left.economy_lab_snapshot(), right.economy_lab_snapshot());
}

#[test]
fn set_npc_trade_ship_count_rebalances_lab_roster() {
    let mut sim = Simulation::new(stage_a_config(), 509);

    sim.set_npc_trade_ship_count(12);

    assert_eq!(
        sim.ships
            .values()
            .filter(|ship| ship.role == ShipRole::NpcTrade)
            .count(),
        12
    );
}

#[test]
fn npc_trade_order_applies_partial_fill_and_realized_loss() {
    let mut sim = Simulation::new(stage_a_config(), 145);
    let company_id = CompanyId(1);
    if let Some(runtime) = sim.npc_company_runtimes.get_mut(&company_id) {
        runtime.balance = 0.0;
    }
    let ship_id = sim
        .ships
        .values()
        .find(|ship| ship.company_id == company_id && ship.role == ShipRole::NpcTrade)
        .map(|ship| ship.id)
        .expect("company ship should exist");
    sim.ships.retain(|id, _| *id == ShipId(0) || *id == ship_id);

    let source_station = sim.ships[&ship_id]
        .current_station
        .expect("ship should start docked");
    let destination_station = sim
        .world
        .stations
        .iter()
        .find(|station| station.id != source_station)
        .map(|station| station.id)
        .expect("destination station should exist");
    let destination_system = sim
        .world
        .stations
        .iter()
        .find(|station| station.id == destination_station)
        .map(|station| station.system_id)
        .expect("destination system should exist");

    for book in sim.markets.values_mut() {
        for state in book.goods.values_mut() {
            state.stock = 100.0;
            state.target_stock = 100.0;
            state.price = state.base_price;
            state.cycle_inflow = 0.0;
            state.cycle_outflow = 0.0;
        }
    }
    if let Some(state) = sim
        .markets
        .get_mut(&source_station)
        .and_then(|book| book.goods.get_mut(&Commodity::Fuel))
    {
        state.stock = 6.0;
        state.target_stock = 100.0;
        state.price = 20.0;
    }
    if let Some(state) = sim
        .markets
        .get_mut(&destination_station)
        .and_then(|book| book.goods.get_mut(&Commodity::Fuel))
    {
        state.stock = 0.0;
        state.target_stock = 100.0;
        state.price = 10.0;
    }

    let order_id = TradeOrderId(9_001);
    sim.trade_orders.insert(
        order_id,
        TradeOrder {
            id: order_id,
            company_id,
            ship_id,
            commodity: Commodity::Fuel,
            amount: 15.0,
            purchased_amount: 0.0,
            cost_basis_total: 0.0,
            gate_fees_accrued: 0.0,
            source_station,
            destination_station,
            stage: TradeOrderStage::ToPickup,
        },
    );
    if let Some(ship) = sim.ships.get_mut(&ship_id) {
        ship.trade_order_id = Some(order_id);
    }

    assert!(sim.advance_npc_trade_ship(ship_id));
    let runtime = sim
        .npc_company_runtimes
        .get(&company_id)
        .expect("runtime should exist");
    let order = sim
        .trade_orders
        .get(&order_id)
        .expect("order should survive pickup");
    assert_eq!(order.stage, TradeOrderStage::ToDropoff);
    assert!((order.purchased_amount - 6.0).abs() < 1e-9);
    assert!((order.cost_basis_total - 126.0).abs() < 1e-9);
    assert!((runtime.balance + 126.0).abs() < 1e-9);

    if let Some(ship) = sim.ships.get_mut(&ship_id) {
        ship.location = destination_system;
        ship.current_station = Some(destination_station);
        ship.segment_eta_remaining = 0;
        ship.segment_progress_total = 0;
        ship.current_segment_kind = None;
        ship.current_target = None;
        ship.eta_ticks_remaining = 0;
        ship.movement_queue.clear();
    }

    assert!(sim.advance_npc_trade_ship(ship_id));
    let runtime = sim
        .npc_company_runtimes
        .get(&company_id)
        .expect("runtime should exist");
    assert!(!sim.trade_orders.contains_key(&order_id));
    assert!(runtime.balance < 0.0, "negative balances must remain valid");
    assert!((runtime.balance + 69.0).abs() < 1e-9);
    assert!((runtime.last_realized_profit + 69.0).abs() < 1e-9);
}

#[test]
fn snapshot_round_trip_preserves_npc_company_runtime_and_trade_orders() {
    let cfg = stage_a_config();
    let mut sim = Simulation::new(cfg.clone(), 147);
    let company_id = CompanyId(4);
    let ship_id = sim
        .ships
        .values()
        .find(|ship| ship.company_id == company_id && ship.role == ShipRole::NpcTrade)
        .map(|ship| ship.id)
        .expect("company ship should exist");
    let source_station = sim.ships[&ship_id]
        .current_station
        .expect("ship should start docked");
    let destination_station = sim
        .world
        .stations
        .iter()
        .find(|station| station.id != source_station)
        .map(|station| station.id)
        .expect("destination station should exist");
    let order_id = TradeOrderId(31337);

    if let Some(runtime) = sim.npc_company_runtimes.get_mut(&company_id) {
        runtime.balance = -42.5;
        runtime.next_plan_tick = 37;
        runtime.last_realized_profit = 12.25;
    }
    sim.trade_orders.insert(
        order_id,
        TradeOrder {
            id: order_id,
            company_id,
            ship_id,
            commodity: Commodity::Parts,
            amount: 8.0,
            purchased_amount: 4.0,
            cost_basis_total: 96.0,
            gate_fees_accrued: 1.2,
            source_station,
            destination_station,
            stage: TradeOrderStage::ToDropoff,
        },
    );
    if let Some(ship) = sim.ships.get_mut(&ship_id) {
        ship.trade_order_id = Some(order_id);
        ship.cargo = CargoManifest::from(CargoLoad {
            commodity: Commodity::Parts,
            amount: 4.0,
            source: CargoSource::Spot,
        });
    }

    let tmp = std::env::temp_dir().join("gatebound_stage_a_snapshot_npc_company_runtime.json");
    sim.save_snapshot(&tmp)
        .expect("snapshot save should succeed");
    let loaded = Simulation::load_snapshot(&tmp, cfg).expect("snapshot load should succeed");

    let runtime = loaded
        .npc_company_runtimes
        .get(&company_id)
        .expect("runtime should round-trip");
    let order = loaded
        .trade_orders
        .get(&order_id)
        .expect("trade order should round-trip");
    assert!((runtime.balance + 42.5).abs() < 1e-9);
    assert_eq!(runtime.next_plan_tick, 37);
    assert!((runtime.last_realized_profit - 12.25).abs() < 1e-9);
    assert_eq!(order.company_id, company_id);
    assert!((order.purchased_amount - 4.0).abs() < 1e-9);
    assert!((order.cost_basis_total - 96.0).abs() < 1e-9);
    assert!((order.gate_fees_accrued - 1.2).abs() < 1e-9);
}

#[test]
fn stage_a_ships_seed_descriptor_modules_and_technical_state() {
    let sim = Simulation::new(stage_a_config(), 151);

    let player_ship = sim.ships.get(&ShipId(0)).expect("player ship should exist");
    assert!(!player_ship.descriptor.name.is_empty());
    assert!(!player_ship.descriptor.description.is_empty());
    assert!(!player_ship.modules.is_empty());
    assert!(player_ship
        .modules
        .iter()
        .all(|module| !module.name.is_empty() && !module.details.is_empty()));
    assert!(player_ship.technical_state.hull > 0.0);
    assert!(player_ship.technical_state.drive > 0.0);
    assert!(!player_ship.technical_state.maintenance_note.is_empty());

    let npc_ship = sim
        .ships
        .values()
        .find(|ship| ship.role == ShipRole::NpcTrade)
        .expect("npc ship should exist");
    assert!(!npc_ship.descriptor.name.is_empty());
    assert!(!npc_ship.modules.is_empty());
    assert!(npc_ship.technical_state.reactor > 0.0);
    assert!(npc_ship.technical_state.sensors > 0.0);
}

#[test]
fn ship_card_view_exposes_owner_route_and_display_metadata() {
    let mut sim = Simulation::new(stage_a_config(), 157);
    let ship_id = ShipId(1);
    let destination_station = station_for_system(&sim, SystemId(1));
    let cargo = CargoLoad {
        commodity: Commodity::Parts,
        amount: 7.5,
        source: CargoSource::Spot,
    };
    if let Some(ship) = sim.ships.get_mut(&ship_id) {
        ship.current_station = Some(destination_station);
        ship.current_target = Some(SystemId(2));
        ship.eta_ticks_remaining = 17;
        ship.current_segment_kind = Some(SegmentKind::InSystem);
        ship.cargo = CargoManifest::from(cargo);
    }

    let view = sim
        .ship_card_view(ship_id)
        .expect("ship card view should exist for seeded ship");

    assert_eq!(view.ship_id, ship_id);
    assert_eq!(view.company_id, CompanyId(1));
    assert_eq!(view.owner_name, "Haulers Alpha");
    assert_eq!(view.owner_archetype, CompanyArchetype::Hauler);
    assert_eq!(view.current_station, Some(destination_station));
    assert_eq!(view.current_target, Some(SystemId(2)));
    assert_eq!(view.eta_ticks_remaining, 17);
    assert_eq!(view.current_segment_kind, Some(SegmentKind::InSystem));
    assert_eq!(view.cargo_lots, vec![cargo]);
    assert!((view.cargo_total_amount - cargo.amount).abs() < 1e-9);
    assert!(view.mission_cargo.is_empty());
    assert!(!view.description.is_empty());
    assert!(!view.modules.is_empty());
    assert!(view.technical_state.cargo_bay > 0.0);
}

#[test]
fn snapshot_round_trip_preserves_multi_cargo_manifest() {
    let cfg = stage_a_config();
    let mut sim = Simulation::new(cfg.clone(), 161);
    let ship_id = ShipId(0);
    if let Some(ship) = sim.ships.get_mut(&ship_id) {
        ship.cargo = CargoManifest::from(vec![
            CargoLoad {
                commodity: Commodity::Fuel,
                amount: 6.0,
                source: CargoSource::Spot,
            },
            CargoLoad {
                commodity: Commodity::Ore,
                amount: 4.5,
                source: CargoSource::Spot,
            },
        ]);
    }

    let payload = sim
        .snapshot_payload()
        .expect("snapshot payload serialization should pass");
    let loaded = Simulation::from_snapshot_payload(&payload, cfg)
        .expect("snapshot payload load should pass");
    let ship = loaded
        .ships
        .get(&ship_id)
        .expect("ship should exist after load");

    assert_eq!(
        ship.cargo_lots(),
        &[
            CargoLoad {
                commodity: Commodity::Ore,
                amount: 4.5,
                source: CargoSource::Spot,
            },
            CargoLoad {
                commodity: Commodity::Fuel,
                amount: 6.0,
                source: CargoSource::Spot,
            },
        ]
    );
    assert!((ship.cargo_total_amount() - 10.5).abs() < 1e-9);
}

#[test]
#[cfg(any())]
fn legacy_snapshot_with_single_cargo_object_loads_into_manifest() {
    let cfg = stage_a_config();
    let sim = Simulation::new(cfg.clone(), 162);
    let mut state =
        serde_json::to_value(sim.snapshot_state()).expect("snapshot state should serialize");
    let cargo = CargoLoad {
        commodity: Commodity::Fuel,
        amount: 5.0,
        source: CargoSource::Spot,
    };
    state["ships"]
        .as_array_mut()
        .expect("snapshot ships should serialize as an array")[0]["cargo"] =
        serde_json::to_value(cargo).expect("cargo should serialize");

    let payload = serde_json::json!({
        "version": 3,
        "state": state,
    });
    let tmp = std::env::temp_dir().join("gatebound_stage_a_snapshot_legacy_single_cargo.json");
    fs::write(
        &tmp,
        serde_json::to_string_pretty(&payload).expect("payload serialization should succeed"),
    )
    .expect("legacy snapshot write should succeed");

    let loaded = Simulation::load_snapshot(&tmp, cfg).expect("legacy snapshot should load");
    let ship = loaded
        .ships
        .get(&ShipId(0))
        .expect("player ship should exist after load");

    assert_eq!(ship.cargo_lots(), &[cargo]);
}

#[test]
#[cfg(any())]
fn legacy_snapshot_without_ship_card_fields_loads_and_backfills_metadata() {
    let cfg = stage_a_config();
    let sim = Simulation::new(cfg.clone(), 163);
    let mut state =
        serde_json::to_value(sim.snapshot_state()).expect("snapshot state should serialize");
    let ships = state["ships"]
        .as_array_mut()
        .expect("snapshot ships should serialize as an array");
    for ship in ships {
        let object = ship
            .as_object_mut()
            .expect("ship snapshot should serialize as an object");
        object.remove("descriptor");
        object.remove("modules");
        object.remove("technical_state");
    }

    let payload = serde_json::json!({
        "version": 3,
        "state": state,
    });
    let tmp = std::env::temp_dir().join("gatebound_stage_a_snapshot_ship_card_legacy.json");
    fs::write(
        &tmp,
        serde_json::to_string_pretty(&payload).expect("legacy payload should serialize"),
    )
    .expect("legacy payload write should pass");

    let loaded = Simulation::load_snapshot(&tmp, cfg).expect("legacy snapshot should load");
    let ship = loaded
        .ships
        .get(&ShipId(0))
        .expect("loaded player ship should exist");

    assert!(!ship.descriptor.name.is_empty());
    assert!(!ship.modules.is_empty());
    assert!(ship.technical_state.hull > 0.0);
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
fn mission_offer_generation_populates_route_gates_and_score() {
    let mut sim = Simulation::new(stage_a_config(), 223);
    let source_station = station_for_system(&sim, SystemId(0));
    let destination_station = station_for_system(&sim, last_station_system(&sim));
    for station_id in all_station_ids(&sim) {
        if let Some(book) = sim.markets.get_mut(&station_id) {
            for state in book.goods.values_mut() {
                state.stock = state.target_stock;
                state.cycle_inflow = 0.0;
                state.cycle_outflow = 0.0;
            }
        }
    }
    sim.markets
        .get_mut(&source_station)
        .and_then(|book| book.goods.get_mut(&Commodity::Fuel))
        .expect("source fuel should exist")
        .stock = 180.0;
    sim.markets
        .get_mut(&destination_station)
        .and_then(|book| book.goods.get_mut(&Commodity::Fuel))
        .expect("destination fuel should exist")
        .stock = 20.0;
    sim.refresh_mission_offers();
    let offer = sim
        .mission_offers
        .values()
        .find(|offer| {
            offer.origin_station == source_station
                && offer.destination_station == destination_station
                && offer.commodity == Commodity::Fuel
        })
        .expect("offer should exist");
    assert!(offer.score.is_finite());
    assert!(offer.score.abs() < 10_000.0);
    assert!(offer.risk_score.is_finite());
    assert!(offer.penalty > 0.0);
    assert!(offer.eta_ticks > 0);
    assert!(!offer.route_gate_ids.is_empty());
}

#[test]
fn accepting_mission_offer_moves_goods_into_player_station_storage_and_fixes_penalty() {
    let mut sim = Simulation::new(stage_a_config(), 224);
    let source_station = station_for_system(&sim, SystemId(0));
    let destination_station = station_for_system(&sim, last_station_system(&sim));

    for station_id in all_station_ids(&sim) {
        if let Some(book) = sim.markets.get_mut(&station_id) {
            for state in book.goods.values_mut() {
                state.stock = state.target_stock;
                state.cycle_inflow = 0.0;
                state.cycle_outflow = 0.0;
            }
        }
    }
    sim.markets
        .get_mut(&source_station)
        .and_then(|book| book.goods.get_mut(&Commodity::Fuel))
        .expect("source fuel should exist")
        .stock = 180.0;
    sim.markets
        .get_mut(&destination_station)
        .and_then(|book| book.goods.get_mut(&Commodity::Fuel))
        .expect("destination fuel should exist")
        .stock = 20.0;

    sim.refresh_mission_offers();
    let offer = sim
        .mission_offers
        .values()
        .find(|offer| {
            offer.origin_station == source_station
                && offer.destination_station == destination_station
                && offer.commodity == Commodity::Fuel
        })
        .cloned()
        .expect("mission offer should exist");
    let (before_stock, before_price) = sim
        .markets
        .get(&source_station)
        .and_then(|book| book.goods.get(&Commodity::Fuel))
        .map(|state| (state.stock, state.price))
        .expect("source stock should exist");

    let mission_id = sim
        .accept_mission_offer(offer.id)
        .expect("accepting mission should succeed");

    let stored_amount = sim
        .player_station_storage
        .get(&source_station)
        .and_then(|goods| goods.get(&Commodity::Fuel))
        .copied()
        .expect("player station storage should exist at source station");
    let mission = sim
        .missions
        .get(&mission_id)
        .expect("mission should exist after acceptance");
    let after_stock = sim
        .markets
        .get(&source_station)
        .and_then(|book| book.goods.get(&Commodity::Fuel))
        .map(|state| state.stock)
        .expect("source stock should still exist");

    assert!((offer.quantity - stored_amount).abs() < 1e-9);
    assert!((before_stock - after_stock - offer.quantity).abs() < 1e-9);
    assert!((mission.penalty - offer.quantity * before_price * 5.0).abs() < 1e-9);
}

#[test]
fn accepted_mission_goods_flow_through_regular_storage_and_trade_api() {
    let mut sim = Simulation::new(stage_a_config(), 225);
    let ship_id = ShipId(0);
    let source_station = station_for_system(&sim, SystemId(0));
    let destination_station = station_for_system(&sim, last_station_system(&sim));

    for station_id in all_station_ids(&sim) {
        if let Some(book) = sim.markets.get_mut(&station_id) {
            for state in book.goods.values_mut() {
                state.stock = state.target_stock;
            }
        }
    }
    sim.markets
        .get_mut(&source_station)
        .and_then(|book| book.goods.get_mut(&Commodity::Fuel))
        .expect("source fuel should exist")
        .stock = 170.0;
    sim.markets
        .get_mut(&destination_station)
        .and_then(|book| book.goods.get_mut(&Commodity::Fuel))
        .expect("destination fuel should exist")
        .stock = 25.0;

    sim.refresh_mission_offers();
    let mission_id = sim
        .accept_mission_offer(
            sim.mission_offers
                .values()
                .find(|offer| {
                    offer.origin_station == source_station
                        && offer.destination_station == destination_station
                        && offer.commodity == Commodity::Fuel
                })
                .map(|offer| offer.id)
                .expect("mission offer should exist"),
        )
        .expect("accepting mission should succeed");

    if let Some(ship) = sim.ships.get_mut(&ship_id) {
        ship.location = SystemId(0);
        ship.current_station = Some(source_station);
        ship.eta_ticks_remaining = 0;
        ship.segment_eta_remaining = 0;
        ship.segment_progress_total = 0;
        ship.current_segment_kind = None;
        ship.movement_queue.clear();
    }

    sim.player_load_from_station_storage(ship_id, source_station, Commodity::Fuel, 4.0)
        .expect("accepted goods should load via ordinary storage");
    sim.player_sell(ship_id, source_station, Commodity::Fuel, 1.0)
        .expect("accepted goods should be sellable");
    sim.player_buy(ship_id, source_station, Commodity::Fuel, 1.0)
        .expect("player should be able to buy matching goods again");
    sim.player_unload_to_station_storage(ship_id, source_station, Commodity::Fuel, 4.0)
        .expect("ordinary storage should accept the goods back");

    let stored_amount = sim
        .player_station_storage
        .get(&source_station)
        .and_then(|goods| goods.get(&Commodity::Fuel))
        .copied()
        .expect("player storage should still contain fuel");
    let mission = sim
        .missions
        .get(&mission_id)
        .expect("mission should still exist");
    assert!((stored_amount - mission.quantity).abs() < 1e-9);
}

#[test]
fn completing_mission_consumes_destination_storage_and_pays_reward() {
    let mut sim = Simulation::new(stage_a_config(), 226);
    let ship_id = ShipId(0);
    let source_station = station_for_system(&sim, SystemId(0));
    let destination_station = station_for_system(&sim, last_station_system(&sim));

    for station_id in all_station_ids(&sim) {
        if let Some(book) = sim.markets.get_mut(&station_id) {
            for state in book.goods.values_mut() {
                state.stock = state.target_stock;
            }
        }
    }
    sim.markets
        .get_mut(&source_station)
        .and_then(|book| book.goods.get_mut(&Commodity::Fuel))
        .expect("source fuel should exist")
        .stock = 180.0;
    sim.markets
        .get_mut(&destination_station)
        .and_then(|book| book.goods.get_mut(&Commodity::Fuel))
        .expect("destination fuel should exist")
        .stock = 20.0;

    sim.refresh_mission_offers();
    let offer = sim
        .mission_offers
        .values()
        .find(|offer| {
            offer.origin_station == source_station
                && offer.destination_station == destination_station
                && offer.commodity == Commodity::Fuel
        })
        .cloned()
        .expect("mission offer should exist");
    let mission_id = sim
        .accept_mission_offer(offer.id)
        .expect("accepting mission should succeed");

    if let Some(ship) = sim.ships.get_mut(&ship_id) {
        ship.location = SystemId(0);
        ship.current_station = Some(source_station);
        ship.eta_ticks_remaining = 0;
        ship.segment_eta_remaining = 0;
        ship.segment_progress_total = 0;
        ship.current_segment_kind = None;
        ship.movement_queue.clear();
    }

    sim.player_load_from_station_storage(ship_id, source_station, Commodity::Fuel, offer.quantity)
        .expect("loading accepted goods should use ordinary storage");
    let destination_system = last_station_system(&sim);
    if let Some(ship) = sim.ships.get_mut(&ship_id) {
        ship.location = destination_system;
        ship.current_station = Some(destination_station);
    }
    sim.player_unload_to_station_storage(
        ship_id,
        destination_station,
        Commodity::Fuel,
        offer.quantity,
    )
    .expect("delivered goods should unload through ordinary storage");

    let before_capital = sim.capital;
    let before_destination_stock = sim
        .markets
        .get(&destination_station)
        .and_then(|book| book.goods.get(&Commodity::Fuel))
        .map(|state| state.stock)
        .expect("destination stock should exist");
    sim.complete_mission(ship_id, mission_id)
        .expect("completing the mission should succeed");

    let mission = sim
        .missions
        .get(&mission_id)
        .expect("mission should still exist after completion");
    let destination_storage_amount = sim
        .player_station_storage
        .get(&destination_station)
        .and_then(|goods| goods.get(&Commodity::Fuel))
        .copied()
        .unwrap_or(0.0);
    let after_destination_stock = sim
        .markets
        .get(&destination_station)
        .and_then(|book| book.goods.get(&Commodity::Fuel))
        .map(|state| state.stock)
        .expect("destination stock should exist");
    assert_eq!(mission.status, MissionStatus::Completed);
    assert!((sim.capital - before_capital - offer.reward).abs() < 1e-9);
    assert!(destination_storage_amount.abs() < 1e-9);
    assert!((after_destination_stock - before_destination_stock - offer.quantity).abs() < 1e-9);
}

#[test]
fn cancelling_mission_applies_penalty_without_touching_player_storage() {
    let mut sim = Simulation::new(stage_a_config(), 227);
    let source_station = station_for_system(&sim, SystemId(0));
    let destination_station = station_for_system(&sim, last_station_system(&sim));

    for station_id in all_station_ids(&sim) {
        if let Some(book) = sim.markets.get_mut(&station_id) {
            for state in book.goods.values_mut() {
                state.stock = state.target_stock;
            }
        }
    }
    sim.markets
        .get_mut(&source_station)
        .and_then(|book| book.goods.get_mut(&Commodity::Fuel))
        .expect("source fuel should exist")
        .stock = 170.0;
    sim.markets
        .get_mut(&destination_station)
        .and_then(|book| book.goods.get_mut(&Commodity::Fuel))
        .expect("destination fuel should exist")
        .stock = 30.0;

    sim.refresh_mission_offers();
    let fuel_offer_id = sim
        .mission_offers
        .values()
        .find(|offer| {
            offer.origin_station == source_station
                && offer.destination_station == destination_station
                && offer.commodity == Commodity::Fuel
        })
        .map(|offer| offer.id)
        .expect("fuel mission offer should exist");
    let fuel_mission = sim
        .accept_mission_offer(fuel_offer_id)
        .expect("fuel mission should be accepted");

    let mission = sim
        .missions
        .get(&fuel_mission)
        .cloned()
        .expect("mission should exist");
    let before_storage = sim
        .player_station_storage
        .get(&source_station)
        .and_then(|goods| goods.get(&Commodity::Fuel))
        .copied()
        .expect("accepted goods should be in ordinary storage");
    sim.capital = mission.penalty / 2.0;

    sim.cancel_mission(fuel_mission)
        .expect("cancelling should always be allowed");

    let after_storage = sim
        .player_station_storage
        .get(&source_station)
        .and_then(|goods| goods.get(&Commodity::Fuel))
        .copied()
        .expect("cancelling should not remove player goods");
    let cancelled = sim
        .missions
        .get(&fuel_mission)
        .expect("mission should still exist");
    assert_eq!(cancelled.status, MissionStatus::Cancelled);
    assert!((after_storage - before_storage).abs() < 1e-9);
    assert!((sim.capital + mission.penalty / 2.0).abs() < 1e-9);
}

#[test]
fn missions_board_reports_destination_storage_readiness() {
    let mut sim = Simulation::new(stage_a_config(), 228);
    let ship_id = ShipId(0);
    let source_station = station_for_system(&sim, SystemId(0));
    let destination_system = last_station_system(&sim);
    let destination_station = station_for_system(&sim, destination_system);

    for station_id in all_station_ids(&sim) {
        if let Some(book) = sim.markets.get_mut(&station_id) {
            for state in book.goods.values_mut() {
                state.stock = state.target_stock;
            }
        }
    }
    sim.markets
        .get_mut(&source_station)
        .and_then(|book| book.goods.get_mut(&Commodity::Fuel))
        .expect("source fuel should exist")
        .stock = 175.0;
    sim.markets
        .get_mut(&destination_station)
        .and_then(|book| book.goods.get_mut(&Commodity::Fuel))
        .expect("destination fuel should exist")
        .stock = 25.0;

    sim.refresh_mission_offers();
    let offer = sim
        .mission_offers
        .values()
        .find(|offer| {
            offer.origin_station == source_station
                && offer.destination_station == destination_station
                && offer.commodity == Commodity::Fuel
        })
        .cloned()
        .expect("mission offer should exist");
    let mission_id = sim
        .accept_mission_offer(offer.id)
        .expect("accepting mission should succeed");

    if let Some(ship) = sim.ships.get_mut(&ship_id) {
        ship.location = SystemId(0);
        ship.current_station = Some(source_station);
        ship.eta_ticks_remaining = 0;
        ship.segment_eta_remaining = 0;
        ship.segment_progress_total = 0;
        ship.current_segment_kind = None;
        ship.movement_queue.clear();
    }

    sim.player_load_from_station_storage(ship_id, source_station, Commodity::Fuel, 4.0)
        .expect("accepted goods should load through ordinary storage");
    if let Some(ship) = sim.ships.get_mut(&ship_id) {
        ship.location = destination_system;
        ship.current_station = Some(destination_station);
    }
    sim.player_unload_to_station_storage(ship_id, destination_station, Commodity::Fuel, 1.5)
        .expect("partial unload should use ordinary storage");

    let board = sim.missions_board_view();
    let detail = board
        .active_missions
        .iter()
        .find(|entry| entry.mission.id == mission_id)
        .expect("mission detail should be visible");
    assert!((detail.destination_storage_amount - 1.5).abs() < 1e-9);
    assert_eq!(detail.mission.status, MissionStatus::Accepted);
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
fn market_panel_view_aggregates_galaxy_metrics_per_commodity() {
    let mut sim = Simulation::new(stage_a_config(), 247);
    let all_stations = all_station_ids(&sim);
    let system0 = sim
        .world
        .stations_by_system
        .get(&SystemId(0))
        .cloned()
        .expect("system 0 should exist");
    let system1 = sim
        .world
        .stations_by_system
        .get(&SystemId(1))
        .cloned()
        .expect("system 1 should exist");
    let s0a = system0[0];
    let s0b = system0[1];
    let s1a = system1[0];
    let s1b = system1[1];

    for station_id in &all_stations {
        let fuel = sim
            .markets
            .get_mut(station_id)
            .and_then(|book| book.goods.get_mut(&Commodity::Fuel))
            .expect("fuel state should exist");
        fuel.price = 20.0;
        fuel.stock = 100.0;
        fuel.target_stock = 100.0;
        fuel.cycle_inflow = 4.0;
        fuel.cycle_outflow = 4.0;
        sim.previous_cycle_prices
            .insert((*station_id, Commodity::Fuel), 18.0);
    }

    let patches = [
        (s0a, 10.0, 50.0, 3.0, 9.0, 8.0),
        (s0b, 14.0, 80.0, 2.0, 7.0, 14.0),
        (s1a, 30.0, 120.0, 9.0, 2.0, 28.0),
        (s1b, 26.0, 110.0, 8.0, 3.0, 25.0),
    ];
    for (station_id, price, stock, inflow, outflow, previous_price) in patches {
        let fuel = sim
            .markets
            .get_mut(&station_id)
            .and_then(|book| book.goods.get_mut(&Commodity::Fuel))
            .expect("patched fuel state should exist");
        fuel.price = price;
        fuel.stock = stock;
        fuel.cycle_inflow = inflow;
        fuel.cycle_outflow = outflow;
        sim.previous_cycle_prices
            .insert((station_id, Commodity::Fuel), previous_price);
    }

    let panel = sim.market_panel_view(SystemId(0), Some(s0a), Commodity::Fuel);
    let fuel = panel
        .commodity_rows
        .iter()
        .find(|row| row.commodity == Commodity::Fuel)
        .expect("fuel commodity row should exist");

    let station_count = all_stations.len() as f64;
    let expected_avg = (10.0 + 14.0 + 30.0 + 26.0 + 20.0 * (station_count - 4.0)) / station_count;
    let expected_total_stock = 50.0 + 80.0 + 120.0 + 110.0 + 100.0 * (station_count - 4.0);
    let expected_total_target = 100.0 * station_count;
    let expected_inflow = 3.0 + 2.0 + 9.0 + 8.0 + 4.0 * (station_count - 4.0);
    let expected_outflow = 9.0 + 7.0 + 2.0 + 3.0 + 4.0 * (station_count - 4.0);
    let expected_trend = ((10.0 - 8.0)
        + (14.0 - 14.0)
        + (30.0 - 28.0)
        + (26.0 - 25.0)
        + 2.0 * (station_count - 4.0))
        / station_count;
    let forecast = |price: f64, stock: f64, inflow: f64, outflow: f64| {
        let imbalance = (100.0 - stock) / 100.0;
        let flow_pressure = (outflow - inflow) / 100.0;
        let raw_delta =
            sim.config.market.k_stock * imbalance + sim.config.market.k_flow * flow_pressure;
        let delta = raw_delta.clamp(-sim.config.market.delta_cap, sim.config.market.delta_cap);
        let floor = 16.0 * sim.config.market.floor_mult;
        let ceil = 16.0 * sim.config.market.ceiling_mult;
        (price * (1.0 + delta)).clamp(floor, ceil)
    };
    let expected_forecast = (forecast(10.0, 50.0, 3.0, 9.0)
        + forecast(14.0, 80.0, 2.0, 7.0)
        + forecast(30.0, 120.0, 9.0, 2.0)
        + forecast(26.0, 110.0, 8.0, 3.0)
        + forecast(20.0, 100.0, 4.0, 4.0) * (station_count - 4.0))
        / station_count;

    assert!((fuel.galaxy_avg_price - expected_avg).abs() < 1e-9);
    assert_eq!(fuel.min_price_station_id, Some(s0a));
    assert!((fuel.min_price - 10.0).abs() < 1e-9);
    assert_eq!(fuel.max_price_station_id, Some(s1a));
    assert!((fuel.max_price - 30.0).abs() < 1e-9);
    assert!((fuel.spread_abs - 20.0).abs() < 1e-9);
    assert_eq!(fuel.cheapest_system_id, Some(SystemId(0)));
    assert!((fuel.cheapest_system_avg_price - 12.0).abs() < 1e-9);
    assert_eq!(fuel.priciest_system_id, Some(SystemId(1)));
    assert!((fuel.priciest_system_avg_price - 28.0).abs() < 1e-9);
    assert!((fuel.total_stock - expected_total_stock).abs() < 1e-9);
    assert!((fuel.stock_coverage - (expected_total_stock / expected_total_target)).abs() < 1e-9);
    assert!((fuel.inflow - expected_inflow).abs() < 1e-9);
    assert!((fuel.outflow - expected_outflow).abs() < 1e-9);
    assert!((fuel.net_flow - (expected_inflow - expected_outflow)).abs() < 1e-9);
    assert!((fuel.trend_delta - expected_trend).abs() < 1e-9);
    assert!((fuel.forecast_next_avg - expected_forecast).abs() < 1e-9);
    assert!((fuel.price_vs_base - (expected_avg / 16.0)).abs() < 1e-9);
    assert_eq!(fuel.stations_below_target, 2);
    assert_eq!(fuel.stations_above_target, 2);
}

#[test]
fn market_panel_view_ranks_system_stress_hotspots_and_station_anomalies() {
    let mut sim = Simulation::new(stage_a_config(), 249);
    let all_stations = all_station_ids(&sim);
    let system0 = sim
        .world
        .stations_by_system
        .get(&SystemId(0))
        .cloned()
        .expect("system 0 should exist");
    let system1 = sim
        .world
        .stations_by_system
        .get(&SystemId(1))
        .cloned()
        .expect("system 1 should exist");
    let hot_station = system0[0];
    let stressed_station = system0[1];
    let cheap_station = system1[0];

    for station_id in &all_stations {
        let fuel = sim
            .markets
            .get_mut(station_id)
            .and_then(|book| book.goods.get_mut(&Commodity::Fuel))
            .expect("fuel state should exist");
        fuel.price = 20.0;
        fuel.stock = 100.0;
        fuel.target_stock = 100.0;
        fuel.cycle_inflow = 4.0;
        fuel.cycle_outflow = 4.0;
    }

    for station_id in [hot_station, stressed_station] {
        let fuel = sim
            .markets
            .get_mut(&station_id)
            .and_then(|book| book.goods.get_mut(&Commodity::Fuel))
            .expect("system 0 fuel state should exist");
        fuel.price = if station_id == hot_station {
            40.0
        } else {
            36.0
        };
        fuel.stock = if station_id == hot_station { 8.0 } else { 16.0 };
        fuel.cycle_inflow = 1.0;
        fuel.cycle_outflow = 12.0;
    }
    let cheap_fuel = sim
        .markets
        .get_mut(&cheap_station)
        .and_then(|book| book.goods.get_mut(&Commodity::Fuel))
        .expect("cheap station fuel should exist");
    cheap_fuel.price = 9.0;
    cheap_fuel.stock = 180.0;
    cheap_fuel.cycle_inflow = 12.0;
    cheap_fuel.cycle_outflow = 1.0;

    if let Some(edges) = sim.world.adjacency.get(&SystemId(0)) {
        for (_, gate_id) in edges {
            sim.gate_queue_load.insert(*gate_id, 48.0);
        }
    }

    let panel = sim.market_panel_view(SystemId(0), Some(hot_station), Commodity::Fuel);

    assert_eq!(panel.system_stress_rows[0].system_id, SystemId(0));
    assert_eq!(
        panel.commodity_hotspots.cheapest_stations[0].station_id,
        cheap_station
    );
    assert_eq!(
        panel.commodity_hotspots.priciest_stations[0].station_id,
        hot_station
    );
    assert_eq!(
        panel.commodity_hotspots.cheapest_systems[0].system_id,
        SystemId(1)
    );
    assert_eq!(
        panel.commodity_hotspots.priciest_systems[0].system_id,
        SystemId(0)
    );
    assert_eq!(panel.station_anomaly_rows[0].station_id, hot_station);
}

#[test]
fn market_panel_view_exposes_selected_station_drilldown() {
    let mut sim = Simulation::new(stage_a_config(), 251);
    let detail_station = station_for_system(&sim, SystemId(0));

    for station_id in all_station_ids(&sim) {
        for commodity in [Commodity::Fuel, Commodity::Electronics] {
            let state = sim
                .markets
                .get_mut(&station_id)
                .and_then(|book| book.goods.get_mut(&commodity))
                .expect("state should exist");
            state.price = match commodity {
                Commodity::Fuel => 20.0,
                Commodity::Electronics => 34.0,
                _ => unreachable!(),
            };
            state.stock = 100.0;
            state.target_stock = 100.0;
            state.cycle_inflow = 4.0;
            state.cycle_outflow = 4.0;
            sim.previous_cycle_prices
                .insert((station_id, commodity), state.price - 1.0);
        }
    }

    let fuel = sim
        .markets
        .get_mut(&detail_station)
        .and_then(|book| book.goods.get_mut(&Commodity::Fuel))
        .expect("fuel state should exist");
    fuel.price = 12.0;
    fuel.stock = 30.0;
    fuel.cycle_inflow = 1.0;
    fuel.cycle_outflow = 10.0;
    sim.previous_cycle_prices
        .insert((detail_station, Commodity::Fuel), 11.0);

    let electronics = sim
        .markets
        .get_mut(&detail_station)
        .and_then(|book| book.goods.get_mut(&Commodity::Electronics))
        .expect("electronics state should exist");
    electronics.price = 52.0;
    electronics.stock = 165.0;
    electronics.cycle_inflow = 12.0;
    electronics.cycle_outflow = 1.0;
    sim.previous_cycle_prices
        .insert((detail_station, Commodity::Electronics), 46.0);

    let panel = sim.market_panel_view(SystemId(0), Some(detail_station), Commodity::Fuel);
    let detail = panel
        .station_detail
        .as_ref()
        .expect("station drilldown should exist");
    let fuel_row = detail
        .commodity_rows
        .iter()
        .find(|row| row.commodity == Commodity::Fuel)
        .expect("fuel detail row should exist");
    let electronics_row = detail
        .commodity_rows
        .iter()
        .find(|row| row.commodity == Commodity::Electronics)
        .expect("electronics detail row should exist");

    assert_eq!(detail.station_id, detail_station);
    assert_eq!(detail.system_id, SystemId(0));
    assert_eq!(detail.strongest_shortage_commodity, Some(Commodity::Fuel));
    assert_eq!(
        detail.strongest_surplus_commodity,
        Some(Commodity::Electronics)
    );
    assert_eq!(detail.best_buy_commodity, Some(Commodity::Fuel));
    assert_eq!(detail.best_sell_commodity, Some(Commodity::Electronics));
    assert!((fuel_row.local_price - 12.0).abs() < 1e-9);
    assert!(fuel_row.galaxy_avg_price > fuel_row.local_price);
    assert!(fuel_row.net_flow < 0.0);
    assert!((electronics_row.local_price - 52.0).abs() < 1e-9);
    assert!(electronics_row.galaxy_avg_price < electronics_row.local_price);
    assert!(electronics_row.net_flow > 0.0);
}

#[test]
fn station_trade_view_reports_effective_prices_caps_and_market_tones() {
    let mut cfg = stage_a_config();
    cfg.pressure.market_fee_rate = 0.1;
    let mut sim = Simulation::new(cfg, 239);
    let ship_id = ShipId(0);
    let station_id = station_for_system(&sim, SystemId(0));
    let comparison_station = sim
        .world
        .stations
        .iter()
        .find(|station| station.id != station_id)
        .map(|station| station.id)
        .expect("comparison station should exist");

    if let Some(ship) = sim.ships.get_mut(&ship_id) {
        ship.location = SystemId(0);
        ship.current_station = Some(station_id);
        ship.eta_ticks_remaining = 0;
        ship.segment_eta_remaining = 0;
        ship.segment_progress_total = 0;
        ship.movement_queue.clear();
        ship.cargo_capacity = 18.0;
        ship.cargo = CargoManifest::from(CargoLoad {
            commodity: Commodity::Fuel,
            amount: 4.0,
            source: CargoSource::Spot,
        });
    }

    if let Some(book) = sim.markets.get_mut(&station_id) {
        if let Some(fuel) = book.goods.get_mut(&Commodity::Fuel) {
            fuel.price = 12.0;
            fuel.stock = 30.0;
            fuel.target_stock = 100.0;
        }
    }
    if let Some(book) = sim.markets.get_mut(&comparison_station) {
        if let Some(fuel) = book.goods.get_mut(&Commodity::Fuel) {
            fuel.price = 28.0;
        }
    }

    let view = sim
        .station_trade_view(ship_id, station_id)
        .expect("station trade view should exist");
    let row = view
        .rows
        .iter()
        .find(|row| row.commodity == Commodity::Fuel)
        .expect("fuel row should exist");

    assert!((row.station_stock - 30.0).abs() < 1e-9);
    assert!((row.player_cargo - 4.0).abs() < 1e-9);
    assert!((row.effective_buy_price - 13.2).abs() < 1e-9);
    assert!((row.effective_sell_price - 10.8).abs() < 1e-9);
    assert!(row.market_avg_price > row.effective_buy_price);
    assert_eq!(row.buy_tone, TradePriceTone::Favorable);
    assert_eq!(row.sell_tone, TradePriceTone::Unfavorable);
    assert!((row.buy_cap - 14.0).abs() < 1e-9);
    assert!((row.sell_cap - 4.0).abs() < 1e-9);
    assert!(row.can_buy);
    assert!(row.can_sell);
}

#[test]
fn station_trade_view_disables_spot_actions_while_undocked() {
    let mut sim = Simulation::new(stage_a_config(), 243);
    let ship_id = ShipId(0);
    let station_id = station_for_system(&sim, SystemId(0));

    if let Some(ship) = sim.ships.get_mut(&ship_id) {
        ship.current_station = None;
        ship.eta_ticks_remaining = 7;
        ship.segment_eta_remaining = 3;
        ship.cargo = CargoManifest::from(CargoLoad {
            commodity: Commodity::Fuel,
            amount: 5.0,
            source: CargoSource::Spot,
        });
    }

    let view = sim
        .station_trade_view(ship_id, station_id)
        .expect("station trade view should exist");
    let row = view
        .rows
        .iter()
        .find(|row| row.commodity == Commodity::Fuel)
        .expect("fuel row should exist");

    assert!(!view.docked);
    assert!(!row.can_buy);
    assert!(!row.can_sell);
    assert!((row.buy_cap - 0.0).abs() < 1e-9);
    assert!((row.sell_cap - 0.0).abs() < 1e-9);
}

#[test]
fn station_trade_view_caps_buy_by_available_capital() {
    let mut cfg = stage_a_config();
    cfg.pressure.market_fee_rate = 0.1;
    let mut sim = Simulation::new(cfg, 245);
    let ship_id = ShipId(0);
    let station_id = station_for_system(&sim, SystemId(0));
    sim.capital = 1.0;

    if let Some(ship) = sim.ships.get_mut(&ship_id) {
        ship.location = SystemId(0);
        ship.current_station = Some(station_id);
        ship.eta_ticks_remaining = 0;
        ship.segment_eta_remaining = 0;
        ship.segment_progress_total = 0;
        ship.movement_queue.clear();
        ship.cargo = CargoManifest::default();
    }
    if let Some(book) = sim.markets.get_mut(&station_id) {
        if let Some(fuel) = book.goods.get_mut(&Commodity::Fuel) {
            fuel.price = 12.0;
            fuel.stock = 100.0;
        }
    }

    let view = sim
        .station_trade_view(ship_id, station_id)
        .expect("station trade view should exist");
    let row = view
        .rows
        .iter()
        .find(|row| row.commodity == Commodity::Fuel)
        .expect("fuel row should exist");

    assert!(!row.can_buy);
    assert!((row.buy_cap - 0.0).abs() < 1e-9);
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
