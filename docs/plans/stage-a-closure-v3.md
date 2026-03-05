# Stage A Closure v3

## Scope
- Stage A-only gap closure from `docs/Idea.md`.
- Deeper `Contracts / Fleet / Markets` panels.
- `Market Share` milestone added to Stage A progression.
- Soft-fail recovery extends to lease-first auto-deactivation with audit log.
- Minimal SystemView readability overlays (sun/orbits/station markers/gate pulse).

## Implemented
- `gatebound_core`:
  - Added config keys:
    - `milestone_market_share_target`
    - `premium_offer_reputation_min`
  - Added offer diagnostics:
    - `OfferProblemTag`
    - `ContractOffer.route_gate_ids`
    - `ContractOffer.problem_tag`
    - `ContractOffer.premium`
    - `ContractOffer.profit_per_ton`
  - Added fleet projection/KPI types:
    - `FleetJobKind`
    - `FleetJobStep`
    - extended `FleetShipStatus` with `job_queue`, `idle_ticks_cycle`, `avg_delay_ticks_cycle`, `profit_per_run`
  - Added milestone:
    - `MilestoneId::MarketShare`
  - Added analytics/recovery types:
    - `MarketInsightRow`
    - `RecoveryAction`
  - Added APIs:
    - `Simulation::market_share_view()`
    - `Simulation::market_insights(system_id)`
    - `Simulation::recent_recovery_actions()`
  - Soft-fail recovery now:
    - issues emergency loan
    - auto-releases up to 2 most expensive active leases
    - appends `RecoveryAction` entry
  - Snapshot v2 now carries optional additional state blobs:
    - `ship_kpis`
    - `recovery_log`
    - `prev_prices`
    - richer offer rows with diagnostics and route gates
- `gatebound_app`:
  - `ContractsFilterState` extended with:
    - `route_gate`
    - `problem`
    - `premium_only`
  - Added `UiKpiTracker` resource:
    - manual actions/min
    - policy edits/min
    - avg route hops (player)
  - Contracts Board:
    - filters for route gate/problem/premium
    - columns for `ppt`, `problem`, `gates`, `intel`
  - Fleet Manager:
    - collapsible `job_queue`
    - KPI lines `idle/delay/profit_run`
  - Markets panel:
    - throughput + global market share
    - insight rows (`trend`, `forecast`, impact factors)
  - Assets panel:
    - lease burden + ROI proxy
    - recovery log
  - Policies panel:
    - manual vs policy KPI block
    - policy edits count toward tracker
  - SystemView overlay additions:
    - sun disk
    - orbit bands
    - station markers
    - gate pulse rings

## Verification Matrix
- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test`
- `cargo run -p gatebound_app`

## Notes
- Scope is intentionally limited to Stage A closure; no Stage B systems/construction/combat/multiplayer changes included.

## Movement Model v1.1
- `Warp` is now teleport (`eta_ticks = 0`) on every inter-system segment.
- Added lightweight station anchors:
  - `World.stations`
  - `World.stations_by_system`
  - deterministic generation: 2 anchors per system.
- Routing pipeline is station-based:
  - `InSystem -> GateQueue -> Warp(0) -> ... -> InSystem`.
- Contracts/offers now carry station endpoints:
  - `origin_station`
  - `destination_station`
- Ship runtime uses segment queue state:
  - `movement_queue`
  - `segment_eta_remaining`
  - `segment_progress_total`
  - `current_segment_kind`
  - `sub_light_speed`
- Snapshot v2 extended with optional movement/station chunks:
  - `stations`
  - `ship_runtime`
  - backward-compatible load keeps deterministic defaults for missing station fields.
