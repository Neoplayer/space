use gatebound_domain::{
    CargoSource, CommandError, Commodity, CompanyArchetype, ContractActionError, ContractProgress,
    CreditError, MilestoneId, MilestoneStatus, OfferError, OfferProblemTag, PriorityMode,
    ShipClass, ShipModuleSlot, ShipModuleStatus, ShipRole, StationProfile, TradeError,
};

use crate::runtime::sim::OfferSortMode;
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
        ShipRole::PlayerContract => "player_contract",
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

pub(super) fn sort_mode_label(mode: OfferSortMode) -> &'static str {
    match mode {
        OfferSortMode::MarginDesc => "Margin desc",
        OfferSortMode::RiskAsc => "Risk asc",
        OfferSortMode::EtaAsc => "ETA asc",
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

pub(super) fn offer_error_label(err: OfferError) -> &'static str {
    match err {
        OfferError::UnknownOffer => "unknown_offer",
        OfferError::ExpiredOffer => "expired_offer",
        OfferError::ShipBusy => "ship_busy",
        OfferError::InvalidAssignment => "invalid_assignment",
        OfferError::InsufficientStock => "insufficient_stock",
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
        TradeError::ContractCargoLocked => "contract_cargo_locked",
    }
}

pub(super) fn contract_action_error_label(err: ContractActionError) -> &'static str {
    match err {
        ContractActionError::UnknownShip => "unknown_ship",
        ContractActionError::UnknownContract => "unknown_contract",
        ContractActionError::InvalidAssignment => "invalid_assignment",
        ContractActionError::NotAssignedShip => "not_assigned_ship",
        ContractActionError::NotDocked => "not_docked",
        ContractActionError::InvalidQuantity => "invalid_quantity",
        ContractActionError::ContractState => "contract_state",
        ContractActionError::InsufficientStock => "insufficient_stock",
        ContractActionError::InsufficientCargo => "insufficient_cargo",
        ContractActionError::CargoCapacityExceeded => "cargo_capacity_exceeded",
        ContractActionError::CommodityMismatch => "commodity_mismatch",
    }
}

pub(super) fn contract_progress_label(progress: ContractProgress) -> &'static str {
    match progress {
        ContractProgress::AwaitPickup => "await_pickup",
        ContractProgress::InTransit => "in_transit",
        ContractProgress::Completed => "completed",
        ContractProgress::Failed => "failed",
    }
}

pub(super) fn cargo_source_label(source: CargoSource) -> &'static str {
    match source {
        CargoSource::Spot => "spot",
        CargoSource::Contract { .. } => "contract",
    }
}

pub(super) fn problem_label(problem: OfferProblemTag) -> &'static str {
    match problem {
        OfferProblemTag::HighRisk => "high_risk",
        OfferProblemTag::CongestedRoute => "congested_route",
        OfferProblemTag::LowMargin => "low_margin",
        OfferProblemTag::FuelVolatility => "fuel_volatility",
    }
}
