pub mod changelog;
pub mod gateways;
pub mod models;
pub mod settings;
pub mod tags;

use crate::app::{now_ts, App, Trigger, View};
use crate::store::secrets;
use egui::Align2;
use egui_phosphor::regular as ic;

fn accent(app: &App) -> egui::Color32 {
    crate::theme::accent_color(&app.config.settings.accent)
}

// ---- top bar ---------------------------------------------------------------

pub fn topbar(app: &mut App, ui: &mut egui::Ui) {
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.add_space(4.0);
        ui.label(egui::RichText::new(format!("{} Modelfolio", ic::SHIELD_CHECK)).heading());
        ui.separator();

        let search = ui.add(
            egui::TextEdit::singleline(&mut app.search_text)
                .hint_text("Search models — tag:x, gateway:x, is:new, is:favorite, is:removed")
                .desired_width(360.0)
                .id(egui::Id::new("search")),
        );
        if app.search_focus {
            search.request_focus();
            app.search_focus = false;
        }
        if search.changed() {
            app.selected_rows.clear();
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // Settings
            if ui
                .add(egui::Button::new(egui::RichText::new(ic::GEAR_SIX).size(15.0)).frame(false))
                .clicked()
            {
                app.settings_open = !app.settings_open;
            }
            ui.separator();

            // Refresh
            let busy = !app.fetching.is_empty();
            let label = if busy {
                egui::RichText::new(format!("{} Refreshing…", ic::ARROWS_CLOCKWISE).to_string())
            } else {
                egui::RichText::new(format!("{} Refresh", ic::ARROWS_CLOCKWISE).to_string()).strong()
            };
            let mut btn = egui::Button::new(label);
            if !busy {
                btn = btn.fill(accent(app));
            }
            if ui.add_enabled(!busy, btn).clicked() {
                app.trigger_fetch(Trigger::Manual, app.selected_gateway.clone());
            }
            if ui
                .add(egui::Button::new(egui::RichText::new(format!("{} New gateway", ic::PLUS)).size(14.0)))
                .clicked()
            {
                app.gateway_modal = Some(gateways::GatewayModal::add());
            }

            // Changelog inbox with badge
            let unacked = app.cache.unacked_count().unwrap_or(0);
            let bell = if unacked > 0 {
                egui::RichText::new(format!("{} {}", ic::BELL, unacked))
                    .strong()
                    .color(accent(app))
            } else {
                egui::RichText::new(ic::BELL.to_string())
            };
            if ui.add(egui::Button::new(bell)).clicked() {
                app.inbox_open = !app.inbox_open;
            }
        });
    });
    ui.add_space(2.0);
}

// ---- sidebar ---------------------------------------------------------------

pub fn sidebar(app: &mut App, ui: &mut egui::Ui) {
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.add_space(6.0);
            section_label(ui, "VIEWS");
            let visible = app.cache.visible_count().unwrap_or(0);
            view_row(app, ui, View::All, ic::SQUARES_FOUR, "All models", Some(visible));
            view_row(app, ui, View::New, ic::SPARKLE, "New", count_new(app));
            view_row(app, ui, View::Favorites, ic::STAR, "Favorites", count_favorites(app));
            view_row(app, ui, View::Removed, ic::MINUS_CIRCLE, "Removed", count_removed(app));
            view_row(app, ui, View::Hidden, ic::EYE_SLASH, "Hidden", count_hidden(app));
            ui.add_space(10.0);

            ui.horizontal(|ui| {
                section_label(ui, "TAGS");
                if ui
                    .add(egui::Button::new(egui::RichText::new(ic::PLUS).size(12.0)).frame(false))
                    .on_hover_text("New tag (Ctrl+N is for gateways)")
                    .clicked()
                {
                    app.new_tag_open = true;
                    app.new_tag_name.clear();
                }
            });
            tags::sidebar_section(app, ui);
            ui.add_space(10.0);

            ui.horizontal(|ui| {
                section_label(ui, "GATEWAYS");
                if ui
                    .add(egui::Button::new(egui::RichText::new(ic::PLUS).size(12.0)).frame(false))
                    .on_hover_text("Add gateway (Ctrl+N)")
                    .clicked()
                {
                    app.gateway_modal = Some(gateways::GatewayModal::add());
                }
            });
            gateways::sidebar_section(app, ui);
            ui.add_space(6.0);
        });
}

fn section_label(ui: &mut egui::Ui, text: &str) {
    ui.label(egui::RichText::new(text).small().weak());
}

fn view_row(
    app: &mut App,
    ui: &mut egui::Ui,
    view: View,
    icon: &str,
    label: &str,
    count: Option<i64>,
) {
    let selected = app.view == view;
    let text = match count {
        Some(c) if c > 0 => format!("{icon}  {label}  {c}"),
        _ => format!("{icon}  {label}"),
    };
    if ui
        .selectable_label(selected, egui::RichText::new(text).size(14.0))
        .clicked()
    {
        app.view = view;
        app.selected_rows.clear();
    }
}

fn count_new(app: &App) -> Option<i64> {
    let cutoff = now_ts() - app.config.settings.new_window_days as i64 * 86400;
    app.cache.count_new(cutoff).ok()
}

fn count_favorites(app: &App) -> Option<i64> {
    app.cache
        .tags()
        .ok()?
        .iter()
        .find(|t| t.name == "favorite")
        .and_then(|t| app.cache.tag_model_count(t.id).ok())
}

