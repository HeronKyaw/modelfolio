//! Central panel: searchable, virtualized, sortable model table with tags,
//! compare, changelog pills, favorites, hide, copy id and export.

use crate::app::{now_ts, time_ago, App, Compare, SortCol, View};
use crate::search;
use crate::store::cache::tag_color;
use egui_extras::{Column, TableBuilder};
use egui_phosphor::regular as ic;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct RowView {
    pub pk: i64,
    pub gateway_name: String,
    pub model_id: String,
    pub owner: Option<String>,
    pub first_seen: i64,
    pub last_seen: i64,
    pub removed_at: Option<i64>,
    pub favorite: bool,
    pub hidden: bool,
    pub tags: Vec<String>,
    pub gw_count: usize,
}

pub fn central(app: &mut App, ui: &mut egui::Ui) {
    // Manual-refresh notice banner (jumps to new rows, no toast).
    if let Some(notice) = app.notice.clone() {
        ui.horizontal(|ui| {
            let accent = crate::theme::accent_color(&app.config.settings.accent);
            ui.label(
                egui::RichText::new(format!(" {} ", ic::SPARKLE))
                    .color(egui::Color32::WHITE)
                    .background_color(accent),
            );
            ui.label(egui::RichText::new(format!("{notice} — manual refresh, no toast")).weak());
            if ui.button("View new models").clicked() {
                app.view = View::New;
                app.notice = None;
                app.selected_rows.clear();
            }
            if ui.button("Dismiss").clicked() {
                app.notice = None;
            }
        });
        ui.add_space(4.0);
    }

    if app.config.gateways.is_empty() {
        empty_state(app, ui);
        return;
    }

    // Toolbar
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(ic::SLIDERS_HORIZONTAL).weak());
        egui::ComboBox::from_label("")
            .selected_text(compare_text(app))
            .show_ui(ui, |ui| {
                ui.set_min_width(180.0);
                ui.selectable_value(&mut app.compare, Compare::Any, Compare::Any.label());
                ui.selectable_value(&mut app.compare, Compare::Multi, Compare::Multi.label());
                let can_only = app.selected_gateway.is_some();
                if ui
                    .add_enabled(
                        can_only,
                        egui::Button::selectable(
                            app.compare == Compare::OnlySelected,
                            Compare::OnlySelected.label(),
                        ),
                    )
                    .clicked()
                {
                    app.compare = Compare::OnlySelected;
                }
            });

        ui.separator();
        egui::ComboBox::from_label("")
            .selected_text(match app.export_kind {
                crate::app::ExportKind::Json => "JSON",
                crate::app::ExportKind::Csv => "CSV",
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut app.export_kind, crate::app::ExportKind::Json, "JSON");
                ui.selectable_value(&mut app.export_kind, crate::app::ExportKind::Csv, "CSV");
            });
        if ui.button("Export").on_hover_text("Ids and metadata only — never keys").clicked() {
            let rows = collect_rows(app);
            let sel: HashSet<i64> = app.selected_rows.clone();
            let chosen: Vec<RowView> = if sel.is_empty() {
                rows
            } else {
                rows.into_iter().filter(|r| sel.contains(&r.pk)).collect()
            };
            app.export_rows(&chosen);
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if let Some(gid) = &app.selected_gateway {
                let name = app.config.gateway(gid).map(|g| g.name.clone()).unwrap_or_default();
                if ui.add(egui::Button::new(egui::RichText::new(format!("{} {}", ic::X, name)).small())).clicked() {
                    app.selected_gateway = None;
                }
            }
            let total = app.cache.visible_count().unwrap_or(0);
            ui.label(egui::RichText::new(format!("of {total} visible")).weak());
        });
    });
    ui.add_space(2.0);

    // Rows: build once per frame, then render virtualized.
    let (rows, compare_names) = build_rows(app);
    let shown = rows.len();

    let mut table = TableBuilder::new(ui)
        .striped(true)
        .resizable(true)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Column::auto().resizable(false)) // select
        .column(Column::auto().resizable(false)) // star
        .column(Column::remainder().clip(true).resizable(true)) // model id
        .column(Column::initial(150.0).range(60.0..=320.0).resizable(true)) // tags
        .column(Column::auto().resizable(false)) // pill
        .column(Column::auto().resizable(false)) // menu
        .vscroll(true);
    if app.selected_gateway.is_none() {
        table = table.column(Column::initial(130.0).range(70.0..=260.0).resizable(true)); // gateway
    }
    table = table
        .column(Column::initial(110.0).range(60.0..=220.0).resizable(true)) // owner
        .column(Column::initial(100.0).range(70.0..=160.0).resizable(true)) // first seen
        .column(Column::initial(100.0).range(70.0..=160.0).resizable(true)); // last seen
    if app.compare != Compare::Any {
        table = table.column(Column::auto().resizable(false)); // gw count
    }

    table
        .header(28.0, |mut header| {
            header.col(|ui| {
                ui.label(egui::RichText::new(ic::CHECK_SQUARE).weak());
            });
            header.col(|ui| {
                ui.label(egui::RichText::new(ic::STAR).weak());
            });
            header.col(|ui| {
                sort_header(ui, app, SortCol::Model, "Model");
            });
            header.col(|ui| {
                ui.label(egui::RichText::new("Tags").weak());
            });
            header.col(|ui| {
                ui.label(egui::RichText::new("Status").weak());
            });
            header.col(|ui| {
                ui.label("");
            });
            if app.selected_gateway.is_none() {
                header.col(|ui| {
                    sort_header(ui, app, SortCol::Gateway, "Gateway");
                });
            }
            header.col(|ui| {
                sort_header(ui, app, SortCol::Owner, "Owner");
            });
            header.col(|ui| {
                sort_header(ui, app, SortCol::FirstSeen, "First seen");
            });
            header.col(|ui| {
                sort_header(ui, app, SortCol::LastSeen, "Last seen");
            });
            if app.compare != Compare::Any {
                header.col(|ui| {
                    ui.label(egui::RichText::new("On").weak());
                });
            }
        })
        .body(|mut body| {
            for r in &rows {
                body.row(26.0, |mut row| {
                    // select
                    row.col(|ui| {
                        let mut on = app.selected_rows.contains(&r.pk);
                        if ui.checkbox(&mut on, "").changed() {
                            if on {
                                app.selected_rows.insert(r.pk);
                            } else {
                                app.selected_rows.remove(&r.pk);
                            }
                        }
                    });
                    // star
                    row.col(|ui| {
                        let star = if r.favorite {
                            egui::RichText::new(ic::STAR).color(crate::theme::accent_color(&app.config.settings.accent))
                        } else {
                            egui::RichText::new(ic::STAR).weak()
                        };
                        if ui
                            .add(egui::Button::new(star).frame(false))
                            .on_hover_text(if r.favorite { "Unfavorite" } else { "Favorite" })
                            .clicked()
                        {
                            if let Err(e) = app.cache.set_favorite(r.pk, !r.favorite) {
                                app.set_status(e);
                            }
                        }
                    });
                    // model id
                    row.col(|ui| {
                        ui.add(
                            egui::Label::new(egui::RichText::new(&r.model_id).monospace())
                                .selectable(true)
                                .truncate(),
                        )
                        .on_hover_text(&r.model_id);
                    });
                    // tags
                    row.col(|ui| {
                        let max = 3;
                        for (i, t) in r.tags.iter().enumerate() {
                            if t == "favorite" || t == "ignore" {
                                continue;
                            }
                            if i >= max {
                                ui.label(egui::RichText::new(format!("+{}", r.tags.len() - 3)).small().weak());
                                break;
                            }
                            ui.label(
                                egui::RichText::new(t)
                                    .small()
                                    .background_color(tag_color(t))
                                    .color(egui::Color32::WHITE),
                            );
                        }
                    });
                    // status pill
                    row.col(|ui| {
                        let cutoff = now_ts() - app.config.settings.new_window_days as i64 * 86400;
                        if r.removed_at.is_some() {
                            ui.label(
                                egui::RichText::new(" REMOVED ")
                                    .small()
                                    .strong()
                                    .background_color(egui::Color32::from_rgb(0xe5, 0x48, 0x4d))
                                    .color(egui::Color32::WHITE),
                            )
                            .on_hover_text(format!("Removed {}", time_ago(r.removed_at.unwrap_or(0))));
                        } else if r.first_seen >= cutoff {
                            ui.label(
                                egui::RichText::new(" NEW ")
                                    .small()
                                    .strong()
                                    .background_color(egui::Color32::from_rgb(0x2e, 0xb4, 0x40))
                                    .color(egui::Color32::WHITE),
                            )
                            .on_hover_text(format!("First seen {}", time_ago(r.first_seen)));
                        }
                    });
                    // menu
                    row.col(|ui| {
                        ui.menu_button("⋯", |ui| row_menu(app, ui, r, &compare_names));
                    });
                    // gateway
                    if app.selected_gateway.is_none() {
                        row.col(|ui| {
                            ui.label(egui::RichText::new(&r.gateway_name).weak());
                        });
                    }
                    // owner
                    row.col(|ui| {
                        match &r.owner {
                            Some(o) => ui.label(egui::RichText::new(o).weak()),
                            None => ui.label(""),
                        };
                    });
                    // first / last seen
                    row.col(|ui| {
                        ui.label(egui::RichText::new(time_ago(r.first_seen)).weak());
                    });
                    row.col(|ui| {
                        ui.label(egui::RichText::new(time_ago(r.last_seen)).weak());
                    });
                    // compare count
                    if app.compare != Compare::Any {
                        row.col(|ui| {
                            ui.label(format!("{}", r.gw_count));
                        });
                    }
                });
            }
        });

    let _ = shown;
}

