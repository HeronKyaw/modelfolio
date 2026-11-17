//! Gateways: sidebar list with health, CRUD modal, keyring-backed secrets.

use crate::app::{time_ago, App};
use crate::store::config::{Gateway, GatewayKind};
use crate::store::secrets;
use egui_phosphor::regular as ic;
use std::collections::HashSet;

// ---- sidebar section --------------------------------------------------------

pub fn sidebar_section(app: &mut App, ui: &mut egui::Ui) {
    let gw_tags = app.cache.gateway_tags().unwrap_or_default();
    let current_tag = if let crate::app::View::Tag(id) = app.view { Some(id) } else { None };
    let tag_assigned_to_any_gateway =
        current_tag.is_some_and(|tid| gw_tags.values().any(|v| v.contains(&tid)));
    let any_enabled = app.config.gateways.iter().any(|g| g.enabled);

    if app.config.gateways.is_empty() {
        ui.weak("No gateways yet — add one to fetch models");
        return;
    }

    for g in app.config.gateways.clone() {
        if tag_assigned_to_any_gateway
            && !current_tag.is_some_and(|tid| {
                gw_tags.get(&g.id).is_some_and(|v| v.contains(&tid))
            })
        {
            continue;
        }

        let selected = app.selected_gateway.as_deref() == Some(g.id.as_str());
        let status = app.cache.gateway_status(&g.id);
        let fetching = app.fetching.contains(&g.id);
        let errored = status.last_error_at > status.last_success_at;

        ui.horizontal(|ui| {
            // status dot / spinner
            if fetching {
                ui.spinner();
            } else {
                let (dot, color) = if !g.enabled {
                    ("●", egui::Color32::GRAY)
                } else if errored {
                    ("●", egui::Color32::from_rgb(0xe5, 0x48, 0x4d))
                } else if status.last_success_at.is_some() {
                    ("●", egui::Color32::from_rgb(0x2e, 0xb4, 0x40))
                } else {
                    ("○", egui::Color32::GRAY)
                };
                ui.label(egui::RichText::new(dot).size(10.0).color(color));
            }

            let label = if g.enabled {
                g.name.clone()
            } else {
                format!("{} (off)", g.name)
            };
            let name_resp = ui.selectable_label(
                selected,
                egui::RichText::new(&label).size(14.0),
            );
            if name_resp.clicked() {
                app.selected_gateway =
                    if selected { None } else { Some(g.id.clone()) };
                app.selected_rows.clear();
            }
            if !g.notes.is_empty() {
                name_resp.on_hover_text(&g.notes);
            } else if errored {
                if let Some(err) = &status.last_error {
                    name_resp.on_hover_text(format!("Last error: {err}"));
                }
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.menu_button("⋯", |ui| gateway_menu(app, ui, &g));
            });
        });

        if errored {
            if let Some(err) = &status.last_error {
                ui.label(
                    egui::RichText::new(format!("  {}", truncate(err, 60)))
                        .small()
                        .color(egui::Color32::from_rgb(0xe5, 0x48, 0x4d)),
                );
            }
        } else if let Some(ts) = status.last_success_at {
            ui.label(
                egui::RichText::new(format!("  ok {}", time_ago(ts)))
                    .small()
                    .weak(),
            );
        }
    }
    ui.add_space(2.0);
    if !any_enabled {
        ui.label(egui::RichText::new("All gateways disabled").small().weak());
    }
}

fn gateway_menu(app: &mut App, ui: &mut egui::Ui, g: &Gateway) {
    let tags = app.cache.tags().unwrap_or_default();
    let gw_tags = app.cache.gateway_tags().unwrap_or_default();
    let current: HashSet<i64> =
        gw_tags.get(&g.id).cloned().unwrap_or_default().into_iter().collect();

    if ui.button("Refresh now").clicked() {
        app.trigger_fetch(crate::app::Trigger::Manual, Some(g.id.clone()));
    }
    if ui.button("Edit…").clicked() {
        let has_key = g
            .secret_id
            .as_deref()
            .map(|sid| secrets::has_key(sid).unwrap_or(false))
            .unwrap_or(false);
        app.gateway_modal = Some(GatewayModal::edit(g, has_key, current.clone()));
    }
    ui.menu_button("Tags", |ui| {
        for t in &tags {
            let mut on = current.contains(&t.id);
            if ui.checkbox(&mut on, &t.name).changed() {
                if let Err(e) = app.cache.set_gateway_tag(&g.id, t.id, on) {
                    app.set_status(e);
                }
            }
        }
    });
    if ui.button(if g.enabled { "Disable" } else { "Enable" }).clicked() {
        let mut cfg = app.config.gateways.clone();
        if let Some(gw) = cfg.iter_mut().find(|x| x.id == g.id) {
            gw.enabled = !gw.enabled;
        }
        app.config.gateways = cfg;
        app.save_config();
    }
    ui.separator();
    if ui.button("Delete…").clicked() {
        if app.config.settings.confirm_delete_gateway {
            app.confirm_delete = Some(g.id.clone());
        } else {
            crate::ui::delete_gateway(app, &g.id);
        }
    }
}

