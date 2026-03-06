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
pub enum LeaseError {
    NoCapacity,
    InvalidCycles,
    UnknownSystem,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OfferError {
    UnknownOffer,
    ExpiredOffer,
    ShipBusy,
    InvalidAssignment,
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
    ContractCargoLocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContractActionError {
    UnknownShip,
    UnknownContract,
    InvalidAssignment,
    NotAssignedShip,
    NotDocked,
    InvalidQuantity,
    ContractState,
    InsufficientStock,
    InsufficientCargo,
    CargoCapacityExceeded,
    CommodityMismatch,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RoutingError {
    Unreachable,
    MaxHopsExceeded,
}