fn compare_text(app: &App) -> String {
    match app.compare {
        Compare::Any => "Any".into(),
        Compare::Multi => "On multiple gateways".into(),
        Compare::OnlySelected => "Only on selected gateway".into(),
    }
}

fn sort_header(ui: &mut egui::Ui, app: &mut App, col: SortCol, label: &str) {
    let arrow = if app.sort_col == col {
        if app.sort_desc { " ▾" } else { " ▴" }
    } else {
        ""
    };
    let label = format!("{}{arrow}", label);
    if ui.add(egui::Button::new(egui::RichText::new(label).weak()).frame(false)).clicked() {
        if app.sort_col == col {
            app.sort_desc = !app.sort_desc;
        } else {
            app.sort_col = col;
            app.sort_desc = false;
        }
    }
}

fn row_menu(app: &mut App, ui: &mut egui::Ui, r: &RowView, compare_names: &HashMap<String, Vec<String>>) {
    if ui.button("Copy model id").clicked() {
        app.copy_text(&r.model_id);
    }
    if ui.button(if r.favorite { "Unfavorite" } else { "Favorite" }).clicked() {
        if let Err(e) = app.cache.set_favorite(r.pk, !r.favorite) {
            app.set_status(e);
        }
    }
    if ui.button(if r.hidden { "Unhide" } else { "Hide (ignore)" }).clicked() {
        if let Err(e) = app.cache.set_hidden(r.pk, !r.hidden) {
            app.set_status(e);
        }
    }
    ui.separator();
    ui.menu_button("Tags", |ui| {
        for t in app.cache.tags().unwrap_or_default() {
            if t.builtin {
                continue;
            }
            let mut on = r.tags.contains(&t.name);
            if ui.checkbox(&mut on, &t.name).changed() {
                if let Err(e) = app.cache.set_model_tag(r.pk, t.id, on) {
                    app.set_status(e);
                }
            }
        }
    });
    let names = compare_names.get(&r.model_id);
    if let Some(names) = names {
        ui.menu_button("On gateways", |ui| {
            for n in names {
                ui.label(egui::RichText::new(n).weak());
            }
        });
    }
}

