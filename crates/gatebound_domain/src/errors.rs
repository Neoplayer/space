use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub enum ConfigError {
    Io(String),
    Parse(String),
    Validation(String),
}

impl Display for ConfigError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(v) | Self::Parse(v) | Self::Validation(v) => write!(f, "{v}"),
        }
    }
}

impl std::error::Error for ConfigError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreditError {
    LoanAlreadyActive,
    InvalidAmount,
    NoActiveLoan,
    InsufficientCapital,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissionOfferError {
    UnknownOffer,
    ExpiredOffer,
    InsufficientStock,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandError {
    UnknownShip,
    UnknownStation,
    InvalidAssignment,
    ShipBusy,
    NoRoute,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TradeError {
    UnknownShip,
    UnknownStation,
    InvalidAssignment,
    NotDocked,
    InvalidQuantity,
    InsufficientStock,
    InsufficientCapital,
    InsufficientCargo,
    CargoCapacityExceeded,
    CommodityMismatch,
    MissionCargoLocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageTransferError {
    UnknownShip,
    UnknownStation,
    InvalidAssignment,
    NotDocked,
    InvalidQuantity,
    InsufficientStoredCargo,
    InsufficientShipCargo,
    CargoCapacityExceeded,
    CommodityMismatch,
    MissionCargoLocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissionActionError {
    UnknownShip,
    UnknownMission,
    UnknownStation,
    NotDocked,
    InvalidQuantity,
    MissionState,
    WrongStation,
    InsufficientStoredCargo,
    InsufficientCargo,
    CargoCapacityExceeded,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RoutingError {
    Unreachable,
    MaxHopsExceeded,
}