fn count_removed(app: &App) -> Option<i64> {
    app.cache.count_removed().ok()
}

fn count_hidden(app: &App) -> Option<i64> {
    app.cache.count_hidden().ok()
}

// ---- status bar -------------------------------------------------------------

pub fn statusbar(app: &mut App, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.add_space(6.0);
        let status = if app.status.is_empty() {
            "Ready — the cache opens instantly, even offline".to_string()
        } else {
            app.status.clone()
        };
        ui.label(egui::RichText::new(status).small().weak());

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(6.0);
            ui.label(
                egui::RichText::new(secrets::backend_name())
                    .small()
                    .weak(),
            );
            ui.separator();
            if app.config.gateways.iter().any(|g| g.enabled) {
                let secs = (app.next_bg_fetch - now_ts()).max(0);
                ui.label(
                    egui::RichText::new(format!(
                        "Next background fetch in {}",
                        human_secs(secs)
                    ))
                    .small()
                    .weak(),
                );
            }
        });
    });
}

fn human_secs(mut s: i64) -> String {
    let h = s / 3600;
    s %= 3600;
    let m = s / 60;
    s %= 60;
    if h > 0 {
        format!("{h}h {m}m")
    } else if m > 0 {
        format!("{m}m {s}s")
    } else {
        format!("{s}s")
    }
}

// ---- overlays ---------------------------------------------------------------

pub fn draw_toasts(app: &mut App, ctx: &egui::Context) {
    let now = now_ts();
    app.toasts.retain(|t| now - t.born < 6);
    for (i, t) in app.toasts.iter().enumerate().rev() {
        egui::Area::new(egui::Id::new("in_app_toast").with(i))
            .anchor(Align2::RIGHT_BOTTOM, [-16.0, -(16.0 + i as f32 * 76.0)])
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(ui.style().visuals.window_fill)
                    .corner_radius(8)
                    .stroke(ui.style().visuals.widgets.noninteractive.bg_stroke)
                    .inner_margin(egui::Margin::same(10))
                    .show(ui, |ui| {
                        ui.set_max_width(300.0);
                        ui.label(egui::RichText::new(&t.title).strong());
                        ui.label(egui::RichText::new(&t.body).size(13.0));
                    });
            });
    }
}

pub fn tag_modals(app: &mut App, ctx: &egui::Context) {
    // New tag
    if app.new_tag_open {
        let mut open = true;
        egui::Window::new("New tag")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Name:");
                    let r = ui.text_edit_singleline(&mut app.new_tag_name);
                    let save = r.lost_focus()
                        && ui.input(|i| i.key_pressed(egui::Key::Enter))
                        || ui.button("Create").clicked();
                    if save {
                        match app.cache.create_tag(&app.new_tag_name) {
                            Ok(_) => {
                                app.new_tag_open = false;
                                app.new_tag_name.clear();
                            }
                            Err(e) => app.set_status(e),
                        }
                    }
                });
            });
        if !open {
            app.new_tag_open = false;
        }
    }

    // Rename tag
    if let Some((id, name)) = app.rename_tag.clone() {
        let mut keep = true;
        let mut name_edit = name.clone();
        egui::Window::new("Rename tag")
            .open(&mut keep)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Name:");
                    ui.text_edit_singleline(&mut name_edit);
                    if ui.button("Save").clicked() {
                        match app.cache.rename_tag(id, &name_edit) {
                            Ok(()) => app.rename_tag = None,
                            Err(e) => app.set_status(e),
                        }
                    }
                    if ui.button("Cancel").clicked() {
                        app.rename_tag = None;
                    }
                });
            });
        if !keep {
            app.rename_tag = None;
        } else if app.rename_tag == Some((id, name)) {
            app.rename_tag = Some((id, name_edit));
        }
    }
}

pub fn confirm_delete(app: &mut App, ctx: &egui::Context) {
    let Some(id) = app.confirm_delete.clone() else { return };
    let mut open = true;
    let name = app
        .config
        .gateway(&id)
        .map(|g| g.name.clone())
        .unwrap_or_else(|| "gateway".into());
    egui::Window::new("Delete gateway?")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.label(format!(
                "Delete “{name}” and its cached catalog? The stored API key will also be removed."
            ));
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let red = egui::Color32::from_rgb(0xe5, 0x48, 0x4d);
                if ui.add(egui::Button::new(egui::RichText::new("Delete").strong()).fill(red)).clicked() {
                    delete_gateway(app, &id);
                    app.confirm_delete = None;
                }
                if ui.button("Cancel").clicked() {
                    app.confirm_delete = None;
                }
            });
        });
    if !open {
        app.confirm_delete = None;
    }
}

pub fn delete_gateway(app: &mut App, id: &str) {
    if let Some(gw) = app.config.gateway(id) {
        if let Some(sid) = &gw.secret_id {
            let sid = sid.clone();
            if let Err(e) = secrets::delete(&sid) {
                app.set_status(format!("Note: {e}"));
            }
        }
    }
    if let Err(e) = app.cache.delete_gateway_data(id) {
        app.set_status(format!("Cache cleanup failed: {e}"));
    }
    app.config.gateways.retain(|g| g.id != id);
    if app.selected_gateway.as_deref() == Some(id) {
        app.selected_gateway = None;
    }
    app.save_config();
    app.set_status("Gateway deleted");
}
