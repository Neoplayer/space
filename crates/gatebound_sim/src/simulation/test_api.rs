use super::*;

impl Simulation {
    #[cfg(feature = "test-support")]
    pub(crate) fn test_support_mission_offers_mut(&mut self) -> &mut BTreeMap<u64, MissionOffer> {
        &mut self.mission_offers
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
}
