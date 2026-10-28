//! __NAME__: a POM plugin.
//!
//! `pom-plugin-host` loads this library through the C ABI in `abi.rs` and
//! sends every node request to `Plugin::query` as `{"operation": ..}`. The
//! operation families are the POM contract (`docs/subsystems/plugins-ui.md`
//! in the POM repository):
//!
//! - `host.*` - the node configures and informs this plugin;
//! - `ui.*` - the manifest and the embedded assets of the screens;
//! - `rpc.*` - the plugin's own API for its screens, with the caller's `profile`;
//! - `peer.*` - calls from this plugin on other machines (none yet).
//!
//! `query` must answer at once: the node waits 3 s and one slow call blocks
//! every other. Slow work belongs to a thread this plugin owns.

mod abi;
mod context;

use context::Context;
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{PoisonError, RwLock};

pub const PLUGIN_CODE: &str = "__CODE__";
const UI_MANIFEST: &str = include_str!("../ui/manifest.json");
/// Protocol of the events the POM delivers through `host.event`.
const EVENTS_PROTOCOL: &str = "pom-plugin-events/v1";
/// How many recent events and ingested items the status keeps.
const MAX_RECENT: usize = 20;

include!(concat!(env!("OUT_DIR"), "/ui_assets.rs"));

/// The symbol `pom-plugin-host` resolves from the manifest's `plugin_code`.
///
/// # Safety
///
/// Returns a pointer to a static; the host only reads it.
#[no_mangle]
pub unsafe extern "C" fn pom___CODE___plugin_v1() -> *const abi::PluginApiV1 {
    &abi::API
}

pub struct Plugin {
    context: RwLock<Context>,
    events: RwLock<VecDeque<Value>>,
    ingested: RwLock<u64>,
}

impl Plugin {
    /// `config` is the host's creation JSON; it may be null.
    pub fn new(_config: &Value) -> Result<Self, String> {
        Ok(Self {
            context: RwLock::new(Context::default()),
            events: RwLock::new(VecDeque::new()),
            ingested: RwLock::new(0),
        })
    }

    fn context(&self) -> Context {
        self.context
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Events the node pushes with `ingest` (`{"type": ..}`), in order. The
    /// node retries on failure, so handling must be idempotent.
    pub fn ingest(&self, event: &Value) -> Result<(), String> {
        event["type"].as_str().ok_or("event has no type")?;
        *self
            .ingested
            .write()
            .unwrap_or_else(PoisonError::into_inner) += 1;
        Ok(())
    }

    pub fn query(&self, request: &Value) -> Result<Value, String> {
        match request["operation"].as_str().unwrap_or_default() {
            "host.configure" => {
                let context = Context::from_configure(request)?;
                *self.context.write().unwrap_or_else(PoisonError::into_inner) = context;
                Ok(json!({"status": "ok"}))
            }
            "host.event" => self.record_event(request),
            // What this plugin serves to the cluster; the dashboard draws each
            // entry next to the machine. Empty until the plugin runs something.
            "host.services" => Ok(json!({"services": []})),
            "ui.manifest" => ui_manifest(),
            "ui.asset" => {
                let path = request["path"].as_str().ok_or("asset path is missing")?;
                let (_, content_type, bytes) = ui_asset(path).ok_or("unknown asset")?;
                Ok(json!({"content_type": content_type, "base64": encode_base64(bytes)}))
            }
            "rpc.status" => Ok(self.status()),
            "rpc.events.clear" => {
                if let Err(refusal) = require_admin(request) {
                    return Ok(refusal);
                }
                self.events
                    .write()
                    .unwrap_or_else(PoisonError::into_inner)
                    .clear();
                Ok(json!({"status": "ok"}))
            }
            _ => Err("unknown operation".into()),
        }
    }

    fn record_event(&self, request: &Value) -> Result<Value, String> {
        let event = request.get("event").ok_or("host.event has no event")?;
        if event["protocol"] != EVENTS_PROTOCOL {
            return Err("unsupported events protocol".into());
        }
        let kind = event["type"].as_str().ok_or("event type is missing")?;
        if kind == "preferences.changed" {
            // `host.configure` follows with the same values; keep them now so
            // a reader between the two calls already sees the change.
            let preferences = event["payload"]["preferences"].clone();
            if preferences.is_object() {
                self.context
                    .write()
                    .unwrap_or_else(PoisonError::into_inner)
                    .preferences = preferences;
            }
        }
        let mut events = self.events.write().unwrap_or_else(PoisonError::into_inner);
        events.push_front(event.clone());
        events.truncate(MAX_RECENT);
        Ok(json!({"status": "ok"}))
    }

    fn status(&self) -> Value {
        let context = self.context();
        json!({
            "plugin_code": PLUGIN_CODE,
            "version": env!("CARGO_PKG_VERSION"),
            "context": context.public_json(),
            "data_directory": plugin_directory().ok(),
            "workspace_state": context.workspace_state_dir(),
            "example_feature_enabled": context.feature("example_feature_enabled"),
            "ingested": *self.ingested.read().unwrap_or_else(PoisonError::into_inner),
            "events": self.events.read().unwrap_or_else(PoisonError::into_inner).iter().cloned().collect::<Vec<_>>(),
        })
    }
}

/// `rpc.*` requests carry the node's persisted `profile` (`admin`, `worker`
/// or `visitor`); the plugin decides who may call what. The refusal is an
/// `Ok` body so the screen can show the reason.
fn require_admin(request: &Value) -> Result<(), Value> {
    if request["profile"] == "admin" {
        Ok(())
    } else {
        Err(json!({"error": "only the administrator can do this", "code": "admin_required"}))
    }
}

/// `<directory of POM_PLUGIN_DB>/<plugin_code>`: fixed files of this plugin
/// (downloaded runtimes, logs, databases). Dynamic user data goes to the
/// workspace instead (`Context::workspace_state_dir`).
pub fn plugin_directory() -> Result<PathBuf, String> {
    let parent = std::env::var_os("POM_PLUGIN_DB")
        .map(PathBuf::from)
        .and_then(|database| database.parent().map(Path::to_path_buf))
        .filter(|parent| !parent.as_os_str().is_empty());
    let parent = match parent {
        Some(parent) => parent,
        None => std::env::current_dir().map_err(|error| error.to_string())?,
    };
    Ok(parent.join(PLUGIN_CODE))
}

fn ui_asset(path: &str) -> Option<&'static (&'static str, &'static str, &'static [u8])> {
    UI_ASSETS.iter().find(|(name, _, _)| *name == path)
}

