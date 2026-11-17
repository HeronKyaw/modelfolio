//! Modelfolio — a local-only catalog for AI gateways and models.

mod app;
mod diff;
mod net;
mod notify;
mod paths;
mod providers;
mod search;
mod store;
mod theme;
mod ui;

use app::{App, Message};
use std::sync::mpsc;
use std::sync::Arc;

fn main() -> eframe::Result<()> {
    if let Err(e) = paths::ensure_dirs() {
        eprintln!("could not create data dir: {e}");
    }
    let runtime = Arc::new(
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("tokio runtime"),
    );
    let config = store::config::Config::load_or_default();
    let cache = store::cache::Cache::open(&paths::cache_path())
        .expect("open SQLite cache");
    let (tx, rx) = mpsc::channel::<Message>();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 840.0])
            .with_min_inner_size([960.0, 600.0]),
        ..Default::default()
    };

    let theme_pref = config.settings.theme;
    let accent = config.settings.accent.clone();

    eframe::run_native(
        "Modelfolio",
        options,
        Box::new(move |cc| {
            theme::init(&cc.egui_ctx, theme_pref, &accent);
            Ok(Box::new(App::new(
                cc.egui_ctx.clone(),
                runtime,
                config,
                cache,
                tx,
                rx,
            )))
        }),
    )
}
