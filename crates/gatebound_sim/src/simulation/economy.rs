use gatebound_domain::*;

use super::state::Simulation;

impl Simulation {
    pub(in crate::simulation) fn run_economy_flow(&mut self) {
        let fuel_shock_factor = self
            .modifiers
            .iter()
            .filter(|m| m.risk == RiskStageA::FuelShock)
            .map(|m| m.magnitude)
            .fold(1.0_f64, f64::min);
        let station_ids: Vec<StationId> = self
            .world
            .stations
            .iter()
            .map(|station| station.id)
            .collect();

        for station_id in &station_ids {
            let Some(profile) = self.station_profile(*station_id) else {
                continue;
            };
            if let Some(book) = self.markets.get_mut(station_id) {
                for commodity in Commodity::ALL {
                    let amount = profile_production(profile, commodity);
                    if amount <= 0.0 {
                        continue;
                    }
                    if let Some(state) = book.goods.get_mut(&commodity) {
                        state.stock += amount;
                        state.cycle_inflow += amount;
                    }
                }
            }
        }

        for station_id in &station_ids {
            let Some(profile) = self.station_profile(*station_id) else {
                continue;
            };
            let fuel_mult = recipe_output_multiplier(profile, Commodity::Fuel) * fuel_shock_factor;
            self.process_station_recipe(
                *station_id,
                &[(Commodity::Ore, 1.6), (Commodity::Fuel, 0.2)],
                (Commodity::Metal, 1.0),
                recipe_output_multiplier(profile, Commodity::Metal),
            );
            self.process_station_recipe(
                *station_id,
                &[(Commodity::Gas, 1.2), (Commodity::Ice, 0.8)],
                (Commodity::Fuel, 1.0),
                fuel_mult,
            );
            self.process_station_recipe(
                *station_id,
                &[(Commodity::Metal, 1.0), (Commodity::Fuel, 0.5)],
                (Commodity::Parts, 0.8),
                recipe_output_multiplier(profile, Commodity::Parts),
            );
            self.process_station_recipe(
                *station_id,
                &[(Commodity::Parts, 0.9), (Commodity::Fuel, 0.6)],
                (Commodity::Electronics, 0.6),
                recipe_output_multiplier(profile, Commodity::Electronics),
            );
        }

        for station_id in &station_ids {
            let Some(profile) = self.station_profile(*station_id) else {
                continue;
            };
            if let Some(book) = self.markets.get_mut(station_id) {
                for commodity in Commodity::ALL {
                    let amount = profile_consumption(profile, commodity);
                    if amount <= 0.0 {
                        continue;
                    }
                    if let Some(state) = book.goods.get_mut(&commodity) {
                        state.stock = (state.stock - amount).max(0.0);
                        state.cycle_outflow += amount;
                    }
                }
            }
        }
    }

    pub(in crate::simulation) fn station_profile(
        &self,
        station_id: StationId,
    ) -> Option<StationProfile> {
        self.world
            .stations
            .iter()
            .find(|station| station.id == station_id)
            .map(|station| station.profile)
    }

    pub(in crate::simulation) fn process_station_recipe(
        &mut self,
        station_id: StationId,
        inputs: &[(Commodity, f64)],
        output: (Commodity, f64),
        multiplier: f64,
    ) {
        if multiplier <= 0.0 {
            return;
        }
        let Some(book) = self.markets.get(&station_id) else {
            return;
        };
        let mut limiting = 1.0_f64;
        for (commodity, amount) in inputs {
            let required = (amount * multiplier).max(0.0);
            if required <= 0.0 {
                continue;
            }
            let available = book
                .goods
                .get(commodity)
                .map(|state| state.stock)
                .unwrap_or(0.0);
            limiting = limiting.min((available / required).clamp(0.0, 1.0));
        }
        if limiting <= 0.0 {
            return;
        }
        let input_deltas = inputs
            .iter()
            .map(|(commodity, amount)| (*commodity, amount * multiplier * limiting))
            .collect::<Vec<_>>();
        let output_amount = output.1 * multiplier * limiting;
        if let Some(book) = self.markets.get_mut(&station_id) {
            for (commodity, amount) in input_deltas {
                if let Some(state) = book.goods.get_mut(&commodity) {
                    state.stock = (state.stock - amount).max(0.0);
                    state.cycle_outflow += amount;
                }
            }
            if let Some(state) = book.goods.get_mut(&output.0) {
                state.stock += output_amount;
                state.cycle_inflow += output_amount;
            }
        }
    }

