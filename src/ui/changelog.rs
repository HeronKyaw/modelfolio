//! Changelog inbox: added / removed / returned per gateway, badge until acked.

use crate::app::{time_ago, App};
use egui_phosphor::regular as ic;

pub fn window(app: &mut App, ctx: &egui::Context) {
    if !app.inbox_open {
        return;
    }
    let mut open = true;
    egui::Window::new(format!("{} Changelog inbox", ic::BELL))
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_width(520.0)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Changes found by fetches — click a row to jump to it").weak());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Acknowledge all").clicked() {
                        if let Err(e) = app.cache.ack_all() {
                            app.set_status(e);
                        }
                    }
                });
            });
            ui.separator();

            let entries = app.cache.changelog(200).unwrap_or_default();
            if entries.is_empty() {
                ui.add_space(12.0);
                ui.vertical_centered(|ui| {
                    ui.label(egui::RichText::new("No changes yet").weak());
                });
                return;
            }

            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                for e in &entries {
                    let (symbol, color) = match e.change.as_str() {
                        "added" => ("+", egui::Color32::from_rgb(0x2e, 0xb4, 0x40)),
                        "removed" => ("−", egui::Color32::from_rgb(0xe5, 0x48, 0x4d)),
                        "returned" => ("↻", egui::Color32::from_rgb(0x0a, 0x84, 0xff)),
                        _ => ("·", egui::Color32::GRAY),
                    };
                    let gw = app
                        .config
                        .gateway(&e.gateway_id)
                        .map(|g| g.name.clone())
                        .unwrap_or_else(|| "(deleted gateway)".into());
                    let dot = if e.ack { "" } else { "●" };
                    let row = ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(symbol).strong().color(color));
                        ui.label(egui::RichText::new(&e.model_id).monospace());
                        ui.label(egui::RichText::new(format!("on {}", gw)).weak());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(
                                egui::RichText::new(dot)
                                    .color(crate::theme::accent_color(&app.config.settings.accent))
                                    .small(),
                            );
                            ui.label(egui::RichText::new(time_ago(e.at)).small().weak())
                                .on_hover_text(crate::app::fmt_ts(e.at));
                        });
                    });
                    let resp = row.response.interact(egui::Sense::click());
                    if resp.clicked() {
                        app.selected_gateway = Some(e.gateway_id.clone());
                        app.view = match e.change.as_str() {
                            "removed" => crate::app::View::Removed,
                            _ => crate::app::View::New,
                        };
                        app.inbox_open = false;
                    }
                    ui.separator();
                }
            });
        });
    if !open {
        app.inbox_open = false;
    }
}
