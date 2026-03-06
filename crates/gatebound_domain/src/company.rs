use serde::{Deserialize, Serialize};

use crate::CompanyId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompanyArchetype {
    Player,
    Hauler,
    Miner,
    Industrial,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Company {
    pub id: CompanyId,
    pub name: String,
    pub archetype: CompanyArchetype,
}
