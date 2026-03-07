use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::BTreeMap;

use crate::ContractId;

const CARGO_EPSILON: f64 = 1e-9;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Commodity {
    Ore,
    Ice,
    Gas,
    Metal,
    Fuel,
    Parts,
    Electronics,
}

impl Commodity {
    pub const ALL: [Commodity; 7] = [
        Commodity::Ore,
        Commodity::Ice,
        Commodity::Gas,
        Commodity::Metal,
        Commodity::Fuel,
        Commodity::Parts,
        Commodity::Electronics,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CargoLoad {
    pub commodity: Commodity,
    pub amount: f64,
    pub source: CargoSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum CargoSource {
    Spot,
    Contract { contract_id: ContractId },
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct CargoManifest {
    lots: Vec<CargoLoad>,
}

impl CargoManifest {
    pub fn new(lots: Vec<CargoLoad>) -> Self {
        let mut manifest = Self { lots };
        manifest.normalize();
        manifest
    }

    pub fn is_empty(&self) -> bool {
        self.lots.is_empty()
    }

    pub fn lots(&self) -> &[CargoLoad] {
        &self.lots
    }

    pub fn total_amount(&self) -> f64 {
        self.lots.iter().map(|cargo| cargo.amount).sum()
    }

    pub fn remaining_capacity(&self, capacity: f64) -> f64 {
        (capacity - self.total_amount()).max(0.0)
    }

    pub fn amount_for(&self, commodity: Commodity, source: CargoSource) -> f64 {
        self.lots
            .iter()
            .find(|cargo| cargo.commodity == commodity && cargo.source == source)
            .map(|cargo| cargo.amount)
            .unwrap_or(0.0)
    }

    pub fn spot_amount(&self, commodity: Commodity) -> f64 {
        self.amount_for(commodity, CargoSource::Spot)
    }

    pub fn has_locked_cargo(&self) -> bool {
        self.lots
            .iter()
            .any(|cargo| cargo.source != CargoSource::Spot)
    }

    pub fn has_spot_cargo(&self) -> bool {
        self.lots
            .iter()
            .any(|cargo| cargo.source == CargoSource::Spot)
    }

    pub fn largest_spot_commodity(&self) -> Option<Commodity> {
        let mut totals = BTreeMap::new();
        for cargo in &self.lots {
            if cargo.source == CargoSource::Spot {
                *totals.entry(cargo.commodity).or_insert(0.0) += cargo.amount;
            }
        }
        totals
            .into_iter()
            .max_by(
                |(left_commodity, left_amount), (right_commodity, right_amount)| {
                    left_amount
                        .total_cmp(right_amount)
                        .then_with(|| left_commodity.cmp(right_commodity))
                },
            )
            .map(|(commodity, _)| commodity)
    }

    pub fn upsert_lot(&mut self, commodity: Commodity, source: CargoSource, amount: f64) {
        if amount <= CARGO_EPSILON {
            return;
        }

        if let Some(existing) = self
            .lots
            .iter_mut()
            .find(|cargo| cargo.commodity == commodity && cargo.source == source)
        {
            existing.amount += amount;
        } else {
            self.lots.push(CargoLoad {
                commodity,
                amount,
                source,
            });
        }
        self.normalize();
    }

    pub fn remove_amount(&mut self, commodity: Commodity, source: CargoSource, amount: f64) -> f64 {
        if amount <= CARGO_EPSILON {
            return 0.0;
        }

        let removed = self
            .lots
            .iter_mut()
            .find(|cargo| cargo.commodity == commodity && cargo.source == source)
            .map(|cargo| {
                let removed = cargo.amount.min(amount);
                cargo.amount = (cargo.amount - removed).max(0.0);
                removed
            })
            .unwrap_or(0.0);
        self.normalize();
        removed
    }

    fn normalize(&mut self) {
        let mut merged = BTreeMap::new();
        for cargo in &self.lots {
            if cargo.amount > CARGO_EPSILON {
                *merged.entry((cargo.commodity, cargo.source)).or_insert(0.0) += cargo.amount;
            }
        }
        self.lots = merged
            .into_iter()
            .filter_map(|((commodity, source), amount)| {
                (amount > CARGO_EPSILON).then_some(CargoLoad {
                    commodity,
                    amount,
                    source,
                })
            })
            .collect();
    }
}

impl From<CargoLoad> for CargoManifest {
    fn from(value: CargoLoad) -> Self {
        Self::new(vec![value])
    }
}

impl From<Vec<CargoLoad>> for CargoManifest {
    fn from(value: Vec<CargoLoad>) -> Self {
        Self::new(value)
    }
}

impl Serialize for CargoManifest {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.lots.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for CargoManifest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum CargoManifestRepr {
            Null(()),
            Single(CargoLoad),
            Many(Vec<CargoLoad>),
        }

        let repr = CargoManifestRepr::deserialize(deserializer)?;
        Ok(match repr {
            CargoManifestRepr::Null(_) => CargoManifest::default(),
            CargoManifestRepr::Single(load) => CargoManifest::from(load),
            CargoManifestRepr::Many(lots) => CargoManifest::from(lots),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cargo_manifest_merges_duplicate_lots_and_filters_empty_amounts() {
        let manifest = CargoManifest::from(vec![
            CargoLoad {
                commodity: Commodity::Fuel,
                amount: 4.0,
                source: CargoSource::Spot,
            },
            CargoLoad {
                commodity: Commodity::Fuel,
                amount: 1.5,
                source: CargoSource::Spot,
            },
            CargoLoad {
                commodity: Commodity::Ore,
                amount: 0.0,
                source: CargoSource::Spot,
            },
        ]);

        assert_eq!(
            manifest.lots(),
            &[CargoLoad {
                commodity: Commodity::Fuel,
                amount: 5.5,
                source: CargoSource::Spot,
            }]
        );
    }
}
