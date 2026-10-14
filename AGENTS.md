# Project agent memory

- The host ABI entry point and embedded asset interface live in `src/lib.rs`.
- The host menu (with `children` submenus), route, description and screenshot contract is `ui/manifest.json`; screen exports are in `ui/src/screens/index.tsx`. Screenshots under `docs/screenshots/` are not assets: the POM loads them from GitHub.
- `ui/src/host/runtime.ts` implements the `pom-plugin-events/v1` helpers with a DOM-channel fallback; `ui/src/host/ready.ts` is the only other place that touches `__POM_HOST__`. `docs/guia-eventos.md` documents the protocol; the native side answers `host.event` in `src/lib.rs`.
- `ui/src/host/ready.ts` - the "finished rendering" signal the POM waits for (host version 6, `__POM_HOST__.ready()`). The POM mounts a screen hidden (`.pom-plugin-pending`) under its pieces animation and only opens it when this signal arrives or its short wait ends: a plugin without the signal keeps working. `useScreenReady(ready)` sends it once per mount, when the first content settles - the workspace answer in `Projects` - and not on the first render. The `tutorial` screen has no async content on mount, so it signals at once. An older host has no `ready`: the call is skipped.
- The contract test forbids some unrelated product words anywhere in the repository (`repository_text_avoids_unrelated_product_terms`); keep new copy clear of them.
- `scripts/build-ui.sh` generates ignored files under `ui/dist/` before native packaging.
- The manual release workflow in `.github/workflows/publish-release.yml` publishes to GitHub releases only; this is a public plugin and never talks to the license server. `release/manifest.json` is the release/capability contract and example local preference schema; `ui/manifest.json` is only the `pom-plugin-ui/v1` screen contract. `scripts/package.sh` emits per-platform GitHub manifests from the release contract.
- Use `cargo test` and `cargo fmt --check` for the Rust and static contract checks.

## Maintaining this file

Keep this file for knowledge useful to almost every future agent session in this project. Do not repeat what the codebase already shows; point to the authoritative file or command instead. Prefer rewriting or pruning existing entries over appending new ones. When updating this file, preserve this bar for all agents and keep entries concise.
