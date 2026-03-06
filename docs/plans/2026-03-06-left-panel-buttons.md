# Left Panel Buttons Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Restore a clickable left-side window switcher and make every HUD window start closed by default.

**Architecture:** Keep panel state in `UiPanelState`, define a single source of truth for button metadata in runtime code, and have the HUD renderer loop over that metadata to draw buttons. Cover the behavior with app-level unit tests so defaults, labels, and toggle wiring stay aligned.

**Tech Stack:** Rust, Bevy, bevy_egui, cargo test

---

### Task 1: Lock down default panel behavior

**Files:**
- Modify: `crates/gatebound_app/src/app_tests.rs`
- Modify: `crates/gatebound_app/src/runtime/sim.rs`

**Step 1: Write the failing test**

Add assertions that `UiPanelState::default()` starts with all windows closed and that panel toggles still open each window when invoked.

**Step 2: Run test to verify it fails**

Run: `cargo test panel_hotkeys_toggle_expected_windows --package gatebound_app`

Expected: FAIL because the current default state opens every panel.

**Step 3: Write minimal implementation**

Change `UiPanelState::default()` so every window flag starts as `false`.

**Step 4: Run test to verify it passes**

Run: `cargo test panel_hotkeys_toggle_expected_windows --package gatebound_app`

Expected: PASS.

### Task 2: Define shared button metadata

**Files:**
- Modify: `crates/gatebound_app/src/runtime/sim.rs`
- Modify: `crates/gatebound_app/src/app_tests.rs`

**Step 1: Write the failing test**

Add a test that validates the button metadata covers all six panels with the expected user-facing labels.

**Step 2: Run test to verify it fails**

Run: `cargo test left_panel_buttons_cover_all_windows --package gatebound_app`

Expected: FAIL because the metadata does not exist yet.

**Step 3: Write minimal implementation**

Add a small panel-button descriptor type plus a constant list for `Contracts`, `MyShip`, `Markets`, `Finance`, `Policies`, and `Station`.

**Step 4: Run test to verify it passes**

Run: `cargo test left_panel_buttons_cover_all_windows --package gatebound_app`

Expected: PASS.

### Task 3: Render the buttons in the left HUD rail

**Files:**
- Modify: `crates/gatebound_app/src/ui/hud/render.rs`
- Test: `crates/gatebound_app/src/app_tests.rs`

**Step 1: Write the failing test**

Use the shared metadata test surface so rendering can iterate the same definitions instead of hardcoded labels.

**Step 2: Run test to verify it fails**

Run: `cargo test left_panel_buttons_cover_all_windows --package gatebound_app`

Expected: FAIL until rendering consumes the shared metadata.

**Step 3: Write minimal implementation**

Replace the left-panel help-only section with a `Windows` button group that toggles the existing panel flags and displays current open/closed state.

**Step 4: Run test to verify it passes**

Run: `cargo test --package gatebound_app`

Expected: PASS.

### Task 4: Verify project checks

**Files:**
- No code changes

**Step 1: Run formatting**

Run: `cargo fmt --all -- --check`

**Step 2: Run lint**

Run: `cargo clippy --all-targets -- -D warnings`

**Step 3: Run tests**

Run: `cargo test`

Expected: all commands pass.
