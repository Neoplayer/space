use std::collections::VecDeque;

use gatebound_domain::*;

use super::state::Simulation;

impl Simulation {
    pub(in crate::simulation) fn normalize_player_ship_roster(&mut self) {
        let mut player_candidates = self
            .ships
            .iter()
            .filter(|(_, ship)| {
                ship.company_id == CompanyId(0) || ship.role == ShipRole::PlayerContract
            })
            .map(|(ship_id, _)| *ship_id)
            .collect::<Vec<_>>();
        player_candidates.sort_by_key(|ship_id| ship_id.0);

        if player_candidates.is_empty() {
            let player_location = self
                .world
                .systems
                .first()
                .map(|system| system.id)
                .unwrap_or(SystemId(0));
            let ship_id = ShipId(
                self.ships
                    .keys()
                    .map(|id| id.0)
                    .max()
                    .unwrap_or(0)
                    .saturating_add(1),
            );
            self.ships.insert(
                ship_id,
                Ship {
                    id: ship_id,
                    company_id: CompanyId(0),
                    role: ShipRole::PlayerContract,
                    location: player_location,
                    current_station: self.world.first_station(player_location),
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
                        waypoints: vec![player_location],
                        ..AutopilotPolicy::default()
                    },
                    planned_path: Vec::new(),
                    current_target: None,
                    last_gate_arrival: None,
                    last_risk_score: 0.0,
                    reroutes: 0,
                },
            );
            player_candidates.push(ship_id);
        }

        player_candidates.sort_by_key(|ship_id| ship_id.0);
        let keep_player_ship = player_candidates[0];

        for ship in self.ships.values_mut() {
            if ship.id != keep_player_ship
                && (ship.company_id == CompanyId(0) || ship.role == ShipRole::PlayerContract)
            {
                if ship.company_id == CompanyId(0) {
                    ship.company_id = CompanyId(1);
                }
                ship.role = ShipRole::NpcTrade;
                ship.active_contract = None;
                ship.trade_order_id = None;
                ship.cargo = None;
            }
        }

        if let Some(ship) = self.ships.get_mut(&keep_player_ship) {
            ship.company_id = CompanyId(0);
            ship.role = ShipRole::PlayerContract;
            ship.trade_order_id = None;
            ship.active_contract = None;
        }

        for ship in self.ships.values_mut() {
            if ship.id != keep_player_ship {
                ship.active_contract = None;
            }
        }

