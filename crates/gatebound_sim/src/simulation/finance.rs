use gatebound_domain::*;

use super::state::Simulation;

const LOAN_OFFERS: [LoanOffer; 3] = [
    LoanOffer {
        id: LoanOfferId::Starter,
        principal: 100.0,
        monthly_interest_rate: 0.02,
        term_months: 3,
    },
    LoanOffer {
        id: LoanOfferId::Growth,
        principal: 250.0,
        monthly_interest_rate: 0.03,
        term_months: 6,
    },
    LoanOffer {
        id: LoanOfferId::Expansion,
        principal: 500.0,
        monthly_interest_rate: 0.04,
        term_months: 12,
    },
];

impl Simulation {
    pub fn loan_offers(&self) -> Vec<LoanOffer> {
        LOAN_OFFERS.to_vec()
    }

    pub fn take_credit(&mut self, offer_id: LoanOfferId) -> Result<(), CreditError> {
        if self.active_loan.is_some() {
            return Err(CreditError::LoanAlreadyActive);
        }

        let Some(offer) = LOAN_OFFERS
            .iter()
            .copied()
            .find(|offer| offer.id == offer_id)
        else {
            return Err(CreditError::NoActiveLoan);
        };

        self.capital += offer.principal;
        self.active_loan = Some(ActiveLoan {
            offer_id: offer.id,
            principal_remaining: offer.principal,
            monthly_interest_rate: offer.monthly_interest_rate,
            remaining_months: offer.term_months,
            next_payment: annuity_payment(
                offer.principal,
                offer.monthly_interest_rate,
                offer.term_months,
            ),
        });
        self.sync_credit_state();
        Ok(())
    }

    pub fn repay_credit(&mut self, amount: f64) -> Result<(), CreditError> {
        if amount <= 0.0 {
            return Err(CreditError::InvalidAmount);
        }

        let Some(mut loan) = self.active_loan else {
            return Err(CreditError::NoActiveLoan);
        };

        let payment = amount.min(loan.principal_remaining);
        if self.capital + 1e-9 < payment {
            return Err(CreditError::InsufficientCapital);
        }

        self.capital -= payment;
        loan.principal_remaining = (loan.principal_remaining - payment).max(0.0);
        if loan.principal_remaining <= 1e-9 {
            self.active_loan = None;
            self.sync_credit_state();
            return Ok(());
        }

        loan.next_payment = annuity_payment(
            loan.principal_remaining,
            loan.monthly_interest_rate,
            loan.remaining_months.max(1),
        );
        self.active_loan = Some(loan);
        self.sync_credit_state();
        Ok(())
    }

    pub(in crate::simulation) fn step_month(&mut self) {
        let Some(mut loan) = self.active_loan else {
            self.sync_credit_state();
            return;
        };

        loan.principal_remaining *= 1.0 + loan.monthly_interest_rate;
        self.capital -= loan.next_payment;
        loan.principal_remaining = (loan.principal_remaining - loan.next_payment).max(0.0);
        loan.remaining_months = loan.remaining_months.saturating_sub(1);

        if loan.principal_remaining <= 1e-9 || loan.remaining_months == 0 {
            self.active_loan = None;
            self.sync_credit_state();
            return;
        }

        loan.next_payment = annuity_payment(
            loan.principal_remaining,
            loan.monthly_interest_rate,
            loan.remaining_months,
        );
        self.active_loan = Some(loan);
        self.sync_credit_state();
    }

    pub(in crate::simulation) fn month_ticks(&self) -> u64 {
        u64::from(self.config.time.day_ticks.max(1))
            .saturating_mul(u64::from(self.config.time.days_per_month.max(1)))
    }

    pub(in crate::simulation) fn record_ship_profit(&mut self, ship_id: ShipId, net_payout: f64) {
        let runs = self.ship_runs_completed.entry(ship_id).or_insert(0);
        *runs = runs.saturating_add(1);
        let profit = self.ship_profit_earned.entry(ship_id).or_insert(0.0);
        *profit += net_payout;
    }

    pub(in crate::simulation) fn sync_credit_state(&mut self) {
        if let Some(loan) = self.active_loan {
            self.outstanding_debt = loan.principal_remaining;
            self.current_loan_interest_rate = loan.monthly_interest_rate;
        } else {
            self.outstanding_debt = 0.0;
            self.current_loan_interest_rate = 0.0;
        }
    }
}

pub(super) fn annuity_payment(principal: f64, monthly_rate: f64, months: u32) -> f64 {
    if months == 0 {
        return 0.0;
    }
    if monthly_rate.abs() < 1e-12 {
        return principal / f64::from(months);
    }

    principal * monthly_rate / (1.0 - (1.0 + monthly_rate).powf(-f64::from(months)))
}
