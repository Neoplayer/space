use gatebound_domain::{
    CargoSource, CommandError, Commodity, CompanyArchetype, CreditError, MilestoneId,
    MilestoneStatus, MissionActionError, MissionOfferError, MissionStatus, PriorityMode, ShipClass,
    ShipModuleSlot, ShipModuleStatus, ShipRole, StationProfile, StorageTransferError, TradeError,
};
use gatebound_sim::PopulationTrend;
pub(super) fn commodity_label(commodity: Commodity) -> &'static str {
    match commodity {
        Commodity::Ore => "Ore",
        Commodity::Ice => "Ice",
        Commodity::Gas => "Gas",
        Commodity::Metal => "Metal",
        Commodity::Fuel => "Fuel",
        Commodity::Parts => "Parts",
        Commodity::Electronics => "Electronics",
    }
}

pub(super) fn station_profile_label(profile: StationProfile) -> &'static str {
    match profile {
        StationProfile::Civilian => "Civilian",
        StationProfile::Industrial => "Industrial",
        StationProfile::Research => "Research",
    }
}

pub(super) fn ship_role_label(role: ShipRole) -> &'static str {
    match role {
        ShipRole::Player => "player",
        ShipRole::NpcTrade => "npc_trade",
    }
}

pub(super) fn company_archetype_label(archetype: CompanyArchetype) -> &'static str {
    match archetype {
        CompanyArchetype::Player => "player",
        CompanyArchetype::Hauler => "hauler",
        CompanyArchetype::Miner => "miner",
        CompanyArchetype::Industrial => "industrial",
    }
}

pub(super) fn ship_class_label(class: ShipClass) -> &'static str {
    match class {
        ShipClass::Courier => "Courier",
        ShipClass::Hauler => "Hauler",
        ShipClass::Miner => "Miner",
        ShipClass::Industrial => "Industrial",
    }
}

pub(super) fn ship_module_slot_label(slot: ShipModuleSlot) -> &'static str {
    match slot {
        ShipModuleSlot::Command => "Command",
        ShipModuleSlot::Drive => "Drive",
        ShipModuleSlot::Cargo => "Cargo",
        ShipModuleSlot::Utility => "Utility",
    }
}

pub(super) fn ship_module_status_label(status: ShipModuleStatus) -> &'static str {
    match status {
        ShipModuleStatus::Optimal => "optimal",
        ShipModuleStatus::Serviceable => "serviceable",
        ShipModuleStatus::Worn => "worn",
    }
}

pub(super) fn milestone_label(milestone: &MilestoneStatus) -> &'static str {
    match milestone.id {
        MilestoneId::Capital => "Capital",
        MilestoneId::MarketShare => "MarketShare",
        MilestoneId::ThroughputControl => "ThroughputControl",
        MilestoneId::Reputation => "Reputation",
    }
}

pub(super) fn priority_mode_label(mode: PriorityMode) -> &'static str {
    match mode {
        PriorityMode::Profit => "profit",
        PriorityMode::Stability => "stability",
        PriorityMode::Hybrid => "hybrid",
    }
}

pub(super) fn credit_error_label(err: CreditError) -> &'static str {
    match err {
        CreditError::LoanAlreadyActive => "loan_already_active",
        CreditError::InvalidAmount => "invalid_amount",
        CreditError::NoActiveLoan => "no_active_loan",
        CreditError::InsufficientCapital => "insufficient_capital",
    }
}

pub(super) fn command_error_label(err: CommandError) -> &'static str {
    match err {
        CommandError::UnknownShip => "unknown_ship",
        CommandError::UnknownStation => "unknown_station",
        CommandError::InvalidAssignment => "invalid_assignment",
        CommandError::ShipBusy => "ship_busy",
        CommandError::NoRoute => "no_route",
    }
}

pub(super) fn trade_error_label(err: TradeError) -> &'static str {
    match err {
        TradeError::UnknownShip => "unknown_ship",
        TradeError::UnknownStation => "unknown_station",
        TradeError::InvalidAssignment => "invalid_assignment",
        TradeError::NotDocked => "not_docked",
        TradeError::InvalidQuantity => "invalid_quantity",
        TradeError::InsufficientStock => "insufficient_stock",
        TradeError::InsufficientCapital => "insufficient_capital",
        TradeError::InsufficientCargo => "insufficient_cargo",
        TradeError::CargoCapacityExceeded => "cargo_capacity_exceeded",
        TradeError::CommodityMismatch => "commodity_mismatch",
        TradeError::MissionCargoLocked => "mission_cargo_locked",
    }
}

pub(super) fn storage_transfer_error_label(err: StorageTransferError) -> &'static str {
    match err {
        StorageTransferError::UnknownShip => "unknown_ship",
        StorageTransferError::UnknownStation => "unknown_station",
        StorageTransferError::InvalidAssignment => "invalid_assignment",
        StorageTransferError::NotDocked => "not_docked",
        StorageTransferError::InvalidQuantity => "invalid_quantity",
        StorageTransferError::InsufficientStoredCargo => "insufficient_stored_cargo",
        StorageTransferError::InsufficientShipCargo => "insufficient_ship_cargo",
        StorageTransferError::CargoCapacityExceeded => "cargo_capacity_exceeded",
        StorageTransferError::CommodityMismatch => "commodity_mismatch",
        StorageTransferError::MissionCargoLocked => "mission_cargo_locked",
    }
}

pub(super) fn mission_offer_error_label(err: MissionOfferError) -> &'static str {
    match err {
        MissionOfferError::UnknownOffer => "unknown_offer",
        MissionOfferError::ExpiredOffer => "expired_offer",
        MissionOfferError::InsufficientStock => "insufficient_stock",
    }
}

pub(super) fn mission_action_error_label(err: MissionActionError) -> &'static str {
    match err {
        MissionActionError::UnknownShip => "unknown_ship",
        MissionActionError::UnknownMission => "unknown_mission",
        MissionActionError::UnknownStation => "unknown_station",
        MissionActionError::NotDocked => "not_docked",
        MissionActionError::InvalidQuantity => "invalid_quantity",
        MissionActionError::MissionState => "mission_state",
        MissionActionError::WrongStation => "wrong_station",
        MissionActionError::InsufficientStoredCargo => "insufficient_stored_cargo",
        MissionActionError::InsufficientCargo => "insufficient_cargo",
        MissionActionError::CargoCapacityExceeded => "cargo_capacity_exceeded",
    }
}

pub(super) fn cargo_source_label(source: CargoSource) -> &'static str {
    match source {
        CargoSource::Spot => "spot",
        CargoSource::Mission { .. } => "mission",
    }
}

pub(super) fn mission_status_label(status: MissionStatus) -> &'static str {
    match status {
        MissionStatus::Accepted => "accepted",
        MissionStatus::InProgress => "in_progress",
        MissionStatus::Completed => "completed",
        MissionStatus::Cancelled => "cancelled",
    }
}

pub(super) fn format_population(population: f64) -> String {
    format!("{population:.0}")
}

pub(super) fn population_trend_label(trend: PopulationTrend) -> &'static str {
    match trend {
        PopulationTrend::Growing => "Rising",
        PopulationTrend::Stable => "Stable",
        PopulationTrend::Shrinking => "Falling",
    }
}