/// Filter + sort. Returns (visible rows, model_id -> gateway names) for compare.
fn build_rows(app: &mut App) -> (Vec<RowView>, HashMap<String, Vec<String>>) {
    let models = app.cache.all_models().unwrap_or_default();
    let links = app.cache.model_tag_links().unwrap_or_default();
    let mut tags_by_pk: HashMap<i64, Vec<String>> = HashMap::new();
    let mut tag_ids_by_pk: HashMap<i64, Vec<i64>> = HashMap::new();
    for (pk, tag_id, tag_name) in links {
        tags_by_pk.entry(pk).or_default().push(tag_name);
        tag_ids_by_pk.entry(pk).or_default().push(tag_id);
    }
    for v in tags_by_pk.values_mut() {
        v.sort();
    }
    let gw_names: HashMap<String, String> = app
        .config
        .gateways
        .iter()
        .map(|g| (g.id.clone(), g.name.clone()))
        .collect();

    // compare maps (non-removed only)
    let mut counts: HashMap<String, usize> = HashMap::new();
    let mut compare_names: HashMap<String, Vec<String>> = HashMap::new();
    for m in &models {
        if m.removed_at.is_some() {
            continue;
        }
        *counts.entry(m.model_id.clone()).or_default() += 1;
        let name = gw_names.get(&m.gateway_id).cloned().unwrap_or_default();
        compare_names.entry(m.model_id.clone()).or_default().push(name);
    }

    let q = search::parse(&app.search_text);
    let cutoff = now_ts() - app.config.settings.new_window_days as i64 * 86400;
    let selected_gw = app.selected_gateway.clone();

    let mut rows: Vec<RowView> = Vec::new();
    for m in &models {
        let tags = tags_by_pk.get(&m.pk).cloned().unwrap_or_default();
        let tag_ids = tag_ids_by_pk.get(&m.pk).cloned().unwrap_or_default();
        let favorite = tags.iter().any(|t| t == "favorite");
        let hidden = tags.iter().any(|t| t == "ignore");
        let removed = m.removed_at.is_some();
        let gw_name = gw_names.get(&m.gateway_id).cloned().unwrap_or_default();
        let gw_count = counts.get(&m.model_id).copied().unwrap_or(1);

        // gateway scope
        if let Some(gid) = &selected_gw {
            if &m.gateway_id != gid {
                continue;
            }
        }

        // view predicate
        let base = match app.view {
            View::All => !hidden,
            View::Favorites => !hidden && favorite,
            View::New => !hidden && !removed && m.first_seen >= cutoff,
            View::Removed => !hidden && removed,
            View::Hidden => hidden,
            View::Tag(id) => !hidden && tag_ids.contains(&id),
        };
        if !base {
            continue;
        }
        // hidden/removed escape hatches via search operators
        if hidden && app.view != View::Hidden && !q.is_hidden {
            continue;
        }
        if removed && app.view != View::Removed && !q.is_removed {
            continue;
        }

        // compare
        match app.compare {
            Compare::Any => {}
            Compare::Multi => {
                if gw_count < 2 {
                    continue;
                }
            }
            Compare::OnlySelected => {
                let on_selected = selected_gw.as_deref() == Some(m.gateway_id.as_str());
                if !(gw_count == 1 && on_selected) {
                    continue;
                }
            }
        }

        // search
        if !q.is_empty() {
            let hay = format!("{} {} {}", m.model_id, m.owner.as_deref().unwrap_or(""), gw_name)
                .to_lowercase();
            if !q.words.iter().all(|w| hay.contains(w)) {
                continue;
            }
            if !q
                .tags
                .iter()
                .all(|t| tags.iter().any(|rt| rt.to_lowercase() == *t))
            {
                continue;
            }
            if !q.gateways.iter().all(|g| gw_name.to_lowercase().contains(g)) {
                continue;
            }
            if q.is_new && !(m.first_seen >= cutoff && !removed) {
                continue;
            }
            if q.is_favorite && !favorite {
                continue;
            }
            if q.is_removed && !removed {
                continue;
            }
            if q.is_hidden && !hidden {
                continue;
            }
        }

        rows.push(RowView {
            pk: m.pk,
            gateway_name: gw_name,
            model_id: m.model_id.clone(),
            owner: m.owner.clone(),
            first_seen: m.first_seen,
            last_seen: m.last_seen,
            removed_at: m.removed_at,
            favorite,
            hidden,
            tags,
            gw_count,
        });
    }

    sort_rows(&mut rows, app.sort_col, app.sort_desc);
    (rows, compare_names)
}