fn ui_manifest() -> Result<Value, String> {
    let manifest: Value = serde_json::from_str(UI_MANIFEST).map_err(|error| error.to_string())?;
    let assets = manifest["assets"]
        .as_array()
        .ok_or("manifest has no assets")?;
    for asset in assets.iter().filter_map(Value::as_str) {
        if ui_asset(asset).is_none() {
            return Err(format!("asset is not embedded: {asset}"));
        }
    }
    Ok(manifest)
}

fn encode_base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let value = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        for index in 0..4 {
            if index <= chunk.len() {
                output.push(TABLE[((value >> (18 - 6 * index)) & 63) as usize] as char);
            } else {
                output.push('=');
            }
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plugin() -> Plugin {
        Plugin::new(&Value::Null).unwrap()
    }

    #[test]
    fn entry_point_is_named_after_the_plugin_code() {
        let api = unsafe { &*pom___CODE___plugin_v1() };
        assert_eq!(api.abi_version, abi::ABI_VERSION);
    }

    #[test]
    fn base64_uses_standard_padding() {
        assert_eq!(encode_base64(b"f"), "Zg==");
        assert_eq!(encode_base64(b"fo"), "Zm8=");
        assert_eq!(encode_base64(b"foo"), "Zm9v");
    }

    #[test]
    fn manifest_resolves_every_embedded_asset() {
        if ui_asset("ui/screens.js").is_some() {
            assert!(ui_manifest().is_ok());
        } else {
            // Without `scripts/build-ui.sh` the bundle is missing and the
            // node must not list the plugin's screens.
            assert!(ui_manifest().is_err());
        }
    }

    #[test]
    fn configure_then_status_never_exposes_secrets() {
        let plugin = plugin();
        plugin
            .query(&json!({
                "operation": "host.configure",
                "gateway": {"openai_base_url": "http://127.0.0.1:8080/v1", "api_key": "sk-secret"},
                "workspace_root": "/data/workspace"
            }))
            .unwrap();
        let status = plugin.query(&json!({"operation": "rpc.status"})).unwrap();
        assert_eq!(status["context"]["gateway"], true);
        assert_eq!(
            status["workspace_state"],
            format!("/data/workspace/.{PLUGIN_CODE}")
        );
        assert!(!status.to_string().contains("sk-secret"));
    }

    #[test]
    fn host_event_keeps_recent_envelopes_and_applies_preferences() {
        let plugin = plugin();
        let event = |id: usize, kind: &str, payload: Value| {
            json!({"operation": "host.event", "event": {
                "protocol": EVENTS_PROTOCOL, "id": format!("e{id}"), "type": kind,
                "target": PLUGIN_CODE, "source": "pom", "at": "2026-10-01T00:00:00Z", "payload": payload
            }})
        };
        plugin
            .query(&event(
                0,
                "preferences.changed",
                json!({"preferences": {"features": {"x": true}}}),
            ))
            .unwrap();
        assert!(plugin.context().feature("x"));
        for id in 1..=MAX_RECENT {
            plugin
                .query(&event(id, "theme.changed", json!({"theme": "dark"})))
                .unwrap();
        }
        let status = plugin.status();
        assert_eq!(status["events"].as_array().unwrap().len(), MAX_RECENT);
        assert_eq!(status["events"][0]["id"], format!("e{MAX_RECENT}"));
        assert!(plugin
            .query(&json!({"operation": "host.event", "event": {"protocol": "other", "type": "x"}}))
            .is_err());
    }

    #[test]
    fn rpc_refusals_are_readable_bodies() {
        let plugin = plugin();
        let refused = plugin
            .query(&json!({"operation": "rpc.events.clear", "params": null, "profile": "visitor", "node_id": "n"}))
            .unwrap();
        assert_eq!(refused["code"], "admin_required");
        let done = plugin
            .query(&json!({"operation": "rpc.events.clear", "params": null, "profile": "admin", "node_id": "n"}))
            .unwrap();
        assert_eq!(done["status"], "ok");
    }

    #[test]
    fn ingest_requires_a_typed_event() {
        let plugin = plugin();
        assert!(plugin.ingest(&json!({"type": "begin"})).is_ok());
        assert!(plugin.ingest(&json!({})).is_err());
        assert_eq!(plugin.status()["ingested"], 1);
    }

    #[test]
    fn unknown_operations_fail() {
        assert!(plugin().query(&json!({"operation": "nope"})).is_err());
    }
}
