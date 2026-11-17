//! Application state, fetch orchestration, background timer, and panel layout.

use crate::providers;
use crate::store::cache::{Cache, DiffSummary};
use crate::store::config::{Config, Gateway, ThemePref};
use crate::store::secrets;
use crate::ui;
use egui::Context;
use std::collections::HashSet;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;
use std::time::Duration;
use zeroize::Zeroizing;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    Manual,
    Background,
}

pub enum Message {
    Done { gateway_id: String, trigger: Trigger, result: Result<DiffSummary, String> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    All,
    Favorites,
    New,
    Removed,
    Hidden,
    Tag(i64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compare {
    Any,
    Multi,
    OnlySelected,
}

impl Compare {
    pub fn label(self) -> &'static str {
        match self {
            Compare::Any => "Any",
            Compare::Multi => "On multiple gateways",
            Compare::OnlySelected => "Only on selected gateway",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortCol {
    Model,
    Gateway,
    Owner,
    FirstSeen,
    LastSeen,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportKind {
    Json,
    Csv,
}

#[derive(Debug, Clone)]
pub struct InAppToast {
    pub title: String,
    pub body: String,
    pub born: i64,
}

pub struct App {
    pub ctx: egui::Context,
    pub runtime: Arc<tokio::runtime::Runtime>,
    pub http: reqwest::Client,
    pub config: Config,
    pub cache: Cache,
    pub tx: Sender<Message>,
    pub rx: Receiver<Message>,

    pub fetching: HashSet<String>,
    pub next_bg_fetch: i64,
    pub last_bg_fetch: Option<i64>,
    pub last_theme_check: i64,

    pub view: View,
    pub selected_gateway: Option<String>,
    pub search_text: String,
    pub search_focus: bool,
    pub compare: Compare,
    pub sort_col: SortCol,
    pub sort_desc: bool,
    pub selected_rows: HashSet<i64>,
    pub export_kind: ExportKind,

    pub toasts: Vec<InAppToast>,
    pub status: String,
    pub notice: Option<String>,

    pub gateway_modal: Option<ui::gateways::GatewayModal>,
    pub confirm_delete: Option<String>,
    pub settings_open: bool,
    pub inbox_open: bool,
    pub new_tag_open: bool,
    pub new_tag_name: String,
    pub rename_tag: Option<(i64, String)>,
}

impl App {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        ctx: egui::Context,
        runtime: Arc<tokio::runtime::Runtime>,
        config: Config,
        cache: Cache,
        tx: Sender<Message>,
        rx: Receiver<Message>,
    ) -> Self {
        let http = reqwest::Client::builder()
            .user_agent("modelfolio/0.1")
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .build()
            .expect("http client");
        let now = now_ts();
        // One background pass shortly after launch, then on the interval.
        let next_bg_fetch = now + 90;
        App {
            ctx,
            runtime,
            http,
            config,
            cache,
            tx,
            rx,
            fetching: HashSet::new(),
            next_bg_fetch,
            last_bg_fetch: None,
            last_theme_check: now,
            view: View::All,
            selected_gateway: None,
            search_text: String::new(),
            search_focus: false,
            compare: Compare::Any,
            sort_col: SortCol::Model,
            sort_desc: false,
            selected_rows: HashSet::new(),
            export_kind: ExportKind::Json,
            toasts: Vec::new(),
            status: String::new(),
            notice: None,
            gateway_modal: None,
            confirm_delete: None,
            settings_open: false,
            inbox_open: false,
            new_tag_open: false,
            new_tag_name: String::new(),
            rename_tag: None,
        }
    }
}

pub fn now_ts() -> i64 {
    chrono::Utc::now().timestamp()
}

pub fn time_ago(ts: i64) -> String {
    let d = now_ts() - ts;
    if d < 0 {
        return "just now".into();
    }
    if d < 60 {
        "just now".into()
    } else if d < 3600 {
        format!("{}m ago", d / 60)
    } else if d < 86400 {
        format!("{}h ago", d / 3600)
    } else {
        format!("{}d ago", d / 86400)
    }
}

pub fn fmt_ts(ts: i64) -> String {
    use chrono::{Local, TimeZone};
    match Local.timestamp_opt(ts, 0).single() {
        Some(t) => t.format("%Y-%m-%d %H:%M").to_string(),
        None => ts.to_string(),
    }
}

impl App {
    // ---- orchestration ----------------------------------------------------

    pub fn trigger_fetch(&mut self, trigger: Trigger, only: Option<String>) {
        let gateways: Vec<Gateway> = self
            .config
            .gateways
            .iter()
            .filter(|g| g.enabled && only.as_deref().is_none_or(|id| g.id == id))
            .cloned()
            .collect();
        if gateways.is_empty() {
            let msg = if only.is_some() { "That gateway is disabled" } else { "No enabled gateways to refresh" };
            self.set_status(msg);
            return;
        }
        let sem = Arc::new(tokio::sync::Semaphore::new(4));
        let now = now_ts();
        for gw in gateways {
            if self.fetching.contains(&gw.id) {
                continue;
            }
            self.fetching.insert(gw.id.clone());
            let http = self.http.clone();
            let cache = self.cache.clone();
            let tx = self.tx.clone();
            let sem = sem.clone();
            self.runtime.spawn(async move {
                let _permit = sem.acquire().await.unwrap();
                let result = fetch_one(&http, &cache, &gw).await;
                let _ = tx.send(Message::Done { gateway_id: gw.id, trigger, result });
            });
        }
        if trigger == Trigger::Background {
            self.last_bg_fetch = Some(now);
            self.schedule_next_background(now);
        }
    }

    pub fn schedule_next_background(&mut self, now: i64) {
        let interval = self.config.settings.background_interval_min.max(15) as i64 * 60;
        self.next_bg_fetch = now + interval;
    }

    fn maybe_background_fetch(&mut self) {
        let now = now_ts();
        if now < self.next_bg_fetch {
            return;
        }
        if self.fetching.is_empty() && self.config.gateways.iter().any(|g| g.enabled) {
            self.trigger_fetch(Trigger::Background, None);
        } else {
            self.schedule_next_background(now);
        }
    }

    fn handle_messages(&mut self) {
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                Message::Done { gateway_id, trigger, result } => {
                    self.on_done(&gateway_id, trigger, result);
                }
            }
        }
    }

    fn on_done(&mut self, gateway_id: &str, trigger: Trigger, result: Result<DiffSummary, String>) {
        self.fetching.remove(gateway_id);
        let name = self
            .config
            .gateway(gateway_id)
            .map(|g| g.name.clone())
            .unwrap_or_else(|| gateway_id.to_string());
        match result {
            Ok(s) => {
                self.cache.set_gateway_ok(gateway_id, now_ts());
                if trigger == Trigger::Manual {
                    if s.imported {
                        self.set_status(format!("Imported {} models from {}", s.total, name));
                    } else if s.added.is_empty() && s.removed.is_empty() && s.returned.is_empty() {
                        self.set_status(format!("Up to date — {} ({} models)", name, s.total));
                    } else {
                        self.set_status(format!(
                            "{}: +{} new, −{} removed, ↻{} returned (manual — no toast)",
                            name,
                            s.added.len(),
                            s.removed.len(),
                            s.returned.len()
                        ));
                        if !s.added.is_empty() {
                            self.notice = Some(format!("{} new on {}", s.added.len(), name));
                        }
                    }
                } else {
                    // Background: in-app highlight always; OS toast per settings.
                    let include_removed = self.config.settings.toast_include_removed;
                    let added_n = s.added.len();
                    let removed_n = s.removed.len();
                    if added_n > 0 || (removed_n > 0 && include_removed) {
                        let mut parts = Vec::new();
                        if added_n > 0 {
                            parts.push(format!("{} new", added_n));
                        }
                        if removed_n > 0 && include_removed {
                            parts.push(format!("{} removed", removed_n));
                        }
                        let body = format!("{}: {}", name, parts.join(", "));
                        self.push_toast("Model changes found", &body);
                        if self.config.settings.background_toasts {
                            let title = "Modelfolio".to_string();
                            let body = body.clone();
                            self.runtime.spawn_blocking(move || {
                                if let Err(e) = crate::notify::send_toast(&title, &body) {
                                    eprintln!("{e}");
                                }
                            });
                        }
                    }
                }
            }
            Err(e) => {
                self.cache.set_gateway_error(gateway_id, &e, now_ts());
                self.set_status(format!("{} — {}", name, e));
            }
        }
    }

    // ---- small helpers ------------------------------------------------------

    pub fn set_status(&mut self, s: impl Into<String>) {
        self.status = s.into();
    }

    pub fn push_toast(&mut self, title: &str, body: &str) {
        self.toasts.push(InAppToast { title: title.into(), body: body.into(), born: now_ts() });
    }

    pub fn copy_text(&mut self, s: &str) {
        match arboard::Clipboard::new() {
            Ok(mut cb) => match cb.set_text(s.to_owned()) {
                Ok(()) => self.set_status(format!("Copied: {}", truncate(s, 80))),
                Err(e) => self.set_status(format!("Clipboard write failed: {e}")),
            },
            Err(e) => self.set_status(format!("Clipboard unavailable: {e}")),
        }
    }

    pub fn save_config(&mut self) {
        if let Err(e) = self.config.save() {
            self.set_status(format!("Config save failed: {e}"));
        } else {
            self.apply_theme();
        }
    }

    pub fn apply_theme(&self) {
        crate::theme::apply(&self.ctx, self.config.settings.theme, &self.config.settings.accent);
    }

    fn maybe_refresh_theme(&mut self, ctx: &Context) {
        if self.config.settings.theme != ThemePref::System {
            return;
        }
        let now = now_ts();
        if now - self.last_theme_check >= 30 {
            self.last_theme_check = now;
            crate::theme::apply(ctx, ThemePref::System, &self.config.settings.accent);
        }
    }

    fn handle_shortcuts(&mut self, ctx: &Context) {
        let mut act: Option<u8> = None;
        ctx.input(|i| {
            let m = i.modifiers;
            if !m.command {
                return;
            }
            for (key, code) in [
                (egui::Key::K, b'K'),
                (egui::Key::R, b'R'),
                (egui::Key::N, b'N'),
            ] {
                if i.key_pressed(key) {
                    act = Some(code);
                }
            }
        });
        let slash = ctx.input(|i| i.key_pressed(egui::Key::Slash)) && !ctx.egui_wants_keyboard_input();
        let esc = ctx.input(|i| i.key_pressed(egui::Key::Escape));
        match act {
            Some(b'K') => self.search_focus = true,
            Some(b'R') => self.trigger_fetch(Trigger::Manual, self.selected_gateway.clone()),
            Some(b'N') => self.gateway_modal = Some(ui::gateways::GatewayModal::add()),
            _ => {}
        }
        if slash {
            self.search_focus = true;
        }
        if esc {
            self.settings_open = false;
            self.inbox_open = false;
            self.gateway_modal = None;
            self.confirm_delete = None;
        }
    }

    pub fn export_rows(&mut self, rows: &[ui::models::RowView]) {
        if rows.is_empty() {
            self.set_status("Nothing to export");
            return;
        }
        let dir = crate::paths::exports_dir();
        let ts = chrono::Local::now().format("%Y%m%d-%H%M%S");
        let (path, body) = match self.export_kind {
            ExportKind::Json => {
                let items: Vec<serde_json::Value> = rows
                    .iter()
                    .map(|r| {
                        serde_json::json!({
                            "gateway": r.gateway_name,
                            "model_id": r.model_id,
                            "owned_by": r.owner,
                            "first_seen": r.first_seen,
                            "last_seen": r.last_seen,
                            "tags": r.tags,
                        })
                    })
                    .collect();
                (
                    dir.join(format!("models-{ts}.json")),
                    serde_json::to_string_pretty(&items).unwrap_or_default(),
                )
            }
            ExportKind::Csv => {
                let mut out = String::from("gateway,model_id,owned_by,first_seen,last_seen,tags\n");
                for r in rows {
                    out.push_str(&csv_line(&[
                        &r.gateway_name,
                        &r.model_id,
                        r.owner.as_deref().unwrap_or(""),
                        &r.first_seen.to_string(),
                        &r.last_seen.to_string(),
                        &r.tags.join(";"),
                    ]));
                }
                (dir.join(format!("models-{ts}.csv")), out)
            }
        };
        match std::fs::write(&path, body) {
            Ok(()) => self.set_status(format!("Exported {} models → {}", rows.len(), path.display())),
            Err(e) => self.set_status(format!("Export failed: {e}")),
        }
    }
}

fn csv_line(fields: &[&str]) -> String {
    let quoted: Vec<String> = fields
        .iter()
        .map(|f| format!("\"{}\"", f.replace('"', "\"\"")))
        .collect();
    quoted.join(",") + "\n"
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        s.chars().take(n).collect::<String>() + "…"
    }
}

/// The tokio task that does the actual work for one gateway.
async fn fetch_one(
    http: &reqwest::Client,
    cache: &Cache,
    gw: &Gateway,
) -> Result<DiffSummary, String> {
    let base = crate::net::validate_base(&gw.base_url)?;
    let key: Option<Zeroizing<String>> = match &gw.secret_id {
        Some(sid) => {
            let sid = sid.clone();
            tokio::task::spawn_blocking(move || secrets::load(&sid))
                .await
                .map_err(|e| format!("key store task failed: {e}"))??
        }
        None => None,
    };
    let items: Vec<providers::ModelItem> =
        providers::fetch(gw.kind, &base, key, http).await?;
    cache.apply_fetch(&gw.id, &items, now_ts())
}

impl eframe::App for App {
    /// Non-UI work: drain fetch results, shortcuts, background timer, theme.
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_messages();
        self.handle_shortcuts(ctx);
        self.maybe_background_fetch();
        self.maybe_refresh_theme(ctx);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::top("topbar").show(ui, |ui| crate::ui::topbar(self, ui));
        egui::Panel::bottom("statusbar").show(ui, |ui| crate::ui::statusbar(self, ui));
        egui::Panel::left("sidebar")
            .resizable(true)
            .default_size(250.0)
            .min_size(200.0)
            .show(ui, |ui| crate::ui::sidebar(self, ui));
        egui::CentralPanel::default_margins().show(ui, |ui| crate::ui::models::central(self, ui));

        // Windows/modals still render against the Context.
        let ctx = ui.ctx().clone();
        crate::ui::gateways::show_gateway_modal(self, &ctx);
        crate::ui::settings::window(self, &ctx);
        crate::ui::changelog::window(self, &ctx);
        crate::ui::tag_modals(self, &ctx);
        crate::ui::confirm_delete(self, &ctx);
        crate::ui::draw_toasts(self, &ctx);

        // Keep the background timer and spinner alive between interactions.
        ui.ctx().request_repaint_after(Duration::from_millis(500));
    }
}
