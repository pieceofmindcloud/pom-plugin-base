//! Static checks of the POM plugin contract: the two manifests agree, every
//! asset and text key exists, and the release scripts keep the shape the POM
//! and the license server expect. Mirrors what the node validates
//! (`crates/node/src/plugin_ui.rs`) so a broken manifest fails here first.

use serde_json::Value;
use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

const PLUGIN_CODE: &str = "__CODE__";

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn text(path: &str) -> String {
    fs::read_to_string(root().join(path)).unwrap_or_else(|error| panic!("{path}: {error}"))
}

fn json(path: &str) -> Value {
    serde_json::from_str(&text(path)).unwrap_or_else(|error| panic!("{path}: {error}"))
}

fn strings(value: &Value) -> Vec<&str> {
    value
        .as_array()
        .map(|items| items.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default()
}

#[test]
fn plugin_code_is_valid_and_the_same_everywhere() {
    let valid = |code: &str| {
        let mut chars = code.chars();
        chars.next().is_some_and(|c| c.is_ascii_lowercase())
            && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
            && (2..=64).contains(&code.len())
    };
    assert!(
        valid(PLUGIN_CODE),
        "plugin_code must match ^[a-z][a-z0-9_]{{1,63}}$"
    );
    assert_eq!(json("ui/manifest.json")["plugin_code"], PLUGIN_CODE);
    assert_eq!(json("release/manifest.json")["plugin_code"], PLUGIN_CODE);
    let entry = format!("pom_{PLUGIN_CODE}_plugin_v1");
    assert!(
        text("src/lib.rs").contains(&format!("fn {entry}()")),
        "missing {entry}"
    );
}

#[test]
fn ui_manifest_follows_pom_plugin_ui_v1() {
    let manifest = json("ui/manifest.json");
    assert_eq!(manifest["schema"], "pom-plugin-ui/v1");
    if let Some(lifecycle) = manifest["lifecycle"].as_str() {
        assert!(["resident", "on_demand"].contains(&lifecycle));
    }
    let assets: BTreeSet<_> = strings(&manifest["assets"]).into_iter().collect();
    for asset in &assets {
        assert!(!asset.contains("..") && !asset.starts_with('/'), "{asset}");
    }

    let routes: BTreeSet<_> = manifest["routes"]
        .as_array()
        .expect("routes")
        .iter()
        .map(|route| {
            let screen = route["screen"].as_str().expect("route screen");
            assert!(
                manifest["screens"][screen].is_object(),
                "undeclared screen {screen}"
            );
            route["path"].as_str().expect("route path")
        })
        .collect();
    let mut ids = BTreeSet::new();
    let mut menu: Vec<&Value> = manifest["menu"].as_array().expect("menu").iter().collect();
    while let Some(item) = menu.pop() {
        assert!(
            ids.insert(item["id"].as_str().expect("menu id")),
            "repeated menu id"
        );
        assert!(item["label"]["en"].is_string(), "menu label needs en");
        let to = item["to"].as_str().expect("menu to");
        assert!(routes.contains(to), "menu target {to} has no route");
        if let Some(children) = item["children"].as_array() {
            menu.extend(children);
        }
    }

    for (name, screen) in manifest["screens"].as_object().expect("screens") {
        let module = screen["module"].as_str().expect("module");
        assert!(
            assets.contains(module),
            "screen {name} module is not an asset"
        );
        for style in strings(&screen["styles"]) {
            assert!(assets.contains(style), "style {style} is not an asset");
        }
    }
    if let Some(slots) = manifest["slots"].as_object() {
        assert!(slots.len() <= 16);
        for screen in slots.values() {
            assert!(manifest["screens"][screen.as_str().unwrap()].is_object());
        }
    }
    if let Some(icon) = manifest["icon_image"].as_str() {
        assert!(icon.ends_with(".png") && assets.contains(icon));
    }
    for section in ["guides", "reference"] {
        for entry in manifest["documentation"][section]
            .as_array()
            .into_iter()
            .flatten()
        {
            let asset = entry["asset"].as_str().expect("documentation asset");
            assert!(asset.starts_with("docs/") && asset.ends_with(".md"));
            assert!(
                assets.contains(asset) && root().join(asset).is_file(),
                "{asset}"
            );
            assert!(entry["title"]["en"].is_string());
        }
    }
    for shot in strings(&manifest["screenshots"]) {
        // Loaded by the browser from GitHub; never an embedded asset.
        assert!(!assets.contains(shot));
        if !shot.starts_with("https://") {
            assert!(root().join(shot).is_file(), "missing screenshot {shot}");
        }
    }
    for step in manifest["development"]["rebuild"]
        .as_array()
        .into_iter()
        .flatten()
    {
        let script = step["run"][0].as_str().expect("rebuild step");
        assert!(
            root().join(script).is_file(),
            "missing rebuild script {script}"
        );
    }
}

#[test]
fn i18n_catalogs_share_keys_and_cover_the_screens() {
    let manifest = json("ui/manifest.json");
    let catalogs = manifest["i18n"].as_object().expect("i18n");
    assert!(catalogs.contains_key("en"), "en is required");
    let english = json(catalogs["en"].as_str().unwrap());
    let keys: BTreeSet<_> = english.as_object().unwrap().keys().cloned().collect();
    for (locale, path) in catalogs {
        let path = path.as_str().unwrap();
        assert!(
            strings(&manifest["assets"]).contains(&path),
            "{path} is not an asset"
        );
        let catalog = json(path);
        let other: BTreeSet<_> = catalog.as_object().unwrap().keys().cloned().collect();
        assert_eq!(keys, other, "{locale} keys differ from en");
        // The build adds the `<plugin_code>.` prefix; source keys stay bare.
        assert!(keys
            .iter()
            .all(|key| !key.starts_with(&format!("{PLUGIN_CODE}."))));
    }
    let screens = fs::read_dir(root().join("ui/src/screens")).unwrap();
    for file in screens.flatten() {
        let source = fs::read_to_string(file.path()).unwrap();
        // Calls of `t("key")`, not of other functions ending in `t(`.
        for (start, _) in source.match_indices("t(\"") {
            if source[..start].ends_with(|c: char| c.is_alphanumeric() || c == '_') {
                continue;
            }
            let key = source[start + 3..].split('"').next().unwrap();
            assert!(
                keys.contains(key),
                "{:?} uses missing key {key}",
                file.file_name()
            );
        }
    }
}

#[test]
fn release_contract_and_packaging_agree() {
    let release = json("release/manifest.json");
    assert_eq!(release["schema"], 1);
    assert_eq!(release["plugin_abi"], 1);
    let features = strings(&release["feature_set"]);
    assert!(features
        .iter()
        .all(|f| f.starts_with(&format!("{PLUGIN_CODE}."))));
    if let Some(toggles) = release["preferences"]["features"].as_array() {
        let keys: BTreeSet<_> = toggles.iter().map(|t| t["key"].as_str().unwrap()).collect();
        assert_eq!(keys.len(), toggles.len(), "preference keys must be unique");
        assert!(toggles.iter().all(|t| t["default"].is_boolean()));
    }

    let package = text("scripts/package.sh");
    assert!(package.contains("pom-plugin-${platform}.json"));
    assert!(package.contains("($contract[0]) + {"));
    let workflow = text(".github/workflows/publish-release.yml");
    assert!(
        workflow.contains("POM_RELEASE_TOKEN")
            && workflow.contains("if: ${{ !inputs.prerelease }}")
    );
    assert!(text("Cargo.toml").contains("crate-type = [\"cdylib\"]"));
}

#[test]
fn styles_are_scoped_and_host_access_is_centralized() {
    let css = text("ui/src/plugin.css");
    assert!(
        !css.contains(":root") && !css.contains("body"),
        "plugin CSS must not be global"
    );
    assert!(text("ui/build.mjs").contains("namespacePluginCode"));
    // Only the host helpers read the POM SDK object.
    let screens = fs::read_dir(root().join("ui/src/screens")).unwrap();
    for file in screens.flatten() {
        let source = fs::read_to_string(file.path()).unwrap();
        assert!(
            !source.contains("__POM_HOST__"),
            "{:?} reads the host directly",
            file.file_name()
        );
    }
}

#[test]
fn built_bundle_exists_when_required() {
    if std::env::var("POM_UI_REQUIRE_DIST").as_deref() == Ok("1") {
        for path in [
            "ui/dist/screens.js",
            "ui/dist/plugin.css",
            "ui/dist/i18n/en.json",
        ] {
            assert!(
                root().join(path).is_file(),
                "missing {path}: run scripts/build-ui.sh"
            );
        }
    }
}
