use serde::{Deserialize, Serialize};

use crate::SystemId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum SlotType {
    Dock,
    Storage,
    Factory,
    Market,
}

impl SlotType {
    pub const ALL: [SlotType; 4] = [
        SlotType::Dock,
        SlotType::Storage,
        SlotType::Factory,
        SlotType::Market,
    ];
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LeasePosition {
    pub system_id: SystemId,
    pub slot_type: SlotType,
    pub cycles_remaining: u32,
    pub price_per_cycle: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LeaseMarketView {
    pub system_id: SystemId,
    pub slot_type: SlotType,
    pub available: u32,
    pub total: u32,
    pub price_per_cycle: f64,
}
