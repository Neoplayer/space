use std::collections::VecDeque;

use gatebound_domain::*;

use super::state::Simulation;

impl Simulation {
    pub fn set_edge_blocked_until(&mut self, edge: GateId, until_tick: u64) {
        if let Some(item) = self.world.edges.iter_mut().find(|e| e.id == edge) {
            item.blocked_until_tick = until_tick;
        }
    }

    pub fn route_for_ship(
        &self,
        ship_id: ShipId,
        destination: SystemId,
    ) -> Option<RoutePlan> {
        let ship = self.ships.get(&ship_id)?;
        let origin_station = ship
            .current_station
            .or_else(|| self.world.first_station(ship.location))?;
        let destination_station = self.world.first_station(destination)?;
        self.build_station_route_with_speed(
            origin_station,
            destination_station,
            ship.policy.clone(),
            ship.sub_light_speed,
        )
    }

    pub fn command_fly_to_station(
        &mut self,
        ship_id: ShipId,
        station_id: StationId,
    ) -> Result<(), CommandError> {
        let Some(ship_snapshot) = self.ships.get(&ship_id).cloned() else {
            return Err(CommandError::UnknownShip);
        };
        if ship_snapshot.company_id != CompanyId(0) {
            return Err(CommandError::InvalidAssignment);
        }
        if !self.world.stations.iter().any(|station| station.id == station_id) {
            return Err(CommandError::UnknownStation);
        }
        if ship_snapshot.segment_eta_remaining > 0
            || ship_snapshot.eta_ticks_remaining > 0
            || !ship_snapshot.movement_queue.is_empty()
        {
            return Err(CommandError::ShipBusy);
        }

        let origin_station = ship_snapshot
            .current_station
            .or_else(|| self.world.first_station(ship_snapshot.location))
            .ok_or(CommandError::NoRoute)?;
        if origin_station == station_id {
            return Ok(());
        }

        let route = self
            .build_station_route_with_speed(
                origin_station,
                station_id,
                ship_snapshot.policy.clone(),
                ship_snapshot.sub_light_speed,
            )
            .ok_or(CommandError::NoRoute)?;

        if let Some(ship) = self.ships.get_mut(&ship_id) {
            ship.last_risk_score = route.risk_score;
            ship.movement_queue = VecDeque::from(route.segments.clone());
            ship.planned_path = route.segments.iter().map(|segment| segment.to).collect();
            ship.segment_eta_remaining = 0;
            ship.segment_progress_total = 0;
            ship.current_segment_kind = None;
            ship.current_target = None;
            ship.eta_ticks_remaining = 0;
            ship.last_gate_arrival = None;
        }
        let dock_delay_factor = self
            .modifiers
            .iter()
            .filter(|m| m.risk == RiskStageA::DockCongestion)
            .map(|m| m.magnitude)
            .fold(1.0_f64, f64::max);
        self.start_next_movement_segment(ship_id, dock_delay_factor);
        Ok(())
    }

    pub fn build_station_route(
        &self,
        origin_station: StationId,
        destination_station: StationId,
        policy: AutopilotPolicy,
    ) -> Option<RoutePlan> {
        self.build_station_route_with_speed(origin_station, destination_station, policy, 18.0)
    }

