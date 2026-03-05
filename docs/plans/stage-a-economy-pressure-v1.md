# Stage A Economy Pressure Loop v1

## Scope
- Station lease only: `Dock`, `Storage`, `Factory`, `Market`.
- Soft-fail only: emergency loan, reputation penalty, interest-rate hike.
- UI interaction only: hotkeys + HUD panel (no dedicated board UI).

## Implemented Changes
- Added lease API in `gatebound_core`:
  - `SlotType`
  - `LeasePosition`
  - `LeaseMarketView`
  - `LeaseError`
  - `Simulation::lease_slot`
  - `Simulation::release_one_slot`
  - `Simulation::lease_market_for_system`
- Added economy pressure state in `Simulation`:
  - `active_leases`
  - `outstanding_debt`
  - `reputation`
  - `current_loan_interest_rate`
  - `recovery_events`
- Implemented lease price model from throughput + gate proximity + congestion signals.
- Implemented lease lifecycle expiration on cycle boundary.
- Reworked upkeep loop:
  - removed unconditional flat slot lease upkeep
  - added per-tick lease upkeep from active lease prices
- Implemented cycle-end debt servicing:
  - interest charge
  - auto-repay from positive capital
- Implemented cycle-end soft-fail recovery on negative capital:
  - emergency loan
  - reputation penalty
  - interest-rate hike with max clamp
- Migrated snapshot save format to version 2.
- Kept backward-compatible loading for snapshot version 1.

## App Integration
- Added lease hotkeys:
  - `Z` Dock lease
  - `X` Storage lease
  - `C` Factory lease
  - `V` Market lease
  - `R` release one lease for last selected slot type
- Added selected-system fallback logic (`Galaxy` -> `SystemId(0)`).
- Extended HUD:
  - economy pressure block (`debt`, `interest rate`, `reputation`, `recovery events`)
  - leases block (active leases and selected-system lease market)
  - controls list with lease hotkeys

## Config
- Extended `assets/config/stage_a/economy_pressure.toml` with:
  - `lease_price_throughput_k`
  - `lease_price_gate_k`
  - `lease_price_congestion_k`
  - `lease_price_min_mult`
  - `lease_price_max_mult`
  - `recovery_loan_base`
  - `recovery_loan_buffer`
  - `recovery_reputation_penalty`
  - `recovery_rate_hike`
  - `recovery_rate_max`
