# POM plugin contract - reference

Everything a POM plugin can do, gathered from the POM (`crates/node`,
`crates/plugin-host`, `apps/frontend`) and from the `pom-plugin-*`
repositories. The authoritative long form is in this repository:
`docs/subsystems/plugins-ui.md` (UI, events, RPC, services, proxy, host API)
and `docs/subsystems/enterprise-licensing.md` (licensing, GitHub install,
release, host process). If this file and the code disagree, the code wins.

## 1. Process and ABI

- The plugin is a Rust `cdylib`. `pom` never loads it: `pom-plugin-host
  <absolute path> <plugin_code>` does, one process per active plugin, and
  talks to the node with length-prefixed JSON frames (4-byte big-endian size,
  at most 16 MiB) on stdin/stdout. IPC methods: `ping`, `ready`, `ingest`,
  `query`, `shutdown`.
- Entry symbol: `pom_<plugin_code>_plugin_v1() -> *const PluginApiV1`
  (`repr(C)`: `abi_version = 1`, NUL-terminated `plugin_version`,
  `capabilities = 0`, and the `create`, `ingest`, `query`, `free_buffer`,
  `shutdown` callbacks, all required). Types live in
  `crates/enterprise-protocol/src/abi.rs`.
- `query(handle, json) -> buffer`: the request is `{"operation": "<op>", ...}`
  and the reply any JSON value. An empty or null buffer is a failure: the
  host answers `ok: false` with a parse error and the node turns it into a
  generic 502 / ignored reply. **The reason is lost**, so a refusal the
  screen must show is an `Ok` body with an `error` field.
- Timeouts: the node waits 3 s per IPC call (`IPC_TIMEOUT`) and 30 s for the
  host to become ready. `query` must return at once; slow work belongs to a
  thread the plugin owns (a "manager" or "supervisor" thread).
- Panics must never cross FFI: wrap every callback in `catch_unwind`.
- Environment: `POM_PLUGIN_DB` is the per-plugin database path the node
  passes (`<pom data dir>/<plugin_code>.db`, `logs.db` for `logs`). By
  convention its **directory** joined with `<plugin_code>` is the plugin's
  fixed-file home (runtimes, logs, binaries).
- On Windows the host runs with `CREATE_NO_WINDOW` inside a kill-on-close Job
  Object; on Linux a child should set `PR_SET_PDEATHSIG` so it dies with the
  host. A child must **never** inherit the host's stdin/stdout (they carry
  the IPC): `Stdio::null()` for stdin, a log file for stdout/stderr.

## 2. Operations the node sends to `query`

| Operation | Request fields | Expected reply | Notes |
|---|---|---|---|
| `host.configure` | `gateway{openai_base_url, api_key}`, `workspace_root`, `host_api{base_url, token}`, `preferences{features, storage_directory}` | `{"status":"ok"}` | On every host start and whenever any part changes. Errors are ignored. |
| `host.event` | `event` (envelope, section 6) | `{"status":"ok"}` | `preferences.changed` to the plugin itself; `deployment.started`, `model.serving`, `model.stopped` to every running plugin. |
| `host.services` | - | `{"services":[{id, kind, name, status, version?, detail?, architectures?, settings?, stages?, pom_settings?}]}` | Polled at most every 5 s; at most 8 entries; published in `GET /api/ui/node.plugin_services` and drawn next to the machine. `status`: `ready`, `starting`, `installing`, `waiting`, `error`. |
| `ui.manifest` | - | the `pom-plugin-ui/v1` manifest | Saved in `pom.db`, so an `on_demand` menu survives a stopped process. |
| `ui.asset` | `path` | `{"content_type", "base64"}` | Only paths listed in `assets`. Closed content-type list. |
| `ui.upstream` | - | `{"status":"ready","port","token"}` or `{"status":"starting"}` / `{"status":"error","error"}` | Mounts the plugin's loopback HTTP server at `/api/ui/plugins/<code>/proxy/*`. Token is 16-512 ASCII bytes. |
| `rpc.<op>` | `params`, `profile` (`admin`/`worker`/`visitor`), `node_id` | any JSON, returned as is | From `POST /api/ui/plugins/<code>/rpc/<op>`; body at most 256 KiB, same origin only, plugin must be active. The plugin authorizes by `profile`. |
| `peer.<op>` | `params`, `caller{node_id, head, model, role, local?}` | any JSON | From another node's signed call (`POST /api/internal/plugins/<code>/peer/<op>`) or this plugin's own `host_api` `/peer`. `role` is proven by the trust chain: `principal`, `admin`, `visitor`, `member`. `head` = the caller serves layer 0. |
| `engine.load` / `engine.unload` / `engine.status` / `engine.upstream` / `engine.link` | see `docs/subsystems/inference.md` 7.1 | see there | Only for a service of kind `inference`. Local node only. Reference: `pom-plugin-strata`. |