        let mut keep_assigned = false;
        let mut contract_ids = self.contracts.keys().copied().collect::<Vec<_>>();
        contract_ids.sort_by_key(|contract_id| contract_id.0);
        for contract_id in contract_ids {
            let Some(contract) = self.contracts.get(&contract_id).cloned() else {
                continue;
            };
            let was_player_assigned = contract
                .assigned_ship
                .is_some_and(|ship_id| player_candidates.contains(&ship_id));
            if !was_player_assigned {
                continue;
            }

            if contract.completed || contract.failed {
                if let Some(item) = self.contracts.get_mut(&contract_id) {
                    item.assigned_ship = None;
                }
                continue;
            }

            if !keep_assigned {
                if let Some(item) = self.contracts.get_mut(&contract_id) {
                    item.assigned_ship = Some(keep_player_ship);
                }
                if let Some(ship) = self.ships.get_mut(&keep_player_ship) {
                    ship.active_contract = Some(contract_id);
                }
                keep_assigned = true;
            } else if let Some(item) = self.contracts.get_mut(&contract_id) {
                item.assigned_ship = None;
                item.progress = ContractProgress::AwaitPickup;
                item.loaded_amount = 0.0;
            }
        }
    }

    pub(in crate::simulation) fn update_ship_movements(&mut self) {
        self.gate_queue_load.clear();
        let dock_delay_factor = self
            .modifiers
            .iter()
            .filter(|m| m.risk == RiskStageA::DockCongestion)
            .map(|m| m.magnitude)
            .fold(1.0_f64, f64::max);

        let ship_ids: Vec<ShipId> = self.ships.keys().copied().collect();
        for ship_id in ship_ids {
            let mut completed_segment = None;
            if let Some(ship) = self.ships.get_mut(&ship_id) {
                if ship.segment_eta_remaining > 0 {
                    ship.segment_eta_remaining = ship.segment_eta_remaining.saturating_sub(1);
                    ship.eta_ticks_remaining = ship.segment_eta_remaining;
                    if ship.segment_eta_remaining == 0 {
                        completed_segment = ship.movement_queue.pop_front();
                        ship.current_segment_kind = None;
                        ship.current_target = None;
                        ship.segment_progress_total = 0;
                    } else {
                        continue;
                    }
                }
            }
            if let Some(segment) = completed_segment {
                if let Some(ship) = self.ships.get_mut(&ship_id) {
                    ship.location = segment.to;
                    if let Some(station_id) = segment.to_anchor {
                        ship.current_station = Some(station_id);
                    } else if segment.kind == SegmentKind::Warp {
                        ship.current_station = None;
                    }
                    if segment.kind == SegmentKind::Warp {
                        ship.last_gate_arrival = segment.edge;
                    }
                }
            }

            self.start_next_movement_segment(ship_id, dock_delay_factor);

            let Some(ship_snapshot) = self.ships.get(&ship_id).cloned() else {
                continue;
            };
            if ship_snapshot.segment_eta_remaining > 0 || !ship_snapshot.movement_queue.is_empty() {
                continue;
            }

            if ship_snapshot.role == ShipRole::NpcTrade {
                if self.advance_npc_trade_ship(ship_id) {
                    self.start_next_movement_segment(ship_id, dock_delay_factor);
                    continue;
                }
                let idle_ticks = self.ship_idle_ticks_cycle.entry(ship_id).or_insert(0);
                *idle_ticks = idle_ticks
                    .saturating_add(1)
                    .min(self.config.time.cycle_ticks.max(1));
                continue;
            }

            if let Some(contract_id) = ship_snapshot.active_contract {
                let should_clear = self.contracts.get(&contract_id).is_none_or(|contract| {
                    matches!(
                        contract.progress,
                        ContractProgress::Completed | ContractProgress::Failed
                    )
                });
                if should_clear {
                    if let Some(ship) = self.ships.get_mut(&ship_id) {
                        ship.active_contract = None;
                    }
                }
            }

            let idle_ticks = self.ship_idle_ticks_cycle.entry(ship_id).or_insert(0);
            *idle_ticks = idle_ticks
                .saturating_add(1)
                .min(self.config.time.cycle_ticks.max(1));
        }
    }

    pub(in crate::simulation) fn start_next_movement_segment(
        &mut self,
        ship_id: ShipId,
        dock_delay_factor: f64,
    ) {
        loop {
            let Some(segment) = self
                .ships
                .get(&ship_id)
                .and_then(|ship| ship.movement_queue.front().cloned())
            else {
                if let Some(ship) = self.ships.get_mut(&ship_id) {
                    ship.segment_eta_remaining = 0;
                    ship.segment_progress_total = 0;
                    ship.current_segment_kind = None;
                    ship.current_target = None;
                    ship.eta_ticks_remaining = 0;
                }
                return;
            };

            let mut eta = segment.eta_ticks;
            if segment.kind == SegmentKind::GateQueue {
                if let Some(edge) = segment.edge {
                    *self.gate_queue_load.entry(edge).or_insert(0.0) += 1.0;
                    let queue_delay = self.gate_queue_eta(edge);
                    self.queue_delay_accumulator = self
                        .queue_delay_accumulator
                        .saturating_add(u64::from(queue_delay));
                    let delay_ticks = self.ship_delay_ticks_cycle.entry(ship_id).or_insert(0);
                    *delay_ticks = delay_ticks
                        .saturating_add(queue_delay)
                        .min(self.config.time.cycle_ticks.max(1) * 4);
                    eta = eta.saturating_add(queue_delay);
                }
                eta = eta.saturating_add(dock_delay_factor.ceil() as u32);
            }

            if segment.kind == SegmentKind::Warp {
                if let Some(edge) = segment.edge {
                    let company_id = self
                        .ships
                        .get(&ship_id)
                        .map(|ship| ship.company_id)
                        .unwrap_or(CompanyId(0));
                    if company_id == CompanyId(0) {
                        self.capital -= self.config.pressure.gate_fee_per_jump;
                    }
                    self.record_gate_traversal(edge, company_id);
                }
            }

            if let Some(ship) = self.ships.get_mut(&ship_id) {
                if segment.from_anchor.is_some() {
                    ship.last_gate_arrival = None;
                    ship.current_station = None;
                }
                ship.current_segment_kind = Some(segment.kind);
                ship.current_target = Some(segment.to);
                ship.segment_progress_total = eta;
                ship.segment_eta_remaining = eta;
                ship.eta_ticks_remaining = eta;
            }

            if eta > 0 {
                return;
            }

            if let Some(ship) = self.ships.get_mut(&ship_id) {
                ship.location = segment.to;
                if let Some(station_id) = segment.to_anchor {
                    ship.current_station = Some(station_id);
                } else if segment.kind == SegmentKind::Warp {
                    ship.current_station = None;
                }
                if segment.kind == SegmentKind::Warp {
                    ship.last_gate_arrival = segment.edge;
                }
                ship.movement_queue.pop_front();
                ship.current_target = None;
                ship.current_segment_kind = None;
                ship.segment_progress_total = 0;
                ship.segment_eta_remaining = 0;
                ship.eta_ticks_remaining = 0;
            }
        }
    }

    pub(in crate::simulation) fn project_ship_job_queue(&self, ship: &Ship) -> Vec<FleetJobStep> {
        let mut queue = Vec::new();
        if let Some(contract_id) = ship.active_contract {
            if let Some(contract) = self.contracts.get(&contract_id) {
                if contract.progress == ContractProgress::AwaitPickup {
                    queue.push(FleetJobStep {
                        kind: FleetJobKind::Pickup,
                        system: contract.origin,
                        eta_ticks: 0,
                    });
                }
            }
        }
        let mut eta_cursor = ship.segment_eta_remaining;
        for (idx, segment) in ship.movement_queue.iter().enumerate() {
            let step_kind = match segment.kind {
                SegmentKind::InSystem => FleetJobKind::Transit,
                SegmentKind::GateQueue => FleetJobKind::GateQueue,
                SegmentKind::Warp => FleetJobKind::Warp,
                SegmentKind::Dock => FleetJobKind::Unload,
            };
            if idx > 0 {
                eta_cursor = eta_cursor.saturating_add(segment.eta_ticks);
            }
            queue.push(FleetJobStep {
                kind: step_kind,
                system: segment.to,
                eta_ticks: eta_cursor,
            });
        }
        if let Some(contract_id) = ship.active_contract {
            if let Some(contract) = self.contracts.get(&contract_id) {
                if matches!(
                    contract.progress,
                    ContractProgress::InTransit | ContractProgress::AwaitPickup
                ) {
                    queue.push(FleetJobStep {
                        kind: FleetJobKind::Unload,
                        system: contract.destination,
                        eta_ticks: eta_cursor,
                    });
                }
            }
        }
        if let Some(loop_target) = ship.policy.waypoints.first().copied() {
            queue.push(FleetJobStep {
                kind: FleetJobKind::LoopReturn,
                system: loop_target,
                eta_ticks: eta_cursor.saturating_add(1),
            });
        }
        queue
    }
}
