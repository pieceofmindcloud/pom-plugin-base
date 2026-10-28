# __NAME__

A POM plugin (`plugin_code: __CODE__`): a native `cdylib` loaded by
`pom-plugin-host`, with its own menu, screens and i18n embedded in the binary.

## Layout

| Path | What it is |
|---|---|
| `src/abi.rs` | C ABI v1 boundary (`create`, `ingest`, `query`, `free_buffer`, `shutdown`) |
| `src/lib.rs` | entry `pom___CODE___plugin_v1` and the operation dispatch |
| `src/context.rs` | `host.configure`: gateway, workspace root, host API, preferences |
| `ui/manifest.json` | `pom-plugin-ui/v1`: menu, routes, screens, i18n, docs, assets |
| `ui/src/host/` | the only code that reads `__POM_HOST__` (SDK helpers, `ready()`) |
| `ui/src/screens/` | one export per screen named in the manifest |
| `i18n/` | catalogs with bare keys; the build prefixes `__CODE__.` |
| `release/manifest.json` | release contract: ABI, `feature_set`, preferences schema |
| `scripts/` | UI build, native build, packaging, CI plan, license server upload |
| `tests/contract.rs` | static contract checks of both manifests |

## Build and verify

```bash
scripts/build-ui.sh
POM_UI_REQUIRE_DIST=1 cargo test --locked
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
tests/test_ci_plan.sh
scripts/package.sh --platform linux-x86_64 --version 0.1.0
```

`scripts/package.sh` writes `dist-release/__REPO__-<platform>.<ext>`, its
metadata and `pom-plugin-<platform>.json`, the manifest a GitHub release
attaches so the POM can install it from **Plugins > Add from GitHub**.

## Release

Run the **Publish plugin release** workflow with the platforms and a
`vX.Y.Z` tag per platform. A prerelease stays on GitHub; a stable release is
also uploaded to the license server (`POM_RELEASE_TOKEN`, `POM_RELEASE_API`
secrets), the POM's fallback when GitHub refuses or rate-limits.

## Local development in a POM node

With `POM_DEV_MODE=1` and this repository mounted next to the POM (under
`/opt/pom/dev-plugins`), **Plugins** offers a rebuild that runs the
`development.rebuild` steps of `ui/manifest.json`, then `cargo build`, and
installs the result without a license.