`ingest(handle, json) -> i32` (0 = accepted) receives node events of the
plugin's own kind (`{"type": ...}`). Today the node only ingests into the
`logs` plugin (request audit `begin`/`complete`, cluster logs) and retries
on failure, so handling must be idempotent. Other plugins accept and ignore.

## 3. Manifests

Two files, never merged:

**`release/manifest.json`** (release contract, `schema: 1`): `plugin_code`,
`plugin_abi: 1`, `feature_set` (`["<code>.core", ...]`), optional
`preferences`:

```json
{"features": [{"key": "x", "label": "X", "default": false, "description": "..."}],
 "storage_directory": {"label": "Data directory", "default": null}}
```

Preference values live in `pom.db` per plugin, reach the plugin only through
`host.configure` / `preferences.changed`, and never go to the license server.
The Plugins card shows a gear while the plugin is active and has a schema.

**`ui/manifest.json`** (`pom-plugin-ui/v1`):

| Field | Rule |
|---|---|
| `schema`, `plugin_code` | `pom-plugin-ui/v1`; code matches the installed plugin, `^[a-z][a-z0-9_]{1,63}$` |
| `lifecycle` | `resident` (default) or `on_demand` (process only while open; menu X badge; `/open`, `/close` routes) |
| `icon_image` | `.png` asset for the menu; `menu[].icon` is the fallback |
| `description` | per language, `en` required, at most 2000 bytes each |
| `screenshots` | up to 8, repository-relative (served from `raw.githubusercontent.com/<owner>/<repo>/HEAD/`) or `https://`; **not** assets |
| `menu[]` | `id` (unique, children included), `label{en,...}`, `icon`, `to`, optional `children[]`, `roles`/`profiles` (validated, not enforced) |
| `routes[]` | `path`, `screen`, `full_bleed`; every `menu.to` needs a route |
| `screens{}` | `module` (ES asset), `export` (default `default`), `styles[]` |
| `slots{}` | slot id -> screen key, at most 16. Offered today: `chat.telemetry` (reference: `pom-plugin-logs`) |
| `i18n{}` | language -> JSON asset, `en` required |
| `documentation` | `{guides: [...], reference: [...]}`, each `{id, title{en}, asset: "docs/<file>.md"}`; shown under `/admin-ui/documentation` |
| `development.rebuild[]` | up to 8 `{run: [script, args...], os?: [linux, macos, windows]}` run before `cargo build` in the POM dev rebuild |
| `assets[]` | closed list of relative paths, no `..`; everything above that the node serves must be here |

Routes and menu targets are namespaced under `/<plugin_code>/...` (a path
already starting with the code is not doubled). `/logs` and `/statistics` are
native extension points: a single active claimant takes the native URL.

Valid menu icons (`apps/frontend/src/components/nav.ts`, `PLUGIN_ICONS`):
`list-tree`, `chart-no-axes-column`, `blocks`, `gauge`, `terminal`,
`history`, `settings`, `network`, `boxes`, `shield-check`, `book-open`,
`folder-tree`, `send`, `messages-square`, `cpu`, `activity`. Anything else
falls back to `blocks`.

## 4. Browser SDK (`globalThis.__POM_HOST__`, `PLUGIN_HOST_VERSION = 7`)