// ---- add / edit modal ---------------------------------------------------------

#[derive(Debug, Clone)]
pub struct GatewayModal {
    pub editing: Option<String>,
    pub name: String,
    pub base_url: String,
    pub kind: GatewayKind,
    pub enabled: bool,
    pub notes: String,
    pub key_input: String,
    pub has_key: bool,
    pub remove_key: bool,
    pub secret_id: Option<String>,
    pub selected_tags: HashSet<i64>,
    pub error: Option<String>,
}

impl GatewayModal {
    pub fn add() -> Self {
        Self {
            editing: None,
            name: String::new(),
            base_url: String::new(),
            kind: GatewayKind::OpenaiCompat,
            enabled: true,
            notes: String::new(),
            key_input: String::new(),
            has_key: false,
            remove_key: false,
            secret_id: None,
            selected_tags: HashSet::new(),
            error: None,
        }
    }

    pub fn edit(gw: &Gateway, has_key: bool, selected_tags: HashSet<i64>) -> Self {
        Self {
            editing: Some(gw.id.clone()),
            name: gw.name.clone(),
            base_url: gw.base_url.clone(),
            kind: gw.kind,
            enabled: gw.enabled,
            notes: gw.notes.clone(),
            key_input: String::new(),
            has_key,
            remove_key: false,
            secret_id: gw.secret_id.clone(),
            selected_tags,
            error: None,
        }
    }
}

pub fn show_gateway_modal(app: &mut App, ctx: &egui::Context) {
    let Some(mut modal) = app.gateway_modal.take() else { return };
    let mut open = true;
    let title = if modal.editing.is_some() { "Edit gateway" } else { "Add gateway" };
    egui::Window::new(title)
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(460.0)
        .show(ctx, |ui| modal_body(app, ui, &mut modal));
    if open {
        app.gateway_modal = Some(modal);
    }
}

