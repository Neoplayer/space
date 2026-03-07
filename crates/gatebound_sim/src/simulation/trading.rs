use gatebound_domain::*;

use super::state::Simulation;

impl Simulation {
    pub fn player_buy(
        &mut self,
        ship_id: ShipId,
        station_id: StationId,
        commodity: Commodity,
        quantity: f64,
    ) -> Result<TradeReceipt, TradeError> {
        if quantity <= 0.0 {
            return Err(TradeError::InvalidQuantity);
        }
        let Some(ship_snapshot) = self.ships.get(&ship_id).cloned() else {
            return Err(TradeError::UnknownShip);
        };
        if ship_snapshot.company_id != CompanyId(0) {
            return Err(TradeError::InvalidAssignment);
        }
        if !self
            .world
            .stations
            .iter()
            .any(|station| station.id == station_id)
        {
            return Err(TradeError::UnknownStation);
        }
        if !self.is_ship_docked_at(ship_id, station_id) {
            return Err(TradeError::NotDocked);
        }

        let Some(price) = self
            .markets
            .get(&station_id)
            .and_then(|book| book.goods.get(&commodity))
            .map(|state| state.price)
        else {
            return Err(TradeError::InsufficientStock);
        };
        let available = self
            .markets
            .get(&station_id)
            .and_then(|book| book.goods.get(&commodity))
            .map(|state| state.stock)
            .unwrap_or(0.0);
        if available + 1e-9 < quantity {
            return Err(TradeError::InsufficientStock);
        }

        if ship_snapshot.has_locked_cargo() {
            return Err(TradeError::MissionCargoLocked);
        }
        if ship_snapshot.remaining_capacity() + 1e-9 < quantity {
            return Err(TradeError::CargoCapacityExceeded);
        }

        let gross = quantity * price;
        let fee = gross * self.config.pressure.market_fee_rate;
        let total_cost = gross + fee;
        if self.capital + 1e-9 < total_cost {
            return Err(TradeError::InsufficientCapital);
        }

        if let Some(state) = self
            .markets
            .get_mut(&station_id)
            .and_then(|book| book.goods.get_mut(&commodity))
        {
            state.stock = (state.stock - quantity).max(0.0);
            state.cycle_outflow += quantity;
        }
        if let Some(ship) = self.ships.get_mut(&ship_id) {
            ship.upsert_lot(commodity, CargoSource::Spot, quantity);
        }
        self.capital -= total_cost;

        Ok(TradeReceipt {
            commodity,
            quantity,
            unit_price: price,
            gross,
            fee,
            net_cash_delta: -total_cost,
        })
    }

    pub fn player_sell(
        &mut self,
        ship_id: ShipId,
        station_id: StationId,
        commodity: Commodity,
        quantity: f64,
    ) -> Result<TradeReceipt, TradeError> {
        if quantity <= 0.0 {
            return Err(TradeError::InvalidQuantity);
        }
        let Some(ship_snapshot) = self.ships.get(&ship_id).cloned() else {
            return Err(TradeError::UnknownShip);
        };
        if ship_snapshot.company_id != CompanyId(0) {
            return Err(TradeError::InvalidAssignment);
        }
        if !self
            .world
            .stations
            .iter()
            .any(|station| station.id == station_id)
        {
            return Err(TradeError::UnknownStation);
        }
        if !self.is_ship_docked_at(ship_id, station_id) {
            return Err(TradeError::NotDocked);
        }

        if ship_snapshot.has_locked_cargo() {
            return Err(TradeError::MissionCargoLocked);
        }
        if ship_snapshot.spot_amount(commodity) + 1e-9 < quantity {
            return Err(TradeError::InsufficientCargo);
        }

        let price = self
            .markets
            .get(&station_id)
            .and_then(|book| book.goods.get(&commodity))
            .map(|state| state.price)
            .unwrap_or(0.0);
        let gross = quantity * price;
        let fee = gross * self.config.pressure.market_fee_rate;
        let net_revenue = gross - fee;

        if let Some(state) = self
            .markets
            .get_mut(&station_id)
            .and_then(|book| book.goods.get_mut(&commodity))
        {
            state.stock += quantity;
            state.cycle_inflow += quantity;
        }
        if let Some(ship) = self.ships.get_mut(&ship_id) {
            ship.remove_amount(commodity, CargoSource::Spot, quantity);
        }
        self.capital += net_revenue;

        Ok(TradeReceipt {
            commodity,
            quantity,
            unit_price: price,
            gross,
            fee,
            net_cash_delta: net_revenue,
        })
    }
}
