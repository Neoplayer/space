use bevy_egui::egui;

use crate::features::finance::FinanceUiState;
use crate::runtime::sim::{SimResource, UiKpiTracker};

use super::labels::credit_error_label;
use super::messages::HudMessages;
use super::snapshot::HudSnapshot;

pub(super) fn render_finance_window(
    ctx: &egui::Context,
    open: &mut bool,
    snapshot: &HudSnapshot,
    finance_ui: &mut FinanceUiState,
    sim: &mut SimResource,
    kpi: &mut UiKpiTracker,
    messages: &mut HudMessages,
) {
    egui::Window::new("Finance").open(open).show(ctx, |ui| {
        ui.heading("Player Finance");
        ui.label(format!("Capital: {:.1}", snapshot.capital));
        ui.label(format!("Debt: {:.1}", snapshot.debt));
        ui.label(format!("Rate: {:.2}%", snapshot.interest_rate * 100.0));
        ui.label(format!("Reputation: {:.2}", snapshot.reputation));
        ui.separator();

        if let Some(active_loan) = snapshot.active_loan {
            finance_ui.pending_offer = None;
            ui.heading("Active Loan");
            ui.label(format!("Offer: {}", active_loan.offer_id.label()));
            ui.label(format!("Principal: {:.1}", active_loan.principal_remaining));
            ui.label(format!(
                "Months remaining: {}",
                active_loan.remaining_months
            ));
            ui.label(format!(
                "Next monthly payment: {:.1}",
                active_loan.next_payment
            ));
            ui.horizontal(|ui| {
                ui.label("Repay amount");
                ui.add(
                    egui::DragValue::new(&mut finance_ui.repayment_amount)
                        .speed(1.0)
                        .range(0.1..=10_000.0),
                );
            });
            ui.horizontal(|ui| {
                if ui.button("Repay Part").clicked() {
                    kpi.record_manual_action(sim.simulation.tick());
                    match sim.simulation.repay_credit(finance_ui.repayment_amount) {
                        Ok(()) => messages.push(format!(
                            "Repaid {:.1} toward active loan",
                            finance_ui
                                .repayment_amount
                                .min(active_loan.principal_remaining)
                        )),
                        Err(err) => {
                            messages.push(format!("Repay failed: {}", credit_error_label(err)))
                        }
                    }
                }
                if ui.button("Repay Full").clicked() {
                    kpi.record_manual_action(sim.simulation.tick());
                    match sim.simulation.repay_credit(active_loan.principal_remaining) {
                        Ok(()) => messages.push("Loan fully repaid".to_string()),
                        Err(err) => {
                            messages.push(format!("Repay failed: {}", credit_error_label(err)))
                        }
                    }
                }
            });
            ui.separator();
            ui.label("Credit offers unlock again after the current loan is closed.");
        } else {
            ui.heading("Credit Offers");
            for offer in &snapshot.loan_offers {
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.heading(offer.label);
                        ui.separator();
                        ui.label(format!("Amount: {:.1}", offer.principal));
                        ui.separator();
                        ui.label(format!(
                            "Rate: {:.2}%/month",
                            offer.monthly_interest_rate * 100.0
                        ));
                        ui.separator();
                        ui.label(format!("Term: {} mo", offer.term_months));
                        ui.separator();
                        ui.label(format!("Payment: {:.1}", offer.monthly_payment));
                    });
                    if finance_ui.pending_offer == Some(offer.id) {
                        ui.horizontal(|ui| {
                            ui.label("Confirm taking this credit?");
                            if ui.button("Confirm").clicked() {
                                kpi.record_manual_action(sim.simulation.tick());
                                match sim.simulation.take_credit(offer.id) {
                                    Ok(()) => {
                                        finance_ui.pending_offer = None;
                                        messages.push(format!(
                                            "Credit approved: {} +{:.1}",
                                            offer.label, offer.principal
                                        ));
                                    }
                                    Err(err) => messages.push(format!(
                                        "Credit failed: {}",
                                        credit_error_label(err)
                                    )),
                                }
                            }
                            if ui.button("Cancel").clicked() {
                                finance_ui.pending_offer = None;
                            }
                        });
                    } else if ui.button(format!("Take {}", offer.label)).clicked() {
                        finance_ui.pending_offer = Some(offer.id);
                    }
                });
            }
        }
    });
}