`React`, `ReactDOM`, `jsxRuntime`, `reactQuery`, `ui` (the design kit:
`Panel`, `PanelHeader`, `PageHeader`, `FullBleedHeader`, `FullBleedToolbar`,
`Button`, `Input`, `Select`, `Field`, `Badge`, `Tabs` (v5), `Dot`, `Progress`,
`Meter`, `Empty`, `Skeleton`, `ErrorState`, `Mono`, `ScrollX`, `Footnote`,
`KeyValue`, `InfoTip`, `IconTip`, `AlertTooltip`), `format`,
`components{Modal, TelemetryLine, ExecutionTelemetry, Markdown}`,
`hooks{useI18n, useTheme, useCluster}`, `api.get`, `storageKey`,
`context()` (v2), `events{subscribe, last}` (v2), `protocol` (v2),
`plugin(code)` -> `{events, notifications{notify, confirm}, rpc, loader}` (v2,
`rpc` v4, `loader` v7), `ready()` (v6).

Rules the existing plugins follow:

- Only `ui/src/host/runtime.ts` and `ui/src/host/ready.ts` read
  `__POM_HOST__`; everything degrades on an older host.
- `ui/build.mjs` (esbuild) maps `react`, `react-dom`, `react/jsx-runtime` to
  the host object, defines `__POM_PLUGIN_CODE__`, renames the `pb-` CSS
  prefix to the plugin code and prefixes every i18n key with `<code>.`.
- CSS hangs off one root class, never `:root`/`body`; colours use the POM
  tokens (`--fg`, `--muted`, `--faint`, `--line`, `--surface`, `--raised`,
  `--iris`, `--aqua`, `--amber`, `--rose`, `--sky`, `--tab-*`).
- `useScreenReady(ready)` calls `ready()` once per mount, when the first real
  content settled (or failed), never on the first render. Without it the POM
  opens the screen after 2 s.
- Work before the screen is usable (installing a runtime, starting a process)
  goes behind POM's loader, never a loading screen of the plugin's own:
  `loader.show/progress/error/success/hide` and `onLoaderRetry` in
  `ui/src/host/runtime.ts` (v7). While the plugin holds a loader state the
  screen stays covered, `ready()` included; only `hide()` or the end of
  `success()` opens it. `progress` is 0..100, `title` <= 200 and `message` <=
  2048 characters; `error` shows "Try again" unless `retry: false`, which
  sends `loader.retry` to the plugin. Contract: POM
  `docs/subsystems/plugins-ui.md`, "POM's loader and installer".
- Same-origin URLs: assets `/api/ui/plugins/<code>/assets/<path>`, RPC
  `/api/ui/plugins/<code>/rpc/<op>`, upstream `/api/ui/plugins/<code>/proxy/<path>`.
  `api.get` only does GET; POST to the proxy with `fetch` and
  `credentials: "same-origin"`.

## 5. Calling back into the node (`host_api`)

From `host.configure.host_api`; send `x-pom-plugin-host-token: <token>` to
loopback only. The token names the plugin, new at every node start.

| Route | Answer |
|---|---|
| `GET /api/plugin-host/node` | `{node_id, hostname, public_key, did_key, role, principal_node_id}` |
| `GET /api/plugin-host/peers` | `{peers: [{node_id, hostname, principal, visitor}]}` |
| `POST /api/plugin-host/peer` | `{node_id, op, params}` -> `{reply}` from `peer.<op>` of the same plugin on that machine (`caller.local: true` for this node) |
| `POST /api/plugin-host/sign` | `{statement}` -> `{signature, public_key, did_key}`; only known identity statements naming this node's `did:key` |

Reference client: `pom-plugin-ghostly/src/node_api.rs`.

The OpenAI-compatible gateway (`gateway.openai_base_url`, Bearer
`gateway.api_key`) is the node's own `/v1` on loopback: chat, models and
`/v1/systemone` decisions (reference: `pom-plugin-jev`). The key never reaches
a browser: a loopback upstream adds it server-side.

## 6. Events (`pom-plugin-events/v1`)

Envelope: `{protocol, id, type, target ("*" or code), source: "pom", at, payload}`.