fn collect_rows(app: &mut App) -> Vec<RowView> {
    build_rows(app).0
}

fn sort_rows(rows: &mut [RowView], col: SortCol, desc: bool) {
    match col {
        SortCol::Model => rows.sort_by(|a, b| cmp_str(&a.model_id, &b.model_id, desc)),
        SortCol::Gateway => rows.sort_by(|a, b| cmp_str(&a.gateway_name, &b.gateway_name, desc)),
        SortCol::Owner => rows.sort_by(|a, b| {
            cmp_opt(&a.owner, &b.owner, desc).then_with(|| cmp_str(&a.model_id, &b.model_id, false))
        }),
        SortCol::FirstSeen => rows.sort_by(|a, b| cmp_i(a.first_seen, b.first_seen, desc)),
        SortCol::LastSeen => rows.sort_by(|a, b| cmp_i(a.last_seen, b.last_seen, desc)),
    }
}

fn cmp_str(a: &str, b: &str, desc: bool) -> std::cmp::Ordering {
    let o = a.to_lowercase().cmp(&b.to_lowercase());
    if desc { o.reverse() } else { o }
}

fn cmp_i(a: i64, b: i64, desc: bool) -> std::cmp::Ordering {
    let o = a.cmp(&b);
    if desc { o.reverse() } else { o }
}

fn cmp_opt(a: &Option<String>, b: &Option<String>, desc: bool) -> std::cmp::Ordering {
    let o = a.as_deref().unwrap_or("").cmp(b.as_deref().unwrap_or(""));
    if desc { o.reverse() } else { o }
}

fn empty_state(app: &mut App, ui: &mut egui::Ui) {
    ui.add_space(40.0);
    ui.vertical_centered(|ui| {
        ui.label(
            egui::RichText::new(ic::SHIELD_CHECK)
                .size(42.0)
                .color(crate::theme::accent_color(&app.config.settings.accent)),
        );
        ui.add_space(8.0);
        ui.label(egui::RichText::new("Welcome to Modelfolio").heading());
        ui.label(egui::RichText::new("Add an OpenAI-compatible gateway or an Ollama instance.\nAPI keys are stored in the OS key store — never on disk, never in the cloud.").weak());
        ui.add_space(12.0);
        let accent = crate::theme::accent_color(&app.config.settings.accent);
        if ui
            .add(
                egui::Button::new(egui::RichText::new(format!("{} Add your first gateway", ic::PLUS)).strong())
                    .fill(accent),
            )
            .clicked()
        {
            app.gateway_modal = Some(crate::ui::gateways::GatewayModal::add());
        }
    });
}