fn modal_body(app: &mut App, ui: &mut egui::Ui, m: &mut GatewayModal) {
    egui::Grid::new("gateway_form")
        .num_columns(2)
        .spacing([10.0, 8.0])
        .show(ui, |ui| {
            ui.label("Name");
            ui.text_edit_singleline(&mut m.name);
            ui.end_row();

            ui.label("Base URL");
            ui.add(
                egui::TextEdit::singleline(&mut m.base_url)
                    .hint_text("https://openrouter.ai/api  ·  http://localhost:11434")
                    .desired_width(320.0),
            );
            ui.end_row();

            ui.label("Kind");
            egui::ComboBox::from_label("")
                .selected_text(m.kind.label())
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut m.kind, GatewayKind::OpenaiCompat, "OpenAI-compatible");
                    ui.selectable_value(&mut m.kind, GatewayKind::Ollama, "Ollama");
                });
            ui.end_row();

            ui.label("API key");
            ui.horizontal(|ui| {
                let show = ui
                    .add(
                        egui::Button::new(egui::RichText::new(ic::EYE).size(13.0)).frame(false),
                    )
                    .is_pointer_button_down_on();
                let resp = ui.add(
                    egui::TextEdit::singleline(&mut m.key_input)
                        .password(!show)
                        .hint_text(if m.has_key { "••••• stored — type to replace" } else { "sk-… (stored in the OS key store)" })
                        .desired_width(280.0),
                );
                let _ = resp;
                if m.has_key {
                    if ui
                        .add(egui::Button::new(egui::RichText::new("Remove").small()))
                        .clicked()
                    {
                        m.remove_key = true;
                        m.has_key = false;
                    }
                    ui.label(
                        egui::RichText::new(secrets::backend_name())
                            .small()
                            .weak(),
                    );
                }
            });
            ui.end_row();

            ui.label("Enabled");
            ui.checkbox(&mut m.enabled, "fetch in manual & background refresh");
            ui.end_row();

            ui.label("Notes");
            ui.add(egui::TextEdit::multiline(&mut m.notes).desired_rows(2).desired_width(320.0));
            ui.end_row();

            ui.label("Tags");
            ui.horizontal_wrapped(|ui| {
                for t in app.cache.tags().unwrap_or_default() {
                    if t.builtin {
                        continue;
                    }
                    let mut on = m.selected_tags.contains(&t.id);
                    if ui.checkbox(&mut on, &t.name).changed() {
                        if on {
                            m.selected_tags.insert(t.id);
                        } else {
                            m.selected_tags.remove(&t.id);
                        }
                    }
                }
                if ui.button("+ tag").clicked() {
                    app.new_tag_open = true;
                }
            });
            ui.end_row();
        });

    if let Some(err) = &m.error {
        ui.add_space(4.0);
        ui.label(egui::RichText::new(err).color(egui::Color32::from_rgb(0xe5, 0x48, 0x4d)));
    }

    ui.add_space(10.0);
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        let accent = crate::theme::accent_color(&app.config.settings.accent);
        if ui
            .add(egui::Button::new(egui::RichText::new("Save").strong()).fill(accent))
            .clicked()
        {
            save(app, m);
        }
        if ui.button("Cancel").clicked() {
            app.gateway_modal = None;
        }
    });
}

fn save(app: &mut App, m: &mut GatewayModal) {
    let name = m.name.trim();
    if name.is_empty() {
        m.error = Some("Name is required".into());
        return;
    }
    let base = match crate::net::validate_base(&m.base_url) {
        Ok(b) => b,
        Err(e) => {
            m.error = Some(e);
            return;
        }
    };

    // Key handling — always through the OS key store, never config.json.
    let mut secret_id = m.secret_id.clone();
    if !m.key_input.trim().is_empty() {
        let id = m
            .editing
            .clone()
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let sid = format!("modelfolio/{id}");
        if let Err(e) = secrets::store(&sid, m.key_input.trim()) {
            m.error = Some(e);
            return;
        }
        secret_id = Some(sid);
        m.has_key = true;
        m.key_input.clear();
    } else if m.remove_key {
        if let Some(sid) = &m.secret_id {
            if let Err(e) = secrets::delete(sid) {
                m.error = Some(e);
                return;
            }
        }
        secret_id = None;
        m.remove_key = false;
    }

    match &m.editing {
        None => {
            let id = uuid::Uuid::new_v4().to_string();
            let secret_id = secret_id.or_else(|| Some(format!("modelfolio/{id}")));
            let gw = Gateway {
                id: id.clone(),
                name: name.to_string(),
                base_url: base,
                kind: m.kind,
                secret_id,
                enabled: m.enabled,
                notes: m.notes.trim().to_string(),
            };
            for t in &m.selected_tags {
                if let Err(e) = app.cache.set_gateway_tag(&id, *t, true) {
                    app.set_status(e);
                }
            }
            app.config.gateways.push(gw);
            app.set_status(format!("Gateway “{name}” added — hit Refresh to fetch models"));
        }
        Some(id) => {
            if let Some(gw) = app.config.gateways.iter_mut().find(|g| g.id == *id) {
                gw.name = name.to_string();
                gw.base_url = base;
                gw.kind = m.kind;
                gw.enabled = m.enabled;
                gw.notes = m.notes.trim().to_string();
                gw.secret_id = secret_id;
            }
            let tags = app.cache.tags().unwrap_or_default();
            for t in &tags {
                if t.builtin {
                    continue;
                }
                if let Err(e) = app.cache.set_gateway_tag(id, t.id, m.selected_tags.contains(&t.id)) {
                    app.set_status(e);
                }
            }
            app.set_status(format!("Gateway “{name}” saved"));
        }
    }
    app.gateway_modal = None;
    app.save_config();
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        s.chars().take(n).collect::<String>() + "…"
    }
}
