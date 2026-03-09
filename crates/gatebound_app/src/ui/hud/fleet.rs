use bevy_egui::egui;

use crate::features::ships::{open_ship_card, ShipUiState};

use super::snapshot::FleetListRowSnapshot;

pub(super) fn render_fleet_window(
    ctx: &egui::Context,
    save_menu_open: bool,
    open: &mut bool,
    fleet_list_rows: &[FleetListRowSnapshot],
    ship_ui: &mut ShipUiState,
) {
    if save_menu_open || !*open {
        return;
    }

    egui::Window::new("Fleet Manager")
        .default_width(560.0)
        .default_height(420.0)
        .open(open)
        .show(ctx, |ui| {
            ui.label(format!("Ships: {}", fleet_list_rows.len()));
            ui.separator();

            if fleet_list_rows.is_empty() {
                ui.label("No player ships available");
                return;
            }

            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for row in fleet_list_rows {
                        egui::Frame::group(ui.style())
                            .fill(egui::Color32::from_rgb(16, 21, 28))
                            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(56, 72, 88)))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.vertical(|ui| {
                                        ui.label(egui::RichText::new(&row.ship_name).strong());
                                        ui.label(&row.location_text);
                                        ui.label(
                                            egui::RichText::new(&row.status_text)
                                                .color(egui::Color32::from_rgb(143, 185, 255)),
                                        );
                                    });
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if ui.button("Open card").clicked() {
                                                open_ship_card(ship_ui, row.ship_id);
                                            }
                                        },
                                    );
                                });
                            });
                        ui.add_space(6.0);
                    }
                });
        });
}
