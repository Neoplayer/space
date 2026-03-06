use super::*;

impl Simulation {
    #[cfg(feature = "test-support")]
    pub(crate) fn test_support_contract_offers_mut(&mut self) -> &mut BTreeMap<u64, ContractOffer> {
        &mut self.contract_offers
    }

    #[cfg(feature = "test-support")]
    pub(crate) fn test_support_ships_mut(&mut self) -> &mut BTreeMap<ShipId, Ship> {
        &mut self.ships
    }

    #[cfg(feature = "test-support")]
    pub(crate) fn test_support_markets_mut(&mut self) -> &mut BTreeMap<StationId, MarketBook> {
        &mut self.markets
    }

    #[cfg(feature = "test-support")]
    pub(crate) fn test_support_ship_idle_ticks_cycle_mut(&mut self) -> &mut BTreeMap<ShipId, u32> {
        &mut self.ship_idle_ticks_cycle
    }

    #[cfg(feature = "test-support")]
    pub(crate) fn test_support_ship_delay_ticks_cycle_mut(&mut self) -> &mut BTreeMap<ShipId, u32> {
        &mut self.ship_delay_ticks_cycle
    }

    #[cfg(feature = "test-support")]
    pub(crate) fn test_support_ship_runs_completed_mut(&mut self) -> &mut BTreeMap<ShipId, u32> {
        &mut self.ship_runs_completed
    }

    #[cfg(feature = "test-support")]
    pub(crate) fn test_support_ship_profit_earned_mut(&mut self) -> &mut BTreeMap<ShipId, f64> {
        &mut self.ship_profit_earned
    }

    #[cfg(feature = "test-support")]
    pub(crate) fn test_support_set_outstanding_debt(&mut self, value: f64) {
        self.outstanding_debt = value;
    }

    #[cfg(feature = "test-support")]
    pub(crate) fn test_support_set_reputation(&mut self, value: f64) {
        self.reputation = value;
    }

    #[cfg(feature = "test-support")]
    pub(crate) fn test_support_set_current_loan_interest_rate(&mut self, value: f64) {
        self.current_loan_interest_rate = value;
    }

    #[cfg(feature = "test-support")]
    pub(crate) fn test_support_set_recovery_events(&mut self, value: u32) {
        self.recovery_events = value;
    }

    #[cfg(feature = "test-support")]
    pub(crate) fn test_support_recovery_log_mut(&mut self) -> &mut Vec<RecoveryAction> {
        &mut self.recovery_log
    }
}
