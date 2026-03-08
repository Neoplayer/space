use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::{Commodity, SystemId};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MarketIntel {
    pub system_id: SystemId,
    pub observed_tick: u64,
    pub staleness_ticks: u64,
    pub confidence: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TradeReceipt {
    pub commodity: Commodity,
    pub quantity: f64,
    pub unit_price: f64,
    pub gross: f64,
    pub fee: f64,
    pub net_cash_delta: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketState {
    pub base_price: f64,
    pub price: f64,
    pub stock: f64,
    pub base_target_stock: f64,
    pub target_stock: f64,
    pub cycle_inflow: f64,
    pub cycle_outflow: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketBook {
    pub goods: BTreeMap<Commodity, MarketState>,
}
