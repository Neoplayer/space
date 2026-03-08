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
    pub fn plan_route_legacy(
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

        let (eta_weight, risk_weight) = match request.policy.priority_mode {
            crate::PriorityMode::Profit => (1.0_f64, 0.15_f64),
            crate::PriorityMode::Hybrid => (1.0_f64, 1.5_f64),
            crate::PriorityMode::Stability => (1.0_f64, 4.0_f64),
        };

        #[derive(Debug, Clone, Copy)]
        struct PathState {
            node: SystemId,
            hops: usize,
            cost: f64,
            eta: u32,
            risk: f64,
        }

        let mut frontier = vec![PathState {
            node: request.origin,
            hops: 0,
            cost: 0.0,
            eta: 0,
            risk: 0.0,
        }];
        let mut best: BTreeMap<(SystemId, usize), (f64, f64)> =
            BTreeMap::from([((request.origin, 0), (0.0, 0.0))]);
        let mut prev: BTreeMap<(SystemId, usize), ((SystemId, usize), GateId)> = BTreeMap::new();
        let mut best_goal: Option<PathState> = None;

        while !frontier.is_empty() {
            let next_idx = frontier
                .iter()
                .enumerate()
                .min_by(|(_, left), (_, right)| {
                    left.cost
                        .total_cmp(&right.cost)
                        .then_with(|| left.hops.cmp(&right.hops))
                })
                .map(|(idx, _)| idx)
                .unwrap_or(0);
            let current = frontier.swap_remove(next_idx);
            if current.node == request.destination {
                best_goal = Some(current);
                break;
            }
            if current.hops >= request.policy.max_hops {
                continue;
            }

            let Some(neighbors) = graph.adjacency.get(&current.node) else {
                continue;
            };
            for (next, gate) in neighbors {
                if graph.blocked_edges.contains(gate) {
                    continue;
                }

                let gate_eta = *graph.gate_eta_ticks.get(gate).unwrap_or(&1);
                let gate_risk = *graph.gate_risk.get(gate).unwrap_or(&0.0);
                let next_risk = current.risk + gate_risk;
                if next_risk > request.policy.max_risk_score {
                    continue;
                }

                let next_hops = current.hops + 1;
                let next_cost =
                    current.cost + gate_eta as f64 * eta_weight + gate_risk * risk_weight;
                let next_eta = current.eta.saturating_add(gate_eta);
                let key = (*next, next_hops);
                let replace = best.get(&key).is_none_or(|(best_cost, best_risk)| {
                    next_cost < *best_cost || (next_cost == *best_cost && next_risk < *best_risk)
                });
                if replace {
                    best.insert(key, (next_cost, next_risk));
                    prev.insert(key, ((current.node, current.hops), *gate));
                    frontier.push(PathState {
                        node: *next,
                        hops: next_hops,
                        cost: next_cost,
                        eta: next_eta,
                        risk: next_risk,
                    });
                }
            }
        }

        let Some(goal) = best_goal else {
            return Err(crate::RoutingError::Unreachable);
        };

        let mut rev = Vec::<(SystemId, SystemId, GateId)>::new();
        let mut cursor = (goal.node, goal.hops);
        while cursor.0 != request.origin {
            let (previous, gate) = prev
                .get(&cursor)
                .copied()
                .ok_or(crate::RoutingError::Unreachable)?;
            rev.push((previous.0, cursor.0, gate));
            cursor = previous;
        }
        rev.reverse();

        let mut segments = Vec::new();
        for (from, to, gate) in rev {
            segments.push(RouteSegment {
                from,
                to,
                from_anchor: None,
                to_anchor: None,
                edge: Some(gate),
                kind: SegmentKind::Warp,
                eta_ticks: *graph.gate_eta_ticks.get(&gate).unwrap_or(&1),
                risk: *graph.gate_risk.get(&gate).unwrap_or(&0.0),
            });
        }

        Ok(RoutePlan {
            segments,
            eta_ticks: goal.eta,
            risk_score: goal.risk,
        })
    }
}
