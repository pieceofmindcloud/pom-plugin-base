//! What the node tells this plugin in `host.configure`: its own OpenAI
//! gateway and chat key, the shared workspace root, the loopback `host_api`
//! and the operator's preferences. The node resends it on every host start
//! and whenever any part changes, so the latest call always wins.

use serde_json::{json, Value};
use std::path::{Path, PathBuf};

#[derive(Clone, Default, PartialEq, Eq)]
pub struct Gateway {
    /// The node's own `/v1` on loopback.
    pub openai_base_url: String,
    /// The node's chat key: never logged, never sent to a browser.
    pub api_key: String,
}

#[derive(Clone, Default, PartialEq, Eq)]
pub struct HostApi {
    /// `http://127.0.0.1:<port>/api/plugin-host`.
    pub base_url: String,
    /// Sent as `x-pom-plugin-host-token`; it names this plugin to the node.
    pub token: String,
}

#[derive(Clone, Default)]
pub struct Context {
    pub gateway: Option<Gateway>,
    pub workspace_root: Option<PathBuf>,
    pub host_api: Option<HostApi>,
    /// `{"features": {<key>: bool}, "storage_directory": <abs path>|null}`.
    pub preferences: Value,
}

impl std::fmt::Debug for Context {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Context")
            .field(
                "gateway",
                &self.gateway.as_ref().map(|g| &g.openai_base_url),
            )
            .field("workspace_root", &self.workspace_root)
            .field("host_api", &self.host_api.as_ref().map(|h| &h.base_url))
            .field("preferences", &self.preferences)
            .finish()
    }
}

fn is_loopback(url: &str) -> bool {
    ["http://127.0.0.1:", "http://localhost:", "http://[::1]:"]
        .iter()
        .any(|prefix| url.starts_with(prefix))
}

impl Context {
    pub fn from_configure(request: &Value) -> Result<Self, String> {
        let gateway = match (
            request["gateway"]["openai_base_url"].as_str(),
            request["gateway"]["api_key"].as_str(),
        ) {
            (Some(url), Some(key)) if !url.is_empty() && !key.is_empty() => Some(Gateway {
                openai_base_url: url.trim_end_matches('/').to_owned(),
                api_key: key.to_owned(),
            }),
            _ => None,
        };
        let workspace_root = match request.get("workspace_root") {
            None | Some(Value::Null) => None,
            Some(Value::String(root)) if root.trim().is_empty() => None,
            Some(Value::String(root)) if Path::new(root).is_absolute() => Some(PathBuf::from(root)),
            Some(Value::String(_)) => return Err("workspace_root must be an absolute path".into()),
            Some(_) => return Err("workspace_root must be a string or null".into()),
        };
        // Only a loopback node API is trusted: the token must never leave the machine.
        let host_api = match (
            request["host_api"]["base_url"].as_str(),
            request["host_api"]["token"].as_str(),
        ) {
            (Some(url), Some(token)) if is_loopback(url) && !token.is_empty() => Some(HostApi {
                base_url: url.trim_end_matches('/').to_owned(),
                token: token.to_owned(),
            }),
            _ => None,
        };
        let preferences = request
            .get("preferences")
            .filter(|value| value.is_object())
            .cloned()
            .unwrap_or_else(|| json!({"features": {}, "storage_directory": null}));
        Ok(Self {
            gateway,
            workspace_root,
            host_api,
            preferences,
        })
    }

    /// A boolean toggle declared in `release/manifest.json`.
    pub fn feature(&self, key: &str) -> bool {
        self.preferences["features"][key].as_bool().unwrap_or(false)
    }

    /// Where this plugin keeps dynamic data inside the shared workspace: a
    /// hidden directory namespaced by its own code (`.<plugin_code>`).
    pub fn workspace_state_dir(&self) -> Option<PathBuf> {
        self.workspace_root
            .as_ref()
            .map(|root| root.join(format!(".{}", crate::PLUGIN_CODE)))
    }

    /// What the screens may see: never the chat key nor the host token.
    pub fn public_json(&self) -> Value {
        json!({
            "gateway": self.gateway.is_some(),
            "host_api": self.host_api.is_some(),
            "workspace_root": self.workspace_root,
            "preferences": self.preferences,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_every_part_of_host_configure() {
        let context = Context::from_configure(&json!({
            "operation": "host.configure",
            "gateway": {"openai_base_url": "http://127.0.0.1:8080/v1/", "api_key": "sk-x"},
            "workspace_root": "/data/workspace",
            "host_api": {"base_url": "http://127.0.0.1:8080/api/plugin-host", "token": "t"},
            "preferences": {"features": {"example_feature_enabled": true}, "storage_directory": null}
        }))
        .unwrap();
        assert_eq!(
            context.gateway.as_ref().unwrap().openai_base_url,
            "http://127.0.0.1:8080/v1"
        );
        assert_eq!(
            context.workspace_root,
            Some(PathBuf::from("/data/workspace"))
        );
        assert!(context.host_api.is_some());
        assert!(context.feature("example_feature_enabled"));
    }

    #[test]
    fn refuses_relative_roots_and_remote_host_apis() {
        assert!(Context::from_configure(&json!({"workspace_root": "data"})).is_err());
        assert!(Context::from_configure(&json!({"workspace_root": 4})).is_err());
        let remote = Context::from_configure(&json!({
            "host_api": {"base_url": "http://10.0.0.2:8080/api/plugin-host", "token": "t"}
        }))
        .unwrap();
        assert!(remote.host_api.is_none());
    }

    #[test]
    fn public_json_hides_the_secrets() {
        let context = Context::from_configure(&json!({
            "gateway": {"openai_base_url": "http://127.0.0.1:8080/v1", "api_key": "sk-secret"},
            "host_api": {"base_url": "http://127.0.0.1:8080/api/plugin-host", "token": "tok-secret"}
        }))
        .unwrap();
        let text = context.public_json().to_string() + &format!("{context:?}");
        assert!(!text.contains("sk-secret") && !text.contains("tok-secret"));
    }
}
