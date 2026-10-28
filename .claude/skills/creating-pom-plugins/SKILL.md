---
name: creating-pom-plugins
description: Use when creating a new POM plugin (a pom-plugin-* repository), adding a capability to an existing plugin (screens, menu, RPC, peer calls, cluster services, inference engine, loopback web app, preferences, release workflow), or checking whether a plugin follows the POM plugin contract (C ABI v1, pom-plugin-ui/v1, pom-plugin-events/v1).
---

# Creating POM plugins

## Overview

A POM plugin is its own `pom-plugin-<name>` repository: a Rust `cdylib`
loaded by `pom-plugin-host` (never by `pom`), with its screens, menu, i18n and
docs embedded in the same verified binary. Every new plugin starts from this
skill's `template/`, which already wires the whole common contract and its
contract tests; optional capabilities are added from `recipes.md`.

## When to use

- New plugin: always scaffold, never copy another plugin by hand.
- New capability in a plugin: find it in `recipes.md`, copy from the named
  reference repository, add tests.
- Review: run the checklist below against the plugin.
- Not for `pom-plugin-llama.cpp`: it is an in-process crate of the POM, not a
  plugin.

## Create a plugin

```bash
bash .claude/skills/creating-pom-plugins/scripts/new-plugin.sh \
  ../pom-plugin-<name> --code <plugin_code> --name "<Display Name>"
cd ../pom-plugin-<name> && git init -b main
scripts/build-ui.sh
POM_UI_REQUIRE_DIST=1 cargo test --locked
cargo fmt --check && cargo clippy --locked --all-targets -- -D warnings
tests/test_ci_plan.sh
scripts/package.sh --platform linux-x86_64 --version 0.1.0
```

`plugin_code` matches `^[a-z][a-z0-9_]{1,63}$`; the entry symbol becomes
`pom_<plugin_code>_plugin_v1`. The repository must sit next to the other
`pom-plugin-*` checkouts so the dev cluster mounts it. Then replace the demo
screen and `rpc.*` operations with the plugin's own, keeping the structure.

## What the template gives

| Area | Files |
|---|---|
| C ABI v1, panic-safe callbacks | `src/abi.rs` |
| Dispatch: `host.configure`, `host.event`, `host.services`, `ui.manifest`, `ui.asset`, `rpc.*` with profile checks, `ingest` | `src/lib.rs` |
| `host.configure` parsing (gateway, workspace root, loopback `host_api`, preferences), secrets hidden | `src/context.rs` |
| `pom-plugin-ui/v1` manifest: icon, description, screenshot, menu, route, screen, i18n, documentation, `development.rebuild` | `ui/manifest.json` |
| Release contract: ABI, `feature_set`, preferences schema | `release/manifest.json` |
| Host SDK helpers (design kit, i18n, RPC, upstream, events, notify/confirm), `ready()` signal | `ui/src/host/` |
| Bundle against the host's React, `pb-` CSS and i18n namespacing | `ui/build.mjs` |
| Build, package, per-platform GitHub manifest, license server upload, CI plan, dev rebuild | `scripts/` |
| CI and manual release (GitHub; stable also to the license server) | `.github/workflows/` |
| Contract tests mirroring the node's validation | `tests/contract.rs` |
| Agent memory and README | `AGENTS.md` (`CLAUDE.md` symlink), `README.md` |

## Rules every plugin keeps

- `query` returns at once (3 s IPC timeout shared by every call); slow work
  runs on a thread the plugin owns.
- Children never inherit the host's stdin/stdout; they die with the host.
- A refusal the screen must show is an `Ok` body `{"error": ...}`: an `Err`
  reaches the browser only as a generic 502.
- Authorization lives in the plugin: `rpc.*` checks `profile`, `peer.*`
  checks `caller.role`/`caller.head`. Menu `roles` are not enforced.
- Secrets (`api_key`, `host_api.token`, upstream tokens, third-party keys)
  never appear in replies, `host.services`, logs or screens.
- Fixed files under `<dir of POM_PLUGIN_DB>/<plugin_code>`; user data under
  `workspace_root/.<plugin_code>`.
- Only `ui/src/host/*` reads `__POM_HOST__`; CSS is scoped; i18n keys are
  bare in source and identical across catalogs.
- Never vendor third-party sources or releases; pin and verify downloads.

## References

- `reference.md` - the full contract: ABI, every operation, both manifests,
  SDK surface, `host_api`, events, distribution, which repository shows what.
- `recipes.md` - optional modules: loopback upstream, supervisor and children,
  verified runtime download, `host.services`, `peer.*`, `host_api`, gateway,
  inference engine, slots, on-demand, dev rebuild steps, licensing, storage.
- POM repository: `docs/subsystems/plugins-ui.md`,
  `docs/subsystems/enterprise-licensing.md`.

## Common mistakes

| Mistake | Fix |
|---|---|
| Copying `pom-plugin-base` and renaming by hand | Scaffold; base carries tutorial text and a product-term test |
| No `Cargo.lock` committed | The POM dev rebuild runs `--locked`; keep the lock |
| Menu target without a route, or asset not in `assets` | The node ignores the whole manifest; `tests/contract.rs` catches it |
| Menu `icon` not in the host map | Falls back to `blocks`; list in `reference.md` |
| Blocking download inside `query` | Supervisor thread, `installing` status |
| Screen `ready()` on first render | Signal when the first real content (or error) settled |
| New plugin missing on the other machine | Add it to `pom-cluster-config/scripts/clone-repos.sh` |
