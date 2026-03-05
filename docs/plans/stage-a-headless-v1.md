# Stage A Headless v1

## Goal
Deliver a deterministic Stage A headless simulation core for Gatebound Logistics:
- cluster simulation in `3..=7` systems
- `tick=1s`, `cycle=60 ticks`
- `Delivery` and `Supply` contracts only
- policy-based loop autopilot with reroute
- Stage A risks: gate congestion, dock congestion, fuel shock
- debug snapshot save/load

## Workspace Layout
- `Cargo.toml` (workspace)
- `crates/gatebound_core` (simulation core + tests)
- `crates/gatebound_app` (thin runtime shell)
- `assets/config/stage_a/*.toml` (external balance/runtime configuration)

## Public API (core)
- `Simulation::new(config, seed) -> Simulation`
- `Simulation::step_tick() -> TickReport`
- `Simulation::step_cycle() -> CycleReport`
- `Simulation::apply_event(event)`
- `Simulation::save_snapshot(path)` / `Simulation::load_snapshot(path, config)`
- `RoutingService::plan_route(graph, request) -> Result<RoutePlan, RoutingError>`

## Scope Guards
- Stage A contract types are restricted to `Delivery | Supply`
- Stage A risk types are restricted to `GateCongestion | DockCongestion | FuelShock`
- No station construction / route-contract / station-service / full manual routing logic in this increment

## Acceptance Matrix
| Requirement | Status | Evidence |
| --- | --- | --- |
| Cluster size and generation invariants | Done | test `generation_respects_cluster_and_connectivity_and_degree` |
| Gate placement on boundary (`1 edge = 1 gate pair`) | Done | test `gate_nodes_are_placed_on_system_boundary` |
| Multi-hop routing + max hops constraints | Done | test `routing_supports_multihop_and_respects_max_hops` |
| Auto reroute on blocked edges | Done | test `reroute_happens_when_edge_blocked` |
| Delivery penalties and soft-fail run continuity | Done | test `delivery_penalty_curve_applies_without_hard_fail` |
| Supply per-cycle shortfall and progressive penalties | Done | test `supply_contract_tracks_cycle_shortfall_and_progressive_penalty` |
| Price formula clamps (`delta_cap`, floor, ceiling) | Done | test `price_update_respects_delta_cap_and_floor_ceiling` |
| Fuel shock propagates into market prices | Done | test `fuel_shock_increases_fuel_price_index` |
| Congestion impacts ETA/risk | Done | test `congestion_changes_eta_and_risk` |
| Autopilot loop and policy effects | Done | test `autopilot_loop_and_policy_change_affect_route` |
| Determinism (same seed + ticks) | Done | test `deterministic_seed_tick_run_produces_same_hash_and_reports` |
| Snapshot round-trip replay | Done | test `snapshot_round_trip_restores_future_ticks` |
| Stage A scope lock checks | Done | test `stage_a_scope_guards_are_locked` |
| Intel freshness model (local vs remote) | Done | test `market_intel_local_is_fresh_remote_is_stale` |
| Tick latency percentile gate | Done | test `benchmark_cluster_tick_latency_reports_percentiles` |

## Verification Commands
```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```

## Notes
- Snapshot format is JSON with deterministic string payload (`SnapshotV1` semantics, versioned as `version=1`).
- Determinism target is same binary + same platform for identical seed/tick schedule.
- `gatebound_app` is intentionally minimal in Stage A and depends on `gatebound_core` only.
