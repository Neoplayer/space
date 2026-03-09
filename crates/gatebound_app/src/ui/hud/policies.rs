use bevy_egui::egui;
use gatebound_domain::{PriorityMode, ShipId};

use crate::runtime::sim::{SelectedShip, SimResource, UiKpiTracker, UiPanelState};

use super::labels::{milestone_label, priority_mode_label};
use super::snapshot::HudSnapshot;

pub(crate) fn resolve_policy_ship_id(
    selected_ship_id: Option<ShipId>,
    default_player_ship_id: Option<ShipId>,
) -> Option<ShipId> {
    selected_ship_id.or(default_player_ship_id)
}

pub(super) fn render_policies_window(
    ctx: &egui::Context,
    snapshot: &HudSnapshot,
    save_menu_open: bool,
    panels: &mut UiPanelState,
    selected_ship: &SelectedShip,
    sim: &mut SimResource,
    kpi: &mut UiKpiTracker,
) {
    if save_menu_open || !panels.policies {
        return;
    }

    let mut open = panels.policies;
    egui::Window::new("Autopilot Policies")
        .open(&mut open)
        .show(ctx, |ui| {
            let Some(ship_id) =
                resolve_policy_ship_id(selected_ship.ship_id, snapshot.default_player_ship_id)
            else {
                ui.label("No player ship available");
                return;
            };
            ui.label(format!("Selected ship: #{}", ship_id.0));
            let tick_now = sim.simulation.tick();
            if let Some(policy_view) = sim.simulation.ship_policy_view(ship_id) {
                let mut policy = policy_view.policy;
                let mut policy_changed = false;
                ui.horizontal(|ui| {
                    ui.label("min_margin");
                    policy_changed |= ui
                        .add(egui::DragValue::new(&mut policy.min_margin).speed(0.1))
                        .changed();
                    ui.label("max_risk");
                    policy_changed |= ui
                        .add(egui::DragValue::new(&mut policy.max_risk_score).speed(0.1))
                        .changed();
                    ui.label("max_hops");
                    policy_changed |= ui
                        .add(egui::DragValue::new(&mut policy.max_hops).speed(1.0))
                        .changed();
                });
                egui::ComboBox::from_label("priority_mode")
                    .selected_text(priority_mode_label(policy.priority_mode))
                    .show_ui(ui, |ui| {
                        policy_changed |= ui
                            .selectable_value(
                                &mut policy.priority_mode,
                                PriorityMode::Profit,
                                priority_mode_label(PriorityMode::Profit),
                            )
                            .changed();
                        policy_changed |= ui
                            .selectable_value(
                                &mut policy.priority_mode,
                                PriorityMode::Stability,
                                priority_mode_label(PriorityMode::Stability),
                            )
                            .changed();
                        policy_changed |= ui
                            .selectable_value(
                                &mut policy.priority_mode,
                                PriorityMode::Hybrid,
                                priority_mode_label(PriorityMode::Hybrid),
                            )
                            .changed();
                    });
                if policy_changed
                    && sim
                        .simulation
                        .update_ship_policy(ship_id, policy.clone())
                        .is_ok()
                {
                    kpi.record_manual_action(tick_now);
                    kpi.record_policy_edit(tick_now);
                }
                ui.label(format!(
                    "waypoints={}",
                    policy
                        .waypoints
                        .iter()
                        .map(|system_id| system_id.0.to_string())
                        .collect::<Vec<_>>()
                        .join(" -> ")
                ));
            } else {
                ui.label("Selected ship not found");
            }

            ui.separator();
            ui.heading("Manual vs Policy KPI");
            ui.monospace(format!(
                "manual/min={:.1} policy_edits/min={:.1} avg_route_hops={:.2}",
                snapshot.manual_actions_per_min,
                snapshot.policy_edits_per_min,
                snapshot.avg_route_hops_player
            ));
            ui.separator();
            ui.heading("Milestones");
            for milestone in &snapshot.milestones {
                ui.monospace(format!(
                    "{} current={:.2} target={:.2} completed={} cycle={}",
                    milestone_label(milestone),
                    milestone.current,
                    milestone.target,
                    milestone.completed,
                    milestone
                        .completed_cycle
                        .map(|cycle| cycle.to_string())
                        .unwrap_or_else(|| "-".to_string())
                ));
            }
        });

    panels.policies = open;
}
