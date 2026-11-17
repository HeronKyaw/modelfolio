# Modelfolio

A local-only catalog for AI gateways and models. Rust + egui, no webview, no cloud, no telemetry.

- **Windows** (Credential Manager), **macOS** (Keychain) and **Linux** (best-effort) are supported.
- API keys live only in the OS key store — never on disk, never in the network beyond the gateway you save.

## What it does

- **Gateways** — OpenAI-compatible (`/v1/models`: OpenRouter, Groq, Together, LiteLLM, Fireworks, self-hosted proxies) and Ollama (`/api/tags`).
- **API keys** — entered once, stored in the OS secure store. `config.json` on disk contains only a secret *id*, never the key.
- **Tags as groups** — tag gateways and models; the sidebar filters by tag. Favorites and "hidden" are built-in tags. Deleting a tag unassigns it, it never deletes gateways or models.
- **Changelog** — every fetch diffs against the cache: **added** / **removed** / **returned**, shown as row pills and a changelog inbox with an unacknowledged badge.
- **Toasts for background fetches only** — a manual refresh never fires an OS toast (you're already looking at the result). The first import of a gateway is an import, not "new models": no toast.
- **Offline first** — the SQLite cache opens instantly without network.
- **Compare** — "on N gateways" / "only on this gateway".
- **Export** — selected models (or the current view) to JSON/CSV: ids and metadata only, never keys.

## Security model

- No telemetry, no analytics, no update phone-home.
- HTTPS required; `http://` only for `localhost` / `127.0.0.1` / `[::1]` (Ollama).
- TLS via **rustls** with system roots; invalid certificates are never accepted.
- Keys are held in zeroized memory buffers while in use and wiped after each request.
- Toasts contain counts and gateway names only — never keys or model id dumps.
- URLs with embedded credentials, query strings or fragments are rejected.

## Search operators

`llama 70b`, `tag:coding`, `gateway:openrouter`, `is:new`, `is:favorite`, `is:removed`, `is:hidden`.

## Shortcuts

| Windows / Linux | macOS | Action |
| --- | --- | --- |
| `Ctrl+K` | `⌘K` | Focus search |
| `Ctrl+R` | `⌘R` | Refresh selected gateway (or all) |
| `Ctrl+N` | `⌘N` | Add gateway |

## Data locations

| OS | Folder |
| --- | --- |
| Windows | `%APPDATA%\modelfolio` |
| macOS | `~/Library/Application Support/modelfolio` |
| Linux | `$XDG_DATA_HOME/modelfolio` |

- `config.json` — gateway metadata (no secrets)
- `cache.sqlite` — models, tags, changelog, per-gateway health
- `exports/` — JSON/CSV exports (ids and metadata only)

## Build

Requires Rust 1.88+ (and MSVC Build Tools on Windows):

```
cargo build --release
```

The binary lands at `target/release/modelfolio(.exe)`.

### Platform notes

- **macOS** — OS notifications are most reliable from a bundled `.app`; use
  `scripts/package-macos.sh` to produce `target/Modelfolio.app`. Dev builds may
  not show system toasts — the in-app overlay always works.
- **Linux** — best-effort. Key storage needs a Secret Service provider
  (gnome-keyring / KWallet); without one, the gateway panel shows an explicit
  error (no plaintext fallback). Toasts need a D-Bus notification daemon;
  without one, alerts fall back to the in-app changelog badge.

## Project layout

```
src/
  app.rs        state, fetch orchestration, background timer
  ui/           topbar, sidebar, model table, inbox, settings
  store/        config.json, keyring, SQLite cache, tags
  providers/    OpenAI-compatible + Ollama fetchers
  theme.rs      fonts, dark/light + accent theming
  net.rs        URL policy + endpoints
  diff.rs       added/removed/returned set-diff
  search.rs     query operators
  notify.rs     OS toasts (background only)
```

## Documentation

API docs: `cargo doc --open` · architecture decisions live in module-level
doc comments (`//!`), which are the source of truth for the security model.

## Contributing

PRs welcome. Please keep the security invariants intact: keys never leave the
OS key store in plaintext, no telemetry, HTTPS-only. `cargo clippy -D warnings`
and `cargo test` must pass (CI enforces this on Windows, macOS and Linux).

## License

Distributed under the [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE) license, at your option — the same dual licensing as the Rust ecosystem.

Bundled third-party assets keep their own licenses:

| Asset | License | File |
| --- | --- | --- |
| Inter typeface | SIL OFL 1.1 | [assets/fonts/LICENSE-Inter-OFL.txt](assets/fonts/LICENSE-Inter-OFL.txt) |
| JetBrains Mono typeface | SIL OFL 1.1 | [assets/fonts/LICENSE-JetBrainsMono-OFL.txt](assets/fonts/LICENSE-JetBrainsMono-OFL.txt) |
| Phosphor icons | MIT | [egui-phosphor](https://github.com/amPerl/egui-phosphor) |

All Rust dependencies are MIT/Apache-2.0 dual licensed or compatible; a full
list is in `Cargo.lock`, and `cargo license` produces a per-crate report.
