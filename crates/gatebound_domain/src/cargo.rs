use serde::{Deserialize, Serialize};

use crate::ContractId;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CargoSource {
    Spot,
    Contract { contract_id: ContractId },
}
