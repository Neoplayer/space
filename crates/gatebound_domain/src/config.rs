use serde::{Deserialize, Serialize};

use crate::ConfigError;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TimeUnitsConfig {
    pub tick_seconds: u32,
    pub cycle_ticks: u32,
    pub rolling_window_cycles: u32,
}

impl Default for TimeUnitsConfig {
    fn default() -> Self {
        Self {
            tick_seconds: 1,
            cycle_ticks: 60,
            rolling_window_cycles: 20,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GalaxyGenConfig {
    pub seed: u64,
    pub cluster_system_min: u8,
    pub cluster_system_max: u8,
    pub min_degree: u8,
    pub max_degree: u8,
    pub system_radius: f64,
    pub base_gate_capacity: f64,
    pub base_gate_travel_ticks: u32,
}

impl Default for GalaxyGenConfig {
    fn default() -> Self {
        Self {
            seed: 42,
            cluster_system_min: 3,
            cluster_system_max: 7,
            min_degree: 1,
            max_degree: 3,
            system_radius: 100.0,
            base_gate_capacity: 8.0,
            base_gate_travel_ticks: 15,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MarketConfig {
    pub k_stock: f64,
    pub k_flow: f64,
    pub delta_cap: f64,
    pub floor_mult: f64,
    pub ceiling_mult: f64,
}

impl Default for MarketConfig {
    fn default() -> Self {
        Self {
            k_stock: 0.08,
            k_flow: 0.04,
            delta_cap: 0.10,
            floor_mult: 0.25,
            ceiling_mult: 4.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EconomyPressureConfig {
    pub loan_interest_rate: f64,
    pub ship_upkeep_per_tick: f64,
    pub slot_lease_cost: f64,
    pub gate_fee_per_jump: f64,
    pub market_fee_rate: f64,
    pub market_depth_per_cycle: f64,
    pub offer_refresh_cycles: u32,
    pub offer_ttl_cycles: u32,
    pub milestone_capital_target: f64,
    pub milestone_market_share_target: f64,
    pub milestone_throughput_target_share: f64,
    pub milestone_reputation_target: f64,
    pub premium_offer_reputation_min: f64,
    pub lease_price_throughput_k: f64,
    pub lease_price_gate_k: f64,
    pub lease_price_congestion_k: f64,
    pub lease_price_min_mult: f64,
    pub lease_price_max_mult: f64,
    pub recovery_loan_base: f64,
    pub recovery_loan_buffer: f64,
    pub recovery_reputation_penalty: f64,
    pub recovery_rate_hike: f64,
    pub recovery_rate_max: f64,
    pub sla_penalty_curve: Vec<f64>,
}

impl Default for EconomyPressureConfig {
    fn default() -> Self {
        Self {
            loan_interest_rate: 0.02,
            ship_upkeep_per_tick: 0.5,
            slot_lease_cost: 2.0,
            gate_fee_per_jump: 0.4,
            market_fee_rate: 0.05,
            market_depth_per_cycle: 16.0,
            offer_refresh_cycles: 2,
            offer_ttl_cycles: 6,
            milestone_capital_target: 900.0,
            milestone_market_share_target: 0.25,
            milestone_throughput_target_share: 0.35,
            milestone_reputation_target: 0.85,
            premium_offer_reputation_min: 0.80,
            lease_price_throughput_k: 0.60,
            lease_price_gate_k: 0.35,
            lease_price_congestion_k: 0.80,
            lease_price_min_mult: 0.70,
            lease_price_max_mult: 2.50,
            recovery_loan_base: 120.0,
            recovery_loan_buffer: 20.0,
            recovery_reputation_penalty: 0.12,
            recovery_rate_hike: 0.01,
            recovery_rate_max: 0.12,
            sla_penalty_curve: vec![1.0, 1.3, 1.7, 2.2, 2.8],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct RuntimeConfig {
    pub time: TimeUnitsConfig,
    pub galaxy: GalaxyGenConfig,
    pub market: MarketConfig,
    pub pressure: EconomyPressureConfig,
}

impl RuntimeConfig {
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.time.tick_seconds == 0 {
            return Err(ConfigError::Validation(
                "tick_seconds must be > 0".to_string(),
            ));
        }
        if self.time.cycle_ticks == 0 {
            return Err(ConfigError::Validation(
                "cycle_ticks must be > 0".to_string(),
            ));
        }
        if self.time.rolling_window_cycles == 0 {
            return Err(ConfigError::Validation(
                "rolling_window_cycles must be > 0".to_string(),
            ));
        }
        if self.galaxy.cluster_system_min == 0 || self.galaxy.cluster_system_max == 0 {
            return Err(ConfigError::Validation(
                "cluster_system_(min|max) must be > 0".to_string(),
            ));
        }
        if self.galaxy.cluster_system_min > self.galaxy.cluster_system_max {
            return Err(ConfigError::Validation(
                "cluster_system_min must be <= cluster_system_max".to_string(),
            ));
        }
        if self.galaxy.min_degree > self.galaxy.max_degree {
            return Err(ConfigError::Validation(
                "min_degree must be <= max_degree".to_string(),
            ));
        }
        if self.market.delta_cap <= 0.0 {
            return Err(ConfigError::Validation("delta_cap must be > 0".to_string()));
        }
        if self.market.floor_mult <= 0.0 || self.market.ceiling_mult <= self.market.floor_mult {
            return Err(ConfigError::Validation(
                "market floor/ceiling multipliers invalid".to_string(),
            ));
        }
        if self.pressure.sla_penalty_curve.is_empty() {
            return Err(ConfigError::Validation(
                "sla_penalty_curve must not be empty".to_string(),
            ));
        }
        if self.pressure.market_fee_rate < 0.0 || self.pressure.market_fee_rate >= 1.0 {
            return Err(ConfigError::Validation(
                "market_fee_rate must be in [0,1)".to_string(),
            ));
        }
        if self.pressure.market_depth_per_cycle <= 0.0 {
            return Err(ConfigError::Validation(
                "market_depth_per_cycle must be > 0".to_string(),
            ));
        }
        if self.pressure.offer_refresh_cycles == 0 || self.pressure.offer_ttl_cycles == 0 {
            return Err(ConfigError::Validation(
                "offer cycles must be > 0".to_string(),
            ));
        }
        if self.pressure.milestone_throughput_target_share < 0.0
            || self.pressure.milestone_throughput_target_share > 1.0
        {
            return Err(ConfigError::Validation(
                "milestone_throughput_target_share must be in [0,1]".to_string(),
            ));
        }
        if self.pressure.premium_offer_reputation_min < 0.0
            || self.pressure.premium_offer_reputation_min > 1.0
        {
            return Err(ConfigError::Validation(
                "premium_offer_reputation_min must be in [0,1]".to_string(),
            ));
        }
        if self.pressure.lease_price_min_mult <= 0.0
            || self.pressure.lease_price_max_mult < self.pressure.lease_price_min_mult
        {
            return Err(ConfigError::Validation(
                "lease price multipliers invalid".to_string(),
            ));
        }
        if self.pressure.recovery_rate_hike < 0.0
            || self.pressure.recovery_rate_max < self.pressure.loan_interest_rate
        {
            return Err(ConfigError::Validation(
                "recovery rate bounds invalid".to_string(),
            ));
        }
        Ok(())
    }
}
