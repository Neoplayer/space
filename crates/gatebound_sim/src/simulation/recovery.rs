use gatebound_domain::*;

use super::state::Simulation;

impl Simulation {
    pub(in crate::simulation) fn apply_upkeep(&mut self) {
        let ship_upkeep = self.config.pressure.ship_upkeep_per_tick * self.ships.len() as f64;
        let cycle_ticks = f64::from(self.config.time.cycle_ticks.max(1));
        let lease_upkeep = self
            .active_leases
            .iter()
            .map(|lease| lease.price_per_cycle / cycle_ticks)
            .sum::<f64>();
        self.capital -= ship_upkeep + lease_upkeep;
    }

    pub(in crate::simulation) fn apply_cycle_financial_pressure(&mut self) {
        if self.outstanding_debt > 0.0 {
            self.capital -= self.outstanding_debt * self.current_loan_interest_rate;
        }

        if self.capital > 0.0 && self.outstanding_debt > 0.0 {
            let repayment = (self.capital * 0.2).min(self.outstanding_debt);
            self.capital -= repayment;
            self.outstanding_debt -= repayment;
        }

        if self.capital < 0.0 {
            let emergency_loan = self
                .config
                .pressure
                .recovery_loan_base
                .max(-self.capital + self.config.pressure.recovery_loan_buffer);
            self.capital += emergency_loan;
            self.outstanding_debt += emergency_loan;
            let mut released_leases = 0_u32;
            if !self.active_leases.is_empty() {
                let mut indices = self.active_leases.iter().enumerate().collect::<Vec<_>>();
                indices.sort_by(|(_, a), (_, b)| b.price_per_cycle.total_cmp(&a.price_per_cycle));
                let mut to_remove = indices
                    .into_iter()
                    .take(2)
                    .map(|(idx, _)| idx)
                    .collect::<Vec<_>>();
                to_remove.sort_unstable_by(|a, b| b.cmp(a));
                for idx in to_remove {
                    if idx < self.active_leases.len() {
                        self.active_leases.remove(idx);
                        released_leases = released_leases.saturating_add(1);
                    }
                }
            }
            self.reputation =
                (self.reputation - self.config.pressure.recovery_reputation_penalty).max(0.0);
            self.current_loan_interest_rate = (self.current_loan_interest_rate
                + self.config.pressure.recovery_rate_hike)
                .min(self.config.pressure.recovery_rate_max);
            self.recovery_events = self.recovery_events.saturating_add(1);
            self.recovery_log.push(RecoveryAction {
                cycle: self.cycle,
                released_leases,
                capital_after: self.capital,
                debt_after: self.outstanding_debt,
            });
            if self.recovery_log.len() > 16 {
                let extra = self.recovery_log.len() - 16;
                self.recovery_log.drain(0..extra);
            }
        }
    }

    pub(in crate::simulation) fn record_ship_profit(&mut self, ship_id: ShipId, net_payout: f64) {
        let runs = self.ship_runs_completed.entry(ship_id).or_insert(0);
        *runs = runs.saturating_add(1);
        let profit = self.ship_profit_earned.entry(ship_id).or_insert(0.0);
        *profit += net_payout;
    }
}
