use serde_json::Value;
use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn text(path: &str) -> String {
    fs::read_to_string(root().join(path)).unwrap_or_else(|error| panic!("{path}: {error}"))
}

fn json(path: &str) -> Value {
    serde_json::from_str(&text(path)).unwrap_or_else(|error| panic!("{path}: {error}"))
}

fn contains_term(haystack: &[u8], pattern: &[u8]) -> bool {
    haystack
        .windows(pattern.len())
        .enumerate()
        .any(|(start, window)| {
            if !window.eq_ignore_ascii_case(pattern) {
                return false;
            }
            let is_word = |byte: u8| byte.is_ascii_alphanumeric() || byte >= 128;
            let before_is_word = start > 0 && is_word(haystack[start - 1]);
            let end = start + pattern.len();
            let after_is_word = end < haystack.len() && is_word(haystack[end]);
            !before_is_word && !after_is_word
        })
}

#[test]
fn manifest_registers_the_tutorial_section_with_two_submenus() {
    let manifest = json("ui/manifest.json");
    assert_eq!(manifest["schema"], "pom-plugin-ui/v1");
    assert_eq!(manifest["plugin_code"], "base");
    assert_eq!(manifest["icon_image"], "ui/icon.png");
    assert_eq!(
        manifest["documentation"],
        serde_json::json!([
            "docs/README.md",
            "docs/guia-desenvolvimento.md",
            "docs/guia-workspace.md",
            "docs/guia-eventos.md"
        ])
    );

    let menu = manifest["menu"].as_array().expect("menu array");
    assert_eq!(menu.len(), 1);
    let section = &menu[0];
    assert_eq!(section["id"], "base");
    assert_eq!(section["label"]["en"], "POM - Plugins");
    assert_eq!(section["label"]["pt-BR"], "POM - Plugins");
    // Hosts without submenu support still get a valid destination.
    assert_eq!(section["to"], "/plugin");
    let children = section["children"].as_array().expect("submenus");
    let ids: Vec<_> = children
        .iter()
        .map(|child| child["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["plugin", "projects"]);
    assert_eq!(children[1]["label"]["pt-BR"], "Projetos");

    let routes = manifest["routes"].as_array().expect("routes array");
    assert_eq!(routes.len(), 2);
    for (path, screen) in [("/plugin", "tutorial"), ("/projetos", "projects")] {
        let route = routes
            .iter()
            .find(|route| route["path"] == path)
            .expect("route");
        assert_eq!(route["screen"], screen);
        assert_eq!(route["full_bleed"], true);
        assert!(children.iter().any(|child| child["to"] == path));
        let descriptor = &manifest["screens"][screen];
        assert_eq!(descriptor["module"], "ui/screens.js");
        assert_eq!(descriptor["export"], screen);
        assert!(descriptor["styles"]
            .as_array()
            .unwrap()
            .iter()
            .any(|style| style == "ui/plugin.css"));
    }
}

#[test]
fn manifest_description_and_screenshots_live_in_the_repository() {
    let manifest = json("ui/manifest.json");
    for locale in ["en", "pt-BR"] {
        let description = manifest["description"][locale]
            .as_str()
            .expect("description");
        assert!(!description.trim().is_empty() && description.len() <= 2000);
    }
    let screenshots = manifest["screenshots"].as_array().expect("screenshots");
    assert!(!screenshots.is_empty() && screenshots.len() <= 8);
    let assets = manifest["assets"].as_array().unwrap();
    for shot in screenshots {
        let path = shot.as_str().unwrap();
        // Loaded by the browser straight from GitHub, never embedded or served by the node.
        assert!(path.starts_with("docs/screenshots/") && path.ends_with(".png"));
        assert!(!assets.iter().any(|asset| asset == shot));
        let bytes = fs::read(root().join(path)).unwrap_or_else(|error| panic!("{path}: {error}"));
        assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
    }
}

#[test]
fn catalogs_assets_and_screen_text_keys_are_complete() {
    let manifest = json("ui/manifest.json");
    assert_eq!(manifest["i18n"]["en"], "i18n/en.json");
    assert_eq!(manifest["i18n"]["pt-BR"], "i18n/pt-BR.json");

    let assets: BTreeSet<_> = manifest["assets"]
        .as_array()
        .unwrap()
        .iter()
        .map(|asset| asset.as_str().unwrap())
        .collect();
    assert_eq!(
        assets,
        BTreeSet::from([
            "ui/screens.js",
            "ui/plugin.css",
            "ui/icon.png",
            "ui/pom-mark-dark.png",
            "i18n/en.json",
            "i18n/pt-BR.json",
            "docs/README.md",
            "docs/guia-desenvolvimento.md",
            "docs/guia-workspace.md",
            "docs/guia-eventos.md",
        ])
    );
    assert!(root().join("ui/src/screens/index.tsx").is_file());
    assert!(root().join("ui/src/plugin.css").is_file());
    let icon = fs::read(root().join("ui/icon.png")).expect("plugin icon");
    assert!(icon.starts_with(b"\x89PNG\r\n\x1a\n"));
    for path in [
        "docs/README.md",
        "docs/guia-desenvolvimento.md",
        "docs/guia-workspace.md",
        "docs/guia-eventos.md",
    ] {
        assert!(root().join(path).is_file(), "missing {path}");
    }
    let screen_index = text("ui/src/screens/index.tsx");
    assert!(screen_index.contains("Tutorial as tutorial"));
    assert!(screen_index.contains("Projects as projects"));

    let en = json("i18n/en.json");
    let pt = json("i18n/pt-BR.json");
    assert_eq!(
        en.as_object().unwrap().keys().collect::<Vec<_>>(),
        pt.as_object().unwrap().keys().collect::<Vec<_>>()
    );
    let catalog_keys: BTreeSet<_> = en.as_object().unwrap().keys().cloned().collect();
    let mut used = BTreeSet::new();
    for path in ["ui/src/screens/Tutorial.tsx", "ui/src/screens/Projects.tsx"] {
        let source = text(path);
        // `t("key"` only when `t` is a whole identifier, not the end of
        // `usePomEvent("...` or similar calls.
        for (index, _) in source.match_indices("t(\"") {
            let before = source[..index].chars().next_back();
            if before.is_some_and(|c| c.is_alphanumeric() || c == '_') {
                continue;
            }
            let rest = &source[index + 3..];
            let key = rest.split_once('\"').expect("translation key").0;
            used.insert(key.to_owned());
        }
    }
    // Keys built from the step, feature and response lists of the tutorial.
    for step in ["one", "two", "three", "four", "five"] {
        for field in ["title", "body", "file"] {
            used.insert(format!("steps.{step}.{field}"));
        }
    }
    for feature in [
        "menu",
        "screens",
        "locales",
        "workspace",
        "preferences",
        "events",
        "notifications",
        "store",
        "loader",
    ] {
        used.insert(format!("features.{feature}.title"));
        used.insert(format!("features.{feature}.body"));
    }
    // The steps of the simulated install (`LOADER_STEPS`).
    for step in ["download", "unpack", "configure"] {
        used.insert(format!("demo.loaderStep.{step}"));
    }
    for action in ["delivered", "failed", "accepted", "cancelled"] {
        used.insert(format!("demo.response.{action}"));
    }
    for theme in ["light", "dark"] {
        used.insert(format!("theme.{theme}"));
    }
    assert_eq!(used, catalog_keys);
    assert_eq!(
        en["projects.body"],
        "The POM provides this workspace. POM - Plugins reads only its immediate project directories."
    );
    assert_eq!(
        pt["projects.body"],
        "O POM fornece este workspace. O POM - Plugins lê apenas os diretórios de projetos diretamente nele."
    );
}

#[test]
fn i18n_namespace_comes_from_manifest_plugin_code() {
    let manifest = json("ui/manifest.json");
    let plugin_code = manifest["plugin_code"].as_str().expect("plugin code");
    let en = json("i18n/en.json");
    let pt = json("i18n/pt-BR.json");
    let english_keys = en.as_object().expect("English catalog");
    let portuguese_keys = pt.as_object().expect("Portuguese catalog");

    assert!(english_keys.contains_key("hero.titleA"));
    assert!(english_keys.contains_key("projects.body"));
    assert!(!english_keys.keys().any(|key| key.starts_with("base.")));
    assert_eq!(
        english_keys.keys().collect::<Vec<_>>(),
        portuguese_keys.keys().collect::<Vec<_>>()
    );

    let runtime = text("ui/src/host/runtime.ts");
    assert!(runtime.contains("${pluginCode}.${key}"));
    let build = text("ui/build.mjs");
    assert!(build.contains("manifest.plugin_code"));
    assert!(build.contains("namespaceLocaleCatalog"));

    let generated = root().join("ui/dist/i18n/en.json");
    if generated.exists() {
        let generated: Value = serde_json::from_slice(&fs::read(generated).unwrap()).unwrap();
        assert!(generated
            .get(format!("{plugin_code}.hero.titleA"))
            .is_some());
        assert!(generated
            .get(format!("{plugin_code}.projects.body"))
            .is_some());
    }
}

#[test]
fn runtime_speaks_the_pom_plugin_events_protocol() {
    let runtime = text("ui/src/host/runtime.ts");
    assert!(runtime.contains("\"pom-plugin-events/v1\""));
    for export in [
        "export function onPomEvent",
        "export function usePomEvent",
        "export function usePomContext",
        "export function notify",
        "export function confirm",
    ] {
        assert!(runtime.contains(export), "missing {export}");
    }
    // Hosts without the v2 SDK are still reached through the DOM channel.
    assert!(runtime.contains("\"pom:plugin-event\""));
    assert!(runtime.contains("\"pom:plugin-request\""));
    let tutorial = text("ui/src/screens/Tutorial.tsx");
    for call in [
        "notify({",
        "confirm({",
        "scope: \"network\"",
        "usePomEvent(\"*\"",
    ] {
        assert!(tutorial.contains(call), "tutorial misses {call}");
    }
    let native = text("src/lib.rs");
    assert!(native.contains("\"host.event\""));
    assert!(native.contains("path == \"/events\""));
}

#[test]
fn plugin_styles_do_not_write_global_rules_and_get_plugin_scoped_names() {
    let css = text("ui/src/plugin.css");
    assert!(!css.contains(":root"));
    let build = text("ui/build.mjs");
    assert!(build.contains("namespacePluginCode"));
    assert!(build.contains("pluginCode"));
    let native_build = text("build.rs");
    assert!(native_build.contains("image/png"));
    assert!(native_build.contains("text/markdown"));
}

#[test]
fn release_manifest_provides_typed_default_preference_examples() {
    let manifest = json("release/manifest.json");
    assert_eq!(manifest["schema"], 1);
    assert_eq!(manifest["plugin_code"], "base");
    assert_eq!(manifest["feature_set"], serde_json::json!(["base.core"]));

    let preferences = &manifest["preferences"];
    let features = preferences["features"]
        .as_array()
        .expect("boolean preferences");
    assert_eq!(features.len(), 1);
    assert_eq!(features[0]["key"], "example_feature_enabled");
    assert_eq!(features[0]["default"], false);
    assert_eq!(
        preferences["storage_directory"]["label"],
        "Plugin data directory"
    );
    assert!(preferences["storage_directory"]["default"].is_null());

    let package = text("scripts/package.sh");
    assert!(package.contains("pom-plugin-${platform}.json"));
    assert!(
        package.contains("($contract[0]) + {"),
        "the public manifest retains schema: 1"
    );
    assert!(!package.contains("del(.schema)"));
    assert!(package.contains(".preferences"));
    // A stable release goes to GitHub and to the license server, POM's
    // fallback when GitHub refuses or rate-limits a download.
    assert!(root().join("scripts/publish-to-license-server.sh").exists());
    let workflow = text(".github/workflows/publish-release.yml");
    assert!(workflow.contains("license-server") && workflow.contains("POM_RELEASE_TOKEN"));
    assert!(workflow.contains("if: ${{ !inputs.prerelease }}"));
}

#[test]
fn generic_build_and_project_files_are_present() {
    let cargo = text("Cargo.toml");
    assert!(cargo.contains("name = \"pom-plugin-base\""));
    assert!(cargo.contains("crate-type = [\"cdylib\"]"));
    let ui_package = json("ui/package.json");
    assert_eq!(ui_package["scripts"]["build"], "node build.mjs");
    let ui_build = text("ui/build.mjs");
    assert!(ui_build.contains("src/screens/index.tsx"));
    assert!(ui_build.contains("dist/plugin.css"));
    assert!(root().join("README.md").is_file());
    assert!(root().join("AGENTS.md").is_file());
    for path in [
        "build.rs",
        "scripts/build.sh",
        "scripts/build-ui.sh",
        "scripts/ci-plan.sh",
        "scripts/package.sh",
        "release/manifest.json",
        ".github/workflows/publish-release.yml",
    ] {
        assert!(root().join(path).is_file(), "missing {path}");
    }
}

#[test]
fn repository_text_avoids_unrelated_product_terms() {
    let forbidden: &[&[u8]] = &[
        &[101, 110, 116, 101, 114, 112, 114, 105, 115, 101],
        &[108, 111, 103, 115],
        &[115, 116, 97, 116, 105, 115, 116, 105, 99, 115],
        &[114, 101, 113, 117, 101, 115, 116, 115],
        &[97, 117, 100, 105, 116],
        &[100, 97, 116, 97, 98, 97, 115, 101],
        &[109, 101, 116, 114, 105, 99, 115],
        &[115, 111, 108, 100],
        &[101, 115, 116, 97, 116, 195, 173, 115, 116, 105, 99, 97, 115],
        &[
            114, 101, 113, 117, 105, 115, 105, 195, 167, 195, 181, 101, 115,
        ],
        &[97, 117, 100, 105, 116, 111, 114, 105, 97],
        &[
            98, 97, 110, 99, 111, 32, 100, 101, 32, 100, 97, 100, 111, 115,
        ],
        &[109, 195, 169, 116, 114, 105, 99, 97, 115],
        &[118, 101, 110, 100, 105, 100, 111, 115],
    ];
    let mut pending = vec![root()];
    while let Some(path) = pending.pop() {
        let name = path
            .file_name()
            .and_then(|part| part.to_str())
            .unwrap_or_default();
        // `.claude/skills/creating-pom-plugins` is a verbatim copy of the POM
        // repository's plugin skill, which covers every kind of plugin.
        if [".git", "target", "node_modules", "dist", "dist-release"].contains(&name)
            || path.ends_with(".claude/skills/creating-pom-plugins")
        {
            continue;
        }
        if path.is_dir() {
            pending.extend(
                fs::read_dir(&path)
                    .unwrap()
                    .flatten()
                    .map(|entry| entry.path()),
            );
            continue;
        }
        let Ok(contents) = fs::read(&path) else {
            continue;
        };
        let mut haystack = path.to_string_lossy().as_bytes().to_vec();
        haystack.push(b'\n');
        haystack.extend(contents);
        for pattern in forbidden {
            assert!(
                !contains_term(&haystack, pattern),
                "unrelated product term found in {}",
                path.display()
            );
        }
    }
}

#[test]
fn projects_screen_lists_projects_from_the_shared_workspace() {
    let source = text("ui/src/screens/Projects.tsx");
    assert!(source.contains("export function Projects"));
    assert!(source.contains("usePluginI18n"));
    assert!(source.contains("getPluginApi"));
    assert!(source.contains("normalizeWorkspace"));
    assert!(source.contains("projects.map"));
    for key in [
        "projects.loading",
        "projects.unavailable",
        "projects.empty",
        "projects.body",
    ] {
        assert!(source.contains(&format!("t(\"{key}\")")), "missing {key}");
    }
    assert_eq!(source.matches("<main").count(), 1);
    assert_eq!(source.matches("<h1").count(), 1);
    assert!(source.contains("<ul") && source.contains("<li"));

    let runtime = text("ui/src/host/runtime.ts");
    assert!(runtime.contains("export function getPluginApi"));
    assert!(runtime.contains("/api/ui/plugins/"));
    assert!(runtime.contains("/proxy/"));

    let css = text("ui/src/plugin.css");
    assert!(css.contains(".pb-project-list"));
    assert!(css.contains("html[data-theme=\"light\"] .pb-page"));
}
