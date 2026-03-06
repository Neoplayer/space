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
        if !self.world.stations.iter().any(|station| station.id == station_id) {
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

        if let Some(cargo) = ship_snapshot.cargo {
            if cargo.commodity != commodity {
                return Err(TradeError::CommodityMismatch);
            }
            if cargo.source != CargoSource::Spot {
                return Err(TradeError::ContractCargoLocked);
            }
            if cargo.amount + quantity > ship_snapshot.cargo_capacity + 1e-9 {
                return Err(TradeError::CargoCapacityExceeded);
            }
        } else if quantity > ship_snapshot.cargo_capacity + 1e-9 {
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
            match &mut ship.cargo {
                Some(cargo) => cargo.amount += quantity,
                None => {
                    ship.cargo = Some(CargoLoad {
                        commodity,
                        amount: quantity,
                        source: CargoSource::Spot,
                    });
                }
            }
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
        if !self.world.stations.iter().any(|station| station.id == station_id) {
            return Err(TradeError::UnknownStation);
        }
        if !self.is_ship_docked_at(ship_id, station_id) {
            return Err(TradeError::NotDocked);
        }

        let Some(cargo) = ship_snapshot.cargo else {
            return Err(TradeError::InsufficientCargo);
        };
        if cargo.commodity != commodity {
            return Err(TradeError::CommodityMismatch);
        }
        if cargo.source != CargoSource::Spot {
            return Err(TradeError::ContractCargoLocked);
        }
        if cargo.amount + 1e-9 < quantity {
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
            if let Some(ship_cargo) = &mut ship.cargo {
                ship_cargo.amount = (ship_cargo.amount - quantity).max(0.0);
                if ship_cargo.amount <= 1e-9 {
                    ship.cargo = None;
                }
            }
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
