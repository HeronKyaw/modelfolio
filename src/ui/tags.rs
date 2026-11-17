//! Tags sidebar section: tag filter rows with counts, rename/delete menus.

use crate::app::{App, View};
use egui_phosphor::regular as ic;

pub fn sidebar_section(app: &mut App, ui: &mut egui::Ui) {
    let tags = match app.cache.tags() {
        Ok(t) => t,
        Err(e) => {
            ui.label(egui::RichText::new(e).small().color(egui::Color32::RED));
            return;
        }
    };
    let mut any_user_tag = false;
    for t in &tags {
        if t.builtin {
            continue; // favorite/ignore are represented by the Favorites/Hidden views
        }
        any_user_tag = true;
        let count = app.cache.tag_model_count(t.id).ok();
        let selected = app.view == View::Tag(t.id);
        let text = match count {
            Some(c) if c > 0 => format!("{}  {}  {c}", ic::TAG, capitalize(&t.name)),
            _ => format!("{}  {}", ic::TAG, capitalize(&t.name)),
        };
        ui.horizontal(|ui| {
            let resp = ui.selectable_label(selected, egui::RichText::new(&text).size(14.0));
            if resp.clicked() {
                app.view = View::Tag(t.id);
                app.selected_rows.clear();
            }
            // small menu for rename/delete
            ui.menu_button("⋯", |ui| {
                if ui.button("Rename").clicked() {
                    app.rename_tag = Some((t.id, t.name.clone()));
                }
                if ui.button("Delete").clicked() {
                    match app.cache.delete_tag(t.id) {
                        Ok(()) => {
                            if app.view == View::Tag(t.id) {
                                app.view = View::All;
                            }
                            app.set_status("Tag deleted (assignments removed)");
                        }
                        Err(e) => app.set_status(e),
                    }
                }
            });
        });
    }
    if !any_user_tag {
        ui.weak("No tags yet — right-click a model to add one");
    }
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}
