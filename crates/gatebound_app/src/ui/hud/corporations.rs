use bevy_egui::egui;
use gatebound_sim::CorporationRowView;

use super::labels::company_archetype_label;

pub(super) fn render_corporations_window(
    ctx: &egui::Context,
    save_menu_open: bool,
    open: &mut bool,
    corporation_rows: &[CorporationRowView],
) {
    if save_menu_open || !*open {
        return;
    }

    egui::Window::new("NPC Corporations")
        .open(open)
        .show(ctx, |ui| {
            ui.label(format!("Tracked corporations: {}", corporation_rows.len()));
            ui.separator();
            egui::Grid::new("corporation_panel_grid")
                .num_columns(8)
                .striped(true)
                .show(ui, |ui| {
                    ui.strong("Corp");
                    ui.strong("Type");
                    ui.strong("Balance");
                    ui.strong("Last P&L");
                    ui.strong("Idle");
                    ui.strong("Transit");
                    ui.strong("Orders");
                    ui.strong("Next Tick");
                    ui.end_row();

                    for row in corporation_rows {
                        ui.label(&row.name);
                        ui.monospace(company_archetype_label(row.archetype));
                        ui.monospace(format!("{:.1}", row.balance));
                        ui.monospace(format!("{:.1}", row.last_realized_profit));
                        ui.monospace(row.idle_ships.to_string());
                        ui.monospace(row.in_transit_ships.to_string());
                        ui.monospace(row.active_orders.to_string());
                        ui.monospace(row.next_plan_tick.to_string());
                        ui.end_row();
                    }
                });
        });
}
