use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::{AutopilotPolicy, GateId, StationId, SystemId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SegmentKind {
    InSystem,
    GateQueue,
    Warp,
    Dock,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RouteSegment {
    pub from: SystemId,
    pub to: SystemId,
    pub from_anchor: Option<StationId>,
    pub to_anchor: Option<StationId>,
    pub edge: Option<GateId>,
    pub kind: SegmentKind,
    pub eta_ticks: u32,
    pub risk: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RoutePlan {
    pub segments: Vec<RouteSegment>,
    pub eta_ticks: u32,
    pub risk_score: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RoutingRequest {
    pub origin: SystemId,
    pub destination: SystemId,
    pub policy: AutopilotPolicy,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RoutingGraphView {
    pub adjacency: BTreeMap<SystemId, Vec<(SystemId, GateId)>>,
    pub gate_eta_ticks: BTreeMap<GateId, u32>,
    pub gate_risk: BTreeMap<GateId, f64>,
    pub blocked_edges: BTreeSet<GateId>,
}

pub struct RoutingService;

impl RoutingService {
    pub fn plan_route(
        graph: &RoutingGraphView,
        request: &RoutingRequest,
    ) -> Result<RoutePlan, crate::RoutingError> {
        if request.origin == request.destination {
            return Ok(RoutePlan {
                segments: Vec::new(),
                eta_ticks: 0,
                risk_score: 0.0,
            });
        }

        let mut queue = VecDeque::new();
        let mut prev: BTreeMap<SystemId, (SystemId, GateId)> = BTreeMap::new();
        let mut visited = BTreeSet::new();
        visited.insert(request.origin);
        queue.push_back(request.origin);

        while let Some(node) = queue.pop_front() {
            if node == request.destination {
                break;
            }
            if let Some(neighbors) = graph.adjacency.get(&node) {
                for (next, gate) in neighbors {
                    if graph.blocked_edges.contains(gate) {
                        continue;
                    }
                    if visited.insert(*next) {
                        prev.insert(*next, (node, *gate));
                        queue.push_back(*next);
                    }
                }
            }
        }

        if !visited.contains(&request.destination) {
            return Err(crate::RoutingError::Unreachable);
        }

        let mut rev = Vec::<(SystemId, SystemId, GateId)>::new();
        let mut cursor = request.destination;
        while cursor != request.origin {
            let (p, gate) = prev
                .get(&cursor)
                .copied()
                .ok_or(crate::RoutingError::Unreachable)?;
            rev.push((p, cursor, gate));
            cursor = p;
        }
        rev.reverse();

        if rev.len() > request.policy.max_hops {
            return Err(crate::RoutingError::MaxHopsExceeded);
        }

        let mut segments = Vec::new();
        let mut eta = 0_u32;
        let mut risk = 0.0_f64;

        for (from, to, gate) in rev {
            let gate_eta = *graph.gate_eta_ticks.get(&gate).unwrap_or(&1);
            let gate_risk = *graph.gate_risk.get(&gate).unwrap_or(&0.0);

            segments.push(RouteSegment {
                from,
                to,
                from_anchor: None,
                to_anchor: None,
                edge: Some(gate),
                kind: SegmentKind::Warp,
                eta_ticks: gate_eta,
                risk: gate_risk,
            });
            eta = eta.saturating_add(gate_eta);
            risk += gate_risk;
        }

        Ok(RoutePlan {
            segments,
            eta_ticks: eta,
            risk_score: risk,
        })
    }
}
