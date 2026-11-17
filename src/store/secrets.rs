//! OS secure storage for API keys.
//!
//! - Windows: Credential Manager (`keyring` windows-native)
//! - macOS:   Keychain (`keyring` apple-native)
//! - Linux:   Secret Service / gnome-keyring / KWallet (`keyring` zbus-secret-service) — best effort
//!
//! Keys never touch config.json, SQLite, the clipboard, logs or toasts.
//! In-memory buffers are zeroized after use.

use keyring::Entry;
use zeroize::Zeroizing;

const SERVICE: &str = "modelfolio";

/// Human-readable backend name for the settings screen / status bar.
pub fn backend_name() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        "Windows Credential Manager"
    }
    #[cfg(target_os = "macos")]
    {
        "macOS Keychain"
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        "Linux Secret Service (gnome-keyring/KWallet)"
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", unix)))]
    {
        "OS key store"
    }
}

fn entry(secret_id: &str) -> Result<Entry, String> {
    Entry::new(SERVICE, secret_id).map_err(|e| format!("key store unavailable: {e}"))
}

/// Store a key. `secret_id` is the account name we control (never the key itself).
pub fn store(secret_id: &str, key: &str) -> Result<(), String> {
    let e = entry(secret_id)?;
    e.set_password(key).map_err(|e| format!("could not store key: {e}"))
}

/// Load a key as None if it was never stored. The returned buffer is zeroized on drop.
pub fn load(secret_id: &str) -> Result<Option<Zeroizing<String>>, String> {
    let e = entry(secret_id)?;
    match e.get_password() {
        Ok(k) => Ok(Some(Zeroizing::new(k))),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(format!("could not read key: {e}")),
    }
}

pub fn delete(secret_id: &str) -> Result<(), String> {
    let e = entry(secret_id)?;
    match e.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(format!("could not delete key: {e}")),
    }
}

pub fn has_key(secret_id: &str) -> Result<bool, String> {
    Ok(load(secret_id)?.is_some())
}
