use bevy_egui::egui;
use gatebound_domain::{CargoLoad, CargoSource, Commodity};
use gatebound_sim::TradePriceTone;

use super::labels::{cargo_source_label, commodity_label};

pub(super) fn tab_button(label: &'static str, selected: bool) -> egui::Button<'static> {
    let mut button = egui::Button::new(label);
    if selected {
        button = button.fill(egui::Color32::from_rgb(51, 86, 117));
    }
    button
}

pub(super) fn price_tone_color(tone: TradePriceTone) -> egui::Color32 {
    match tone {
        TradePriceTone::Favorable => egui::Color32::from_rgb(112, 214, 147),
        TradePriceTone::Neutral => egui::Color32::from_rgb(198, 202, 208),
        TradePriceTone::Unfavorable => egui::Color32::from_rgb(232, 112, 112),
    }
}

pub(super) fn sorted_cargo_lots(cargo_lots: &[CargoLoad]) -> Vec<CargoLoad> {
    let mut lots = cargo_lots.to_vec();
    lots.sort_by(|left, right| {
        right
            .amount
            .total_cmp(&left.amount)
            .then_with(|| left.commodity.cmp(&right.commodity))
            .then_with(|| left.source.cmp(&right.source))
    });
    lots
}

pub(super) fn cargo_summary_line(cargo_lots: &[CargoLoad]) -> String {
    if cargo_lots.is_empty() {
        return "-".to_string();
    }

    let lots = sorted_cargo_lots(cargo_lots);
    let mut parts = lots
        .iter()
        .take(3)
        .map(|cargo| {
            format!(
                "{} {:.1} ({})",
                commodity_label(cargo.commodity),
                cargo.amount,
                cargo_source_label(cargo.source)
            )
        })
        .collect::<Vec<_>>();
    if lots.len() > 3 {
        parts.push(format!("+{} more", lots.len() - 3));
    }
    parts.join(", ")
}

pub(super) fn buy_disabled_reason(
    docked: bool,
    _cargo_lots: &[CargoLoad],
    row: &gatebound_sim::StationTradeRowView,
) -> Option<&'static str> {
    if !docked {
        return Some("ship must be docked at the station before spot trading is available");
    }
    if row.can_buy {
        return None;
    }
    if row.station_stock + 1e-9 < 0.1 {
        return Some("station stock is below the minimum tradable lot");
    }
    if row.insufficient_capital {
        return Some("insufficient capital for the minimum trade lot");
    }
    Some("the hold has no remaining capacity for this commodity")
}

pub(super) fn sell_disabled_reason(
    docked: bool,
    cargo_lots: &[CargoLoad],
    row: &gatebound_sim::StationTradeRowView,
) -> Option<&'static str> {
    if !docked {
        return Some("ship must be docked at the station before spot trading is available");
    }
    if row.can_sell {
        return None;
    }
    if has_matching_spot_cargo(cargo_lots, row.commodity) {
        return Some("matching spot cargo is below the minimum trade lot");
    }
    Some("no matching spot cargo is loaded for this row")
}

pub(super) fn storage_load_disabled_reason(
    docked: bool,
    _cargo_lots: &[CargoLoad],
    row: &gatebound_sim::StationStorageRowView,
) -> Option<&'static str> {
    if !docked {
        return Some("ship must be docked at the station before storage transfer is available");
    }
    if row.can_load {
        return None;
    }
    if row.stored_amount + 1e-9 < 0.1 {
        return Some("this station storage row is below the minimum transferable lot");
    }
    Some("the hold has no remaining capacity for this commodity")
}

pub(super) fn storage_unload_disabled_reason(
    docked: bool,
    cargo_lots: &[CargoLoad],
    row: &gatebound_sim::StationStorageRowView,
) -> Option<&'static str> {
    if !docked {
        return Some("ship must be docked at the station before storage transfer is available");
    }
    if row.can_unload {
        return None;
    }
    if cargo_lots.is_empty() {
        return Some("ship has no cargo available for storage");
    }
    if has_matching_spot_cargo(cargo_lots, row.commodity) {
        return Some("matching spot cargo is below the minimum transferable lot");
    }
    Some("selected storage row does not match any spot cargo loaded")
}

fn has_matching_spot_cargo(cargo_lots: &[CargoLoad], commodity: Commodity) -> bool {
    cargo_lots.iter().any(|cargo| {
        cargo.source == CargoSource::Spot && cargo.commodity == commodity && cargo.amount > 0.0
    })
}
