//! Embeds the UI bundle, the menu icon, the i18n catalogs and the Markdown
//! documentation into the library, so the manifest and the backend always
//! ship in the same verified binary.

use std::{
    env, fs,
    path::{Path, PathBuf},
};

fn content_type(name: &str) -> &'static str {
    match Path::new(name)
        .extension()
        .and_then(|extension| extension.to_str())
    {
        Some("js") => "text/javascript",
        Some("css") => "text/css",
        Some("md") => "text/markdown",
        Some("png") => "image/png",
        Some("json") => "application/json",
        _ => "application/octet-stream",
    }
}

/// Where the built UI is: the POM's local rebuild builds it in
/// `POM_PLUGIN_OUT_DIR` (`scripts/dev-ui.sh`, the source is read-only there),
/// any other build in the source's `ui/dist` (`scripts/build-ui.sh`).
fn ui_dist(root: &Path) -> PathBuf {
    println!("cargo:rerun-if-env-changed=POM_PLUGIN_OUT_DIR");
    env::var("POM_PLUGIN_OUT_DIR")
        .ok()
        .filter(|value| !value.is_empty())
        .map(|out| Path::new(&out).join("ui-build/ui/dist"))
        .filter(|dist| dist.join("screens.js").is_file())
        .unwrap_or_else(|| root.join("ui/dist"))
}

fn main() {
    let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("manifest directory"));
    let dist = ui_dist(&root);
    println!("cargo:rerun-if-env-changed=POM_UI_REQUIRE_DIST");
    if env::var("POM_UI_REQUIRE_DIST").as_deref() == Ok("1") {
        assert!(
            dist.join("screens.js").is_file(),
            "POM_UI_REQUIRE_DIST=1 but {} has no screens.js: run scripts/build-ui.sh",
            dist.display()
        );
    }

    let mut entries = Vec::new();
    for (directory, prefix, extensions) in [
        (dist.clone(), "ui", &["js", "css"][..]),
        (root.join("ui"), "ui", &["png"][..]),
        (dist.join("i18n"), "i18n", &["json"][..]),
        (root.join("docs"), "docs", &["md"][..]),
    ] {
        println!("cargo:rerun-if-changed={}", directory.display());
        let Ok(files) = fs::read_dir(&directory) else {
            continue;
        };
        for file in files.flatten() {
            let name = file.file_name().to_string_lossy().into_owned();
            let extension = Path::new(&name)
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or_default();
            if extensions.contains(&extension) {
                entries.push((
                    format!("{prefix}/{name}"),
                    file.path().to_string_lossy().into_owned(),
                ));
            }
        }
    }

    entries.sort();
    let mut generated = String::from("pub static UI_ASSETS: &[(&str, &str, &[u8])] = &[\n");
    for (name, path) in &entries {
        generated.push_str(&format!(
            "    ({name:?}, {:?}, include_bytes!({path:?})),\n",
            content_type(name)
        ));
    }
    generated.push_str("];\n");
    let output = Path::new(&env::var("OUT_DIR").expect("output directory")).join("ui_assets.rs");
    fs::write(output, generated).expect("write embedded asset index");
    println!("cargo:rerun-if-changed=ui/manifest.json");
    println!("cargo:rerun-if-changed=i18n");
}