    pub(in crate::simulation) fn build_station_route_with_speed(
        &self,
        origin_station: StationId,
        destination_station: StationId,
        policy: AutopilotPolicy,
        sub_light_speed: f64,
    ) -> Option<RoutePlan> {
        let origin_anchor = self
            .world
            .stations
            .iter()
            .find(|station| station.id == origin_station)?;
        let destination_anchor = self
            .world
            .stations
            .iter()
            .find(|station| station.id == destination_station)?;

        let request = RoutingRequest {
            origin: origin_anchor.system_id,
            destination: destination_anchor.system_id,
            policy,
        };
        let graph = self.world.to_graph_view(self.tick, &self.gate_queue_load);
        let system_route = RoutingService::plan_route(&graph, &request).ok()?;

        let mut segments = Vec::new();
        let mut eta_total = 0_u32;
        let mut risk_total = 0.0_f64;

        let mut cursor_system = origin_anchor.system_id;
        let mut cursor_x = origin_anchor.x;
        let mut cursor_y = origin_anchor.y;
        let mut cursor_anchor = Some(origin_anchor.id);

        for hop in &system_route.segments {
            let gate_id = hop.edge?;
            let (exit_x, exit_y) = self.world.gate_coords(hop.from, gate_id)?;
            let in_eta =
                self.in_system_eta_ticks(cursor_x, cursor_y, exit_x, exit_y, sub_light_speed);
            segments.push(RouteSegment {
                from: cursor_system,
                to: hop.from,
                from_anchor: cursor_anchor,
                to_anchor: None,
                edge: Some(gate_id),
                kind: SegmentKind::InSystem,
                eta_ticks: in_eta,
                risk: 0.0,
            });
            eta_total = eta_total.saturating_add(in_eta);

            let queue_eta = self.gate_queue_eta(gate_id);
            let queue_risk = self.gate_risk(gate_id);
            segments.push(RouteSegment {
                from: hop.from,
                to: hop.from,
                from_anchor: None,
                to_anchor: None,
                edge: Some(gate_id),
                kind: SegmentKind::GateQueue,
                eta_ticks: queue_eta,
                risk: queue_risk,
            });
            eta_total = eta_total.saturating_add(queue_eta);
            risk_total += queue_risk;

            segments.push(RouteSegment {
                from: hop.from,
                to: hop.to,
                from_anchor: None,
                to_anchor: None,
                edge: Some(gate_id),
                kind: SegmentKind::Warp,
                eta_ticks: 0,
                risk: 0.0,
            });

            let (entry_x, entry_y) = self.world.gate_coords(hop.to, gate_id)?;
            cursor_system = hop.to;
            cursor_x = entry_x;
            cursor_y = entry_y;
            cursor_anchor = None;
        }

        let final_eta = self.in_system_eta_ticks(
            cursor_x,
            cursor_y,
            destination_anchor.x,
            destination_anchor.y,
            sub_light_speed,
        );
        segments.push(RouteSegment {
            from: cursor_system,
            to: destination_anchor.system_id,
            from_anchor: cursor_anchor,
            to_anchor: Some(destination_anchor.id),
            edge: None,
            kind: SegmentKind::InSystem,
            eta_ticks: final_eta,
            risk: 0.0,
        });
        eta_total = eta_total.saturating_add(final_eta);

        Some(RoutePlan {
            segments,
            eta_ticks: eta_total,
            risk_score: risk_total,
        })
    }

    pub(in crate::simulation) fn record_gate_traversal(
        &mut self,
        gate_id: GateId,
        company_id: CompanyId,
    ) {
        let by_company = self.gate_traversals_cycle.entry(gate_id).or_default();
        let count = by_company.entry(company_id).or_insert(0);
        *count = count.saturating_add(1);
    }

    pub(in crate::simulation) fn roll_gate_traversal_window(&mut self) {
        self.gate_traversals_window
            .push_back(self.gate_traversals_cycle.clone());
        self.gate_traversals_cycle.clear();
        let max_len = usize::try_from(self.config.time.rolling_window_cycles).unwrap_or(1);
        while self.gate_traversals_window.len() > max_len {
            self.gate_traversals_window.pop_front();
        }
    }

    pub(in crate::simulation) fn gate_queue_eta(&self, gate_id: GateId) -> u32 {
        let load = self.gate_queue_load.get(&gate_id).copied().unwrap_or(0.0);
        let effective_capacity = self
            .world
            .edges
            .iter()
            .find(|edge| edge.id == gate_id)
            .map(|edge| (edge.base_capacity * edge.capacity_factor).max(1.0))
            .unwrap_or(1.0);
        (load / effective_capacity).ceil() as u32
    }

    pub(in crate::simulation) fn gate_risk(&self, gate_id: GateId) -> f64 {
        let load = self.gate_queue_load.get(&gate_id).copied().unwrap_or(0.0);
        let effective_capacity = self
            .world
            .edges
            .iter()
            .find(|edge| edge.id == gate_id)
            .map(|edge| (edge.base_capacity * edge.capacity_factor).max(1.0))
            .unwrap_or(1.0);
        load / effective_capacity
    }

    pub(in crate::simulation) fn in_system_eta_ticks(
        &self,
        from_x: f64,
        from_y: f64,
        to_x: f64,
        to_y: f64,
        sub_light_speed: f64,
    ) -> u32 {
        let speed = sub_light_speed.max(0.1);
        let dx = to_x - from_x;
        let dy = to_y - from_y;
        let distance = (dx * dx + dy * dy).sqrt();
        (distance / speed).ceil().max(1.0) as u32
    }

    pub(in crate::simulation) fn average_gate_load(&self) -> f64 {
        if self.gate_queue_load.is_empty() {
            return 0.0;
        }
        self.gate_queue_load.values().sum::<f64>() / self.gate_queue_load.len() as f64
    }
}
