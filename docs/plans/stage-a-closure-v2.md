# Stage A Closure v2

## Scope
- Contracts Board, Fleet Manager, Markets, Assets, Policies panels.
- Stage A baseline NPC roster with active ships.
- Offer pipeline + acceptance flow.
- Economy anti-exploit: gate fee, market fee, market depth cap.
- Milestones v1: Capital, Throughput Control, Reputation.

## Implemented
- `gatebound_core`:
  - Added public economy/gameplay types:
    - `CompanyArchetype`, `Company`
    - `ContractOffer`, `OfferError`
    - `FleetWarning`, `FleetShipStatus`
    - `MilestoneId`, `MilestoneStatus`
    - `GateThroughputSnapshot`
  - Extended `Simulation` with:
    - `companies`
    - `contract_offers`, `next_offer_id`
    - `milestones`
    - `gate_traversals_cycle`, `gate_traversals_window`
  - Added APIs:
    - `refresh_contract_offers`
    - `accept_contract_offer`
    - `fleet_status`
    - `gate_throughput_view`
    - `milestone_status`
  - Added Stage A baseline company/ship seeding.
  - Added fees/depth behavior:
    - gate fee per warp segment
    - market fee on payouts
    - market depth cap for supply cycle accounting
  - Added milestone updates on cycle boundary.
  - Extended snapshot v2 payload with optional keys:
    - `companies`, `offers`, `milestones`, `gate_cycle`, `gate_window`
  - Preserved backward-compatible loading for snapshot v1.
- `gatebound_app`:
  - Added UI resources:
    - `UiPanelState`
    - `ContractsFilterState`
    - `SelectedShip`
    - `SelectedSystem`
  - Added panel hotkeys:
    - `F1..F5` panel toggles
    - `[` `]` ship selection
  - Added selected-system sync from camera mode.
  - Reworked HUD into multi-window Stage A UI:
    - Contracts Board
    - Fleet Manager
    - Markets
    - Assets / Real Estate
    - Autopilot Policies
  - Added bottleneck readability overlays in world render:
    - gate load intensity
    - dock congestion ring
    - fuel stress ring
- Config:
  - Extended `assets/config/stage_a/economy_pressure.toml` with fees/offers/milestones keys.

## Verification Matrix
- Core tests cover offers, acceptance, expiration, fees, depth, NPC roster, throughput, milestones, snapshot defaults.
- App tests cover panel toggles/filtering, fleet warnings, selected system/fallback, policy edits.
- Full quality gate:
  - `cargo fmt --all -- --check`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test`
