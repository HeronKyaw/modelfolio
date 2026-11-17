//! Settings: background interval, toasts, new-window, theme, accent, info.

use crate::app::App;
use crate::store::config::ThemePref;
use crate::store::secrets;
use egui_phosphor::regular as ic;

pub fn window(app: &mut App, ctx: &egui::Context) {
    if !app.settings_open {
        return;
    }
    let mut open = true;
    egui::Window::new(format!("{} Settings", ic::GEAR_SIX))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(460.0)
        .show(ctx, |ui| content(app, ui));
    if !open {
        app.settings_open = false;
    }
}

fn content(app: &mut App, ui: &mut egui::Ui) {
    ui.label(egui::RichText::new("Background fetch").strong());
    egui::Grid::new("settings_bg").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
        ui.label("Interval");
        let mut interval = app.config.settings.background_interval_min.max(15);
        if ui
            .add(egui::Slider::new(&mut interval, 15..=720).suffix(" min").integer())
            .changed()
        {
            app.config.settings.background_interval_min = interval;
            app.schedule_next_background(crate::app::now_ts());
            app.save_config();
        }
        ui.end_row();

        ui.label("Toasts on changes");
        let mut toasts = app.config.settings.background_toasts;
        if ui.checkbox(&mut toasts, "(background fetches only)").changed() {
            app.config.settings.background_toasts = toasts;
            app.save_config();
        }
        ui.end_row();

        ui.label("Include removed");
        let mut include_removed = app.config.settings.toast_include_removed;
        if ui
            .add_enabled(
                app.config.settings.background_toasts,
                egui::Checkbox::new(&mut include_removed, "mention removed models in the toast"),
            )
            .changed()
        {
            app.config.settings.toast_include_removed = include_removed;
            app.save_config();
        }
        ui.end_row();
    });

    ui.add_space(10.0);
    ui.label(egui::RichText::new("Catalog").strong());
    egui::Grid::new("settings_catalog").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
        ui.label("“New” window");
        let mut days = app.config.settings.new_window_days.max(1);
        if ui.add(egui::DragValue::new(&mut days).range(1..=30).suffix(" days")).changed() {
            app.config.settings.new_window_days = days;
            app.save_config();
        }
        ui.end_row();

        ui.label("Confirm deletes");
        let mut confirm = app.config.settings.confirm_delete_gateway;
        if ui.checkbox(&mut confirm, "before deleting a gateway").changed() {
            app.config.settings.confirm_delete_gateway = confirm;
            app.save_config();
        }
        ui.end_row();
    });

    ui.add_space(10.0);
    ui.label(egui::RichText::new("Appearance").strong());
    ui.horizontal(|ui| {
        ui.label("Theme");
        let mut theme = app.config.settings.theme;
        let mut changed = false;
        ui.radio_value(&mut theme, ThemePref::System, "System");
        ui.radio_value(&mut theme, ThemePref::Dark, "Dark");
        ui.radio_value(&mut theme, ThemePref::Light, "Light");
        if theme != app.config.settings.theme {
            app.config.settings.theme = theme;
            changed = true;
        }
        if changed {
            app.save_config();
        }
    });
    ui.horizontal(|ui| {
        ui.label("Accent");
        for (name, hex) in crate::theme::ACCENT_PRESETS {
            let color = crate::theme::accent_color(hex);
            if ui
                .add(
                    egui::Button::new(egui::RichText::new("  ").background_color(color))
                        .fill(color)
                        .min_size(egui::vec2(20.0, 14.0)),
                )
                .on_hover_text(*name)
                .clicked()
            {
                app.config.settings.accent = (*hex).to_string();
                app.save_config();
            }
        }
    });

    ui.add_space(12.0);
    ui.separator();
    ui.add_space(6.0);
    ui.label(egui::RichText::new("Security").strong());
    ui.label(
        egui::RichText::new(format!(
            "Keys live in: {}\nData folder: {}\nKeys are never written to disk, logs, exports or toasts.",
            secrets::backend_name(),
            crate::paths::data_dir().display(),
        ))
        .weak(),
    );
}
