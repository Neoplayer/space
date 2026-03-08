use gatebound_domain::{
    AutopilotPolicy, Cluster, ClusterId, Faction, FactionId, GateEdge, GateId, GateNode,
    PriorityMode, RepeatMode, RouteSegment, RoutingError, RoutingGraphView, RoutingRequest,
    RoutingService, SegmentKind, StationAnchor, StationId, StationProfile, SystemId, SystemNode,
    World,
};
use std::collections::{BTreeMap, BTreeSet};

#[test]
fn routing_service_plans_multihop_warp_segments() {
    let mut adjacency = BTreeMap::new();
    adjacency.insert(SystemId(0), vec![(SystemId(1), GateId(5))]);
    adjacency.insert(
        SystemId(1),
        vec![(SystemId(0), GateId(5)), (SystemId(2), GateId(9))],
    );
    adjacency.insert(SystemId(2), vec![(SystemId(1), GateId(9))]);

    let graph = RoutingGraphView {
        adjacency,
        gate_eta_ticks: BTreeMap::from([(GateId(5), 7), (GateId(9), 11)]),
        gate_risk: BTreeMap::from([(GateId(5), 0.2), (GateId(9), 0.4)]),
        blocked_edges: BTreeSet::new(),
    };
    let request = RoutingRequest {
        origin: SystemId(0),
        destination: SystemId(2),
        policy: AutopilotPolicy {
            min_margin: 0.0,
            max_risk_score: 10.0,
            max_hops: 4,
            priority_mode: PriorityMode::Hybrid,
            waypoints: vec![SystemId(0)],
            repeat_mode: RepeatMode::Loop,
        },
    };

    let route = RoutingService::plan_route(&graph, &request).expect("route should exist");

    assert_eq!(route.eta_ticks, 18);
    assert!((route.risk_score - 0.6).abs() < 1e-9);
    assert_eq!(
        route.segments,
        vec![
            RouteSegment {
                from: SystemId(0),
                to: SystemId(1),
                from_anchor: None,
                to_anchor: None,
                edge: Some(GateId(5)),
                kind: SegmentKind::Warp,
                eta_ticks: 7,
                risk: 0.2,
            },
            RouteSegment {
                from: SystemId(1),
                to: SystemId(2),
                from_anchor: None,
                to_anchor: None,
                edge: Some(GateId(9)),
                kind: SegmentKind::Warp,
                eta_ticks: 11,
                risk: 0.4,
            },
        ]
    );
}

#[test]
fn routing_service_prefers_lower_risk_path_for_stability_policy() {
    let mut adjacency = BTreeMap::new();
    adjacency.insert(
        SystemId(0),
        vec![(SystemId(1), GateId(1)), (SystemId(2), GateId(2))],
    );
    adjacency.insert(
        SystemId(1),
        vec![(SystemId(0), GateId(1)), (SystemId(3), GateId(3))],
    );
    adjacency.insert(
        SystemId(2),
        vec![(SystemId(0), GateId(2)), (SystemId(3), GateId(4))],
    );
    adjacency.insert(
        SystemId(3),
        vec![(SystemId(1), GateId(3)), (SystemId(2), GateId(4))],
    );

    let graph = RoutingGraphView {
        adjacency,
        gate_eta_ticks: BTreeMap::from([
            (GateId(1), 3),
            (GateId(2), 4),
            (GateId(3), 3),
            (GateId(4), 4),
        ]),
        gate_risk: BTreeMap::from([
            (GateId(1), 1.8),
            (GateId(2), 0.2),
            (GateId(3), 1.8),
            (GateId(4), 0.2),
        ]),
        blocked_edges: BTreeSet::new(),
    };

    let stable_route = RoutingService::plan_route(
        &graph,
        &RoutingRequest {
            origin: SystemId(0),
            destination: SystemId(3),
            policy: AutopilotPolicy {
                min_margin: 0.0,
                max_risk_score: 5.0,
                max_hops: 4,
                priority_mode: PriorityMode::Stability,
                waypoints: vec![SystemId(0)],
                repeat_mode: RepeatMode::Loop,
            },
        },
    )
    .expect("stability route should exist");

    assert_eq!(
        stable_route
            .segments
            .iter()
            .filter_map(|segment| segment.edge)
            .collect::<Vec<_>>(),
        vec![GateId(2), GateId(4)]
    );
    assert!((stable_route.risk_score - 0.4).abs() < 1e-9);
}

#[test]
fn routing_service_rejects_paths_above_max_risk_score() {
    let graph = RoutingGraphView {
        adjacency: BTreeMap::from([
            (SystemId(0), vec![(SystemId(1), GateId(7))]),
            (SystemId(1), vec![(SystemId(0), GateId(7))]),
        ]),
        gate_eta_ticks: BTreeMap::from([(GateId(7), 5)]),
        gate_risk: BTreeMap::from([(GateId(7), 2.5)]),
        blocked_edges: BTreeSet::new(),
    };

    let err = RoutingService::plan_route(
        &graph,
        &RoutingRequest {
            origin: SystemId(0),
            destination: SystemId(1),
            policy: AutopilotPolicy {
                min_margin: 0.0,
                max_risk_score: 1.0,
                max_hops: 2,
                priority_mode: PriorityMode::Hybrid,
                waypoints: vec![SystemId(0)],
                repeat_mode: RepeatMode::Loop,
            },
        },
    )
    .expect_err("risk ceiling should make route unreachable");

    assert!(matches!(err, RoutingError::Unreachable));
}

#[test]
fn world_queries_return_station_and_gate_coordinates() {
    let world = World {
        systems: vec![SystemNode {
            id: SystemId(0),
            cluster_id: ClusterId(0),
            owner_faction_id: FactionId(0),
            x: 10.0,
            y: 20.0,
            radius: 100.0,
            gate_nodes: vec![GateNode {
                gate_id: GateId(3),
                x: 110.0,
                y: 20.0,
            }],
            dock_capacity: 4.0,
        }],
        edges: vec![GateEdge {
            id: GateId(3),
            a: SystemId(0),
            b: SystemId(0),
            base_capacity: 8.0,
            travel_ticks: 15,
            blocked_until_tick: 0,
            capacity_factor: 1.0,
        }],
        adjacency: BTreeMap::from([(SystemId(0), vec![(SystemId(0), GateId(3))])]),
        factions: vec![Faction {
            id: FactionId(0),
            name: "Test Collective".to_string(),
            color_rgb: [64, 169, 255],
        }],
        clusters: vec![Cluster {
            id: ClusterId(0),
            faction_id: FactionId(0),
            system_ids: vec![SystemId(0)],
        }],
        stations: vec![StationAnchor {
            id: StationId(4),
            system_id: SystemId(0),
            profile: StationProfile::Industrial,
            x: 33.0,
            y: 44.0,
        }],
        stations_by_system: BTreeMap::from([(SystemId(0), vec![StationId(4)])]),
    };

    assert_eq!(world.system_count(), 1);
    assert_eq!(world.first_station(SystemId(0)), Some(StationId(4)));
    assert_eq!(world.station_coords(StationId(4)), Some((33.0, 44.0)));
    assert_eq!(
        world.gate_coords(SystemId(0), GateId(3)),
        Some((110.0, 20.0))
    );
    assert!(world.is_connected());
}
