use serde::{Deserialize, Serialize};

use crate::ConfigError;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FactionSeedConfig {
    pub name: String,
    pub color_rgb: [u8; 3],
    pub cluster_weight: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TimeUnitsConfig {
    pub tick_seconds: u32,
    pub cycle_ticks: u32,
    pub rolling_window_cycles: u32,
    pub day_ticks: u32,
    pub days_per_month: u32,
    pub months_per_year: u32,
    pub start_year: u32,
}

impl Default for TimeUnitsConfig {
    fn default() -> Self {
        Self {
            tick_seconds: 1,
            cycle_ticks: 60,
            rolling_window_cycles: 20,
            day_ticks: 200,
            days_per_month: 30,
            months_per_year: 12,
            start_year: 3500,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GalaxyGenConfig {
    pub seed: u64,
    pub system_count: u8,
    pub cluster_size_min: u8,
    pub cluster_size_max: u8,
    pub station_count_min: u8,
    pub station_count_max: u8,
    pub inter_cluster_gate_min: u8,
    pub inter_cluster_gate_max: u8,
    pub min_degree: u8,
    pub max_degree: u8,
    pub system_radius: f64,
    pub base_gate_capacity: f64,
    pub base_gate_travel_ticks: u32,
    pub factions: Vec<FactionSeedConfig>,
}

impl Default for GalaxyGenConfig {
    fn default() -> Self {
        Self {
            seed: 42,
            system_count: 25,
            cluster_size_min: 3,
            cluster_size_max: 5,
            station_count_min: 0,
            station_count_max: 4,
            inter_cluster_gate_min: 1,
            inter_cluster_gate_max: 3,
            min_degree: 2,
            max_degree: 4,
            system_radius: 100.0,
            base_gate_capacity: 8.0,
            base_gate_travel_ticks: 15,
            factions: vec![
                FactionSeedConfig {
                    name: "Aegis Collective".to_string(),
                    color_rgb: [64, 169, 255],
                    cluster_weight: 1.3,
                },
                FactionSeedConfig {
                    name: "Cinder Consortium".to_string(),
                    color_rgb: [255, 122, 72],
                    cluster_weight: 1.1,
                },
                FactionSeedConfig {
                    name: "Verdant League".to_string(),
                    color_rgb: [108, 214, 112],
                    cluster_weight: 0.9,
                },
                FactionSeedConfig {
                    name: "Helix Syndicate".to_string(),
                    color_rgb: [198, 108, 255],
                    cluster_weight: 0.8,
                },
                FactionSeedConfig {
                    name: "Solar Union".to_string(),
                    color_rgb: [255, 214, 82],
                    cluster_weight: 1.4,
                },
            ],
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
    pub gate_fee_per_jump: f64,
    pub market_fee_rate: f64,
    pub market_depth_per_cycle: f64,
    pub offer_refresh_cycles: u32,
    pub offer_ttl_cycles: u32,
    pub npc_company_starting_balances: Vec<f64>,
    pub milestone_capital_target: f64,
    pub milestone_market_share_target: f64,
    pub milestone_throughput_target_share: f64,
    pub milestone_reputation_target: f64,
    pub premium_offer_reputation_min: f64,
    pub sla_penalty_curve: Vec<f64>,
}

impl Default for EconomyPressureConfig {
    fn default() -> Self {
        Self {
            gate_fee_per_jump: 0.4,
            market_fee_rate: 0.05,
            market_depth_per_cycle: 16.0,
            offer_refresh_cycles: 2,
            offer_ttl_cycles: 6,
            npc_company_starting_balances: vec![1400.0, 1100.0, 1800.0, 2600.0, 3200.0, 2200.0],
            milestone_capital_target: 900.0,
            milestone_market_share_target: 0.25,
            milestone_throughput_target_share: 0.35,
            milestone_reputation_target: 0.85,
            premium_offer_reputation_min: 0.80,
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
        if self.time.day_ticks == 0 {
            return Err(ConfigError::Validation("day_ticks must be > 0".to_string()));
        }
        if self.time.days_per_month == 0 {
            return Err(ConfigError::Validation(
                "days_per_month must be > 0".to_string(),
            ));
        }
        if self.time.months_per_year == 0 {
            return Err(ConfigError::Validation(
                "months_per_year must be > 0".to_string(),
            ));
        }
        if self.galaxy.system_count == 0 {
            return Err(ConfigError::Validation(
                "system_count must be > 0".to_string(),
            ));
        }
        if self.galaxy.cluster_size_min == 0 || self.galaxy.cluster_size_max == 0 {
            return Err(ConfigError::Validation(
                "cluster_size_(min|max) must be > 0".to_string(),
            ));
        }
        if self.galaxy.cluster_size_min > self.galaxy.cluster_size_max {
            return Err(ConfigError::Validation(
                "cluster_size_min must be <= cluster_size_max".to_string(),
            ));
        }
        if self.galaxy.station_count_min > self.galaxy.station_count_max {
            return Err(ConfigError::Validation(
                "station_count_min must be <= station_count_max".to_string(),
            ));
        }
        if self.galaxy.inter_cluster_gate_min == 0 || self.galaxy.inter_cluster_gate_max == 0 {
            return Err(ConfigError::Validation(
                "inter_cluster_gate_(min|max) must be > 0".to_string(),
            ));
        }
        if self.galaxy.inter_cluster_gate_min > self.galaxy.inter_cluster_gate_max {
            return Err(ConfigError::Validation(
                "inter_cluster_gate_min must be <= inter_cluster_gate_max".to_string(),
            ));
        }
        if self.galaxy.min_degree > self.galaxy.max_degree {
            return Err(ConfigError::Validation(
                "min_degree must be <= max_degree".to_string(),
            ));
        }
        if self.galaxy.factions.len() != 5 {
            return Err(ConfigError::Validation(
                "galaxy factions must contain exactly 5 entries".to_string(),
            ));
        }
        if self
            .galaxy
            .factions
            .iter()
            .any(|faction| faction.name.trim().is_empty())
        {
            return Err(ConfigError::Validation(
                "galaxy faction names must not be empty".to_string(),
            ));
        }
        if self
            .galaxy
            .factions
            .iter()
            .any(|faction| faction.cluster_weight <= 0.0)
        {
            return Err(ConfigError::Validation(
                "galaxy faction weights must be > 0".to_string(),
            ));
        }
        let min_clusters = usize::from(self.galaxy.system_count)
            .div_ceil(usize::from(self.galaxy.cluster_size_max));
        let max_clusters =
            usize::from(self.galaxy.system_count) / usize::from(self.galaxy.cluster_size_min);
        if min_clusters > max_clusters {
            return Err(ConfigError::Validation(
                "system_count must be partitionable into cluster size bounds".to_string(),
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
        if self.pressure.npc_company_starting_balances.len() != 6 {
            return Err(ConfigError::Validation(
                "npc_company_starting_balances must contain exactly 6 entries".to_string(),
            ));
        }
        if self
            .pressure
            .npc_company_starting_balances
            .iter()
            .any(|balance| *balance < 0.0)
        {
            return Err(ConfigError::Validation(
                "npc_company_starting_balances must be >= 0".to_string(),
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
        Ok(())
    }
}
