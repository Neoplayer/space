use super::*;

const STORAGE_EPSILON: f64 = 1e-9;

impl Simulation {
    pub fn player_unload_to_station_storage(
        &mut self,
        ship_id: ShipId,
        station_id: StationId,
        quantity: f64,
    ) -> Result<(), StorageTransferError> {
        if quantity <= 0.0 {
            return Err(StorageTransferError::InvalidQuantity);
        }
        let Some(ship_snapshot) = self.ships.get(&ship_id).cloned() else {
            return Err(StorageTransferError::UnknownShip);
        };
        if ship_snapshot.company_id != CompanyId(0) {
            return Err(StorageTransferError::InvalidAssignment);
        }
        if !self.station_exists(station_id) {
            return Err(StorageTransferError::UnknownStation);
        }
        if !self.is_ship_docked_at(ship_id, station_id) {
            return Err(StorageTransferError::NotDocked);
        }

        let Some(cargo) = ship_snapshot.cargo else {
            return Err(StorageTransferError::InsufficientShipCargo);
        };
        if cargo.source != CargoSource::Spot {
            return Err(StorageTransferError::ContractCargoLocked);
        }
        if cargo.amount + STORAGE_EPSILON < quantity {
            return Err(StorageTransferError::InsufficientShipCargo);
        }

        if let Some(ship) = self.ships.get_mut(&ship_id) {
            if let Some(ship_cargo) = &mut ship.cargo {
                ship_cargo.amount = (ship_cargo.amount - quantity).max(0.0);
                if ship_cargo.amount <= STORAGE_EPSILON {
                    ship.cargo = None;
                }
            }
        }
        *self
            .player_station_storage
            .entry(station_id)
            .or_default()
            .entry(cargo.commodity)
            .or_insert(0.0) += quantity;

        Ok(())
    }

    pub fn player_load_from_station_storage(
        &mut self,
        ship_id: ShipId,
        station_id: StationId,
        commodity: Commodity,
        quantity: f64,
    ) -> Result<(), StorageTransferError> {
        if quantity <= 0.0 {
            return Err(StorageTransferError::InvalidQuantity);
        }
        let Some(ship_snapshot) = self.ships.get(&ship_id).cloned() else {
            return Err(StorageTransferError::UnknownShip);
        };
        if ship_snapshot.company_id != CompanyId(0) {
            return Err(StorageTransferError::InvalidAssignment);
        }
        if !self.station_exists(station_id) {
            return Err(StorageTransferError::UnknownStation);
        }
        if !self.is_ship_docked_at(ship_id, station_id) {
            return Err(StorageTransferError::NotDocked);
        }

        let available = self
            .player_station_storage
            .get(&station_id)
            .and_then(|goods| goods.get(&commodity))
            .copied()
            .unwrap_or(0.0);
        if available + STORAGE_EPSILON < quantity {
            return Err(StorageTransferError::InsufficientStoredCargo);
        }

        if let Some(cargo) = ship_snapshot.cargo {
            if cargo.source != CargoSource::Spot {
                return Err(StorageTransferError::ContractCargoLocked);
            }
            if cargo.commodity != commodity {
                return Err(StorageTransferError::CommodityMismatch);
            }
            if cargo.amount + quantity > ship_snapshot.cargo_capacity + STORAGE_EPSILON {
                return Err(StorageTransferError::CargoCapacityExceeded);
            }
        } else if quantity > ship_snapshot.cargo_capacity + STORAGE_EPSILON {
            return Err(StorageTransferError::CargoCapacityExceeded);
        }

        let mut remove_station_entry = false;
        if let Some(goods) = self.player_station_storage.get_mut(&station_id) {
            if let Some(stored_amount) = goods.get_mut(&commodity) {
                *stored_amount = (*stored_amount - quantity).max(0.0);
                if *stored_amount <= STORAGE_EPSILON {
                    goods.remove(&commodity);
                }
            }
            remove_station_entry = goods.is_empty();
        }
        if remove_station_entry {
            self.player_station_storage.remove(&station_id);
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

        Ok(())
    }

    fn station_exists(&self, station_id: StationId) -> bool {
        self.world
            .stations
            .iter()
            .any(|station| station.id == station_id)
    }
}
