use bevy::prelude::*;
use gatebound_domain::LoanOfferId;

#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct FinanceUiState {
    pub pending_offer: Option<LoanOfferId>,
    pub repayment_amount: f64,
}

impl Default for FinanceUiState {
    fn default() -> Self {
        Self {
            pending_offer: None,
            repayment_amount: 25.0,
        }
    }
}

pub struct FinanceFeaturePlugin;

impl Plugin for FinanceFeaturePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FinanceUiState>();
    }
}