| Type | Payload |
|---|---|
| `locale.changed` / `theme.changed` | `{locale}` / `{theme}` (also once on mount) |
| `preferences.changed` | `{preferences}` (native channel, to this plugin) |
| `notification.received` | `{message_id, plugin_code, title, body, level, sender_node_id, sent_at}` |
| `notification.response` | `{request_id, kind, action, error?}` |
| `deployment.started` | `{kind, deploy_id, job_id, model, nodes, deployment_name?, reused?, version?}` |
| `model.serving` / `model.stopped` | `{model, deploy_ids, node_ids}` / `{model}` |
| `loader.retry` | `{}` ("Try again" on this plugin's loader error) |

Browser channels: SDK `plugin(code).events.subscribe(type|"*", h, {replay})`,
DOM `pom:plugin-event` on `window`, `postMessage` to
`iframe[data-pom-plugin="<code>"]`. Commands from the plugin:
`notification.notify {title, body?, level?, scope: local|network}` and
`notification.confirm {title, body?, acceptLabel?, cancelLabel?, tone?, display?}`
and the `loader.*` commands (section 4) through the SDK or `pom:plugin-request`.

## 7. Distribution

- GitHub: the POM reads `manifest.json` or `ui/manifest.json` at the default
  branch root; each release attaches `pom-plugin-<os>-<arch>.json`
  (`scripts/package.sh` output) and the binary it names; the API's SHA-256
  digest is required. Platforms: `linux-x86_64` (`.so`), `macos-aarch64`
  (`.dylib`), `windows-x86_64` (`.dll`).
- License server: every stable release is also uploaded
  (`scripts/publish-to-license-server.sh`, `POM_RELEASE_TOKEN`,
  `POM_RELEASE_API`); prereleases stay on GitHub. Free plugins are served
  there publicly as the GitHub fallback; licensed plugins need a license key
  whose `licensed_plugins` names the code.
- Local development: `POM_DEV_MODE=1`, sources mounted under
  `/opt/pom/dev-plugins` as `pom-plugin-*`, `cargo metadata --no-deps
  --locked` then `development.rebuild` steps then `cargo build --release
  --locked` with `POM_PLUGIN_OUT_DIR`, `POM_PLUGIN_PLATFORM`,
  `POM_PLUGIN_SOURCE_DIR`. The source is read-only: steps write only to
  `POM_PLUGIN_OUT_DIR`. A `Cargo.lock` is required.
- Cluster: `pom-cluster-config/scripts/lib.sh` mounts every sibling
  `pom-plugin-*` checkout read-only into the dev nodes, so a new repository
  next to the others appears in the rebuild list after `up.sh`. To have it
  cloned on the other machines too, it must be listed in
  `pom-cluster-config/scripts/clone-repos.sh` (authorized change, pushed to
  `main`).

## 8. Existing plugins as references

| Repository | Shows |
|---|---|
| `pom-plugin-base` | minimal ABI, every UI feature, events and notifications, workspace listing, loopback upstream |
| `pom-plugin-jev` | gateway use through a loopback upstream that hides the key, `on_demand` |
| `pom-plugin-qdrant` | supervised third-party binary (verified download), `host.services` kind `rag`, `peer.search`/`peer.connection`/`peer.collections`, admin-gated `rpc.*`, upstream that rewrites a dashboard |
| `pom-plugin-logs` | licensed plugin, `ingest`, SQLite in `POM_PLUGIN_DB`, profile/node scoped `rpc.*`, `peer.quota.*`, `chat.telemetry` slot |
| `pom-plugin-ghostly` | `host_api` client (`node`, `peers`, `peer`, `sign`), `peer.group_link`, `peer.whoami` |
| `pom-plugin-code-server`, `pom-plugin-harness`, `pom-plugin-deepseek-harness` | `on_demand` web apps behind `ui.upstream` with WebSockets, pinned runtime archives, `development.rebuild` with runtime steps, `rpc.runtime.*` |
| `pom-plugin-strata` | `inference` service: `engine.*`, `settings`, `stages`, `pom_settings` |
| `pom-plugin-llama.cpp` | **not** a cdylib plugin: an in-process Rust crate of the POM, pinned by `rev`. Do not copy its layout |
