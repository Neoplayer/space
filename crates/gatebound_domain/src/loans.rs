use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum LoanOfferId {
    Starter,
    Growth,
    Expansion,
}

impl LoanOfferId {
    pub const ALL: [LoanOfferId; 3] = [
        LoanOfferId::Starter,
        LoanOfferId::Growth,
        LoanOfferId::Expansion,
    ];

    pub fn label(self) -> &'static str {
        match self {
            LoanOfferId::Starter => "Starter",
            LoanOfferId::Growth => "Growth",
            LoanOfferId::Expansion => "Expansion",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LoanOffer {
    pub id: LoanOfferId,
    pub principal: f64,
    pub monthly_interest_rate: f64,
    pub term_months: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ActiveLoan {
    pub offer_id: LoanOfferId,
    pub principal_remaining: f64,
    pub monthly_interest_rate: f64,
    pub remaining_months: u32,
    pub next_payment: f64,
}