    pub(in crate::simulation) fn capture_previous_cycle_prices(&mut self) {
        self.previous_cycle_prices.clear();
        for (system_id, book) in &self.markets {
            for commodity in Commodity::ALL {
                if let Some(state) = book.goods.get(&commodity) {
                    self.previous_cycle_prices
                        .insert((*system_id, commodity), state.price);
                }
            }
        }
    }

    pub(in crate::simulation) fn update_market_prices(&mut self) {
        for market in self.markets.values_mut() {
            for state in market.goods.values_mut() {
                let imbalance = (state.target_stock - state.stock) / state.target_stock.max(1.0);
                let flow_pressure =
                    (state.cycle_outflow - state.cycle_inflow) / state.target_stock.max(1.0);
                let raw_delta = self.config.market.k_stock * imbalance
                    + self.config.market.k_flow * flow_pressure;
                let delta = raw_delta
                    .max(-self.config.market.delta_cap)
                    .min(self.config.market.delta_cap);
                let floor = state.base_price * self.config.market.floor_mult;
                let ceil = state.base_price * self.config.market.ceiling_mult;
                state.price = (state.price * (1.0 + delta)).clamp(floor, ceil);
                state.cycle_inflow = 0.0;
                state.cycle_outflow = 0.0;
            }
        }
    }

    pub(in crate::simulation) fn average_price_index(&self) -> f64 {
        let mut sum = 0.0;
        let mut count = 0_usize;
        for market in self.markets.values() {
            for state in market.goods.values() {
                sum += state.price / state.base_price;
                count += 1;
            }
        }
        if count == 0 { 1.0 } else { sum / count as f64 }
    }
}

pub(super) fn profile_production(profile: StationProfile, commodity: Commodity) -> f64 {
    match profile {
        StationProfile::Industrial => match commodity {
            Commodity::Ore => 2.0,
            Commodity::Gas => 1.2,
            Commodity::Ice => 0.4,
            _ => 0.0,
        },
        StationProfile::Civilian => match commodity {
            Commodity::Ore => 0.2,
            Commodity::Gas => 0.3,
            Commodity::Ice => 1.4,
            _ => 0.0,
        },
        StationProfile::Research => match commodity {
            Commodity::Ore => 0.1,
            Commodity::Gas => 0.8,
            Commodity::Ice => 0.6,
            _ => 0.0,
        },
    }
}

pub(super) fn recipe_output_multiplier(profile: StationProfile, output: Commodity) -> f64 {
    match profile {
        StationProfile::Industrial => match output {
            Commodity::Metal => 1.3,
            Commodity::Fuel => 1.1,
            Commodity::Parts => 1.0,
            Commodity::Electronics => 0.6,
            _ => 1.0,
        },
        StationProfile::Civilian => match output {
            Commodity::Metal => 0.4,
            Commodity::Fuel => 0.8,
            Commodity::Parts => 0.4,
            Commodity::Electronics => 0.7,
            _ => 1.0,
        },
        StationProfile::Research => match output {
            Commodity::Metal => 0.2,
            Commodity::Fuel => 0.9,
            Commodity::Parts => 0.8,
            Commodity::Electronics => 1.4,
            _ => 1.0,
        },
    }
}

pub(super) fn profile_consumption(profile: StationProfile, commodity: Commodity) -> f64 {
    match profile {
        StationProfile::Civilian => match commodity {
            Commodity::Ice => 1.2,
            Commodity::Fuel => 1.0,
            Commodity::Electronics => 1.1,
            _ => 0.2,
        },
        StationProfile::Industrial => match commodity {
            Commodity::Ore => 0.9,
            Commodity::Metal => 1.1,
            Commodity::Parts => 1.0,
            Commodity::Fuel => 0.9,
            _ => 0.2,
        },
        StationProfile::Research => match commodity {
            Commodity::Electronics => 1.3,
            Commodity::Parts => 0.8,
            Commodity::Fuel => 0.9,
            _ => 0.2,
        },
    }
}
