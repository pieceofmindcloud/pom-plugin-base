# Optional modules - recipes

The template wires what every plugin needs. Add a module below only when the
plugin uses it, and add its tests next to the code. Each recipe names the
existing repository to copy from; read that file before adapting it.

## A. Loopback web server behind the POM proxy (`ui.upstream`)

Use when a screen needs more than RPC (an embedded web app, streaming, POST
bodies over 256 KiB, WebSockets). Copy from `pom-plugin-base/src/lib.rs`
(`WorkspaceServer`, std only) or `pom-plugin-jev/src/lib.rs` (also forwards to
the gateway).

- Bind `127.0.0.1:0`, never `0.0.0.0`; a random token per start.
- Refuse any request without `x-pom-plugin-token: <token>` (401).
- Build URLs under `x-forwarded-prefix` (the node mounts you at
  `/api/ui/plugins/<code>/proxy`); both `/proxy` and `/proxy/` reach `/`.
- Never trust or forward browser `authorization`, `cookie`, `api-key`; the
  node already strips them. Add your own secrets server-side.
- Answer `ui.upstream` with `{"status":"starting"}` until listening, then
  `{"status":"ready","port":<port>,"token":"<token>"}`.
- Stop the thread in `Drop` so `shutdown` releases the port.

## B. Supervisor thread and child processes

Use when the plugin runs a third-party program. Copy from
`pom-plugin-qdrant/src/supervisor.rs` (`Running`, spawn at the end of the
file) or `pom-plugin-code-server/src/supervisor.rs` (generations, retry).

```rust
let mut command = Command::new(&binary);
command
    .current_dir(base.join("run"))
    .env_clear()
    .envs(child_environment())          // only what the child needs
    .stdin(Stdio::null())               // the host's stdin/stdout are the IPC
    .stdout(Stdio::from(log))
    .stderr(Stdio::from(log_errors));
#[cfg(target_os = "linux")]
unsafe {
    use std::os::unix::process::CommandExt;
    command.pre_exec(|| {
        // The child dies with the plugin host.
        if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM) != 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(())
    });
}
```

- Start children from the long-lived manager thread, never from a short-lived
  one (`PR_SET_PDEATHSIG` fires when the *thread* that spawned it exits).
- `query` reads a snapshot (`Arc<RwLock<Status>>`) and returns at once.
- Phases map to `host.services` status: `installing`, `starting`, `ready`,
  `waiting`, `error`; expose download progress in `rpc.status`.
- Keep files under `plugin_directory()` (`<dir of POM_PLUGIN_DB>/<code>`).
  A data reset on upgrade is explicit (`DATA_EPOCH` in
  `pom-plugin-harness/src/supervisor.rs`).

## C. Pinned, verified runtime download

Never vendor a third-party release into the repository or the `cdylib`.
Pin URL, size and SHA-256 at build time and verify on the node:
`pom-plugin-code-server/scripts/fetch-runtime.sh --metadata-only` ->
`build.rs` (`CODE_SERVER_URL/SIZE/SHA256`) -> `supervisor.rs` (`download`,
`install_runtime`, `unpack_archive_file`). GitHub assets: refuse an asset
without the API's SHA-256 `digest` (`pom-plugin-qdrant/src/install.rs`).
Offer `rpc.runtime.status` and `rpc.runtime.retry` for the screen, and show
the install behind POM's loader: report `step` and `progress` in the status
and map them to `loader.progress` (byte fraction for the downloads, a creep
toward the step's end for the rest), `loader.error` with `onLoaderRetry` ->
`rpc.runtime.retry`. Reference: `pom-plugin-hermes` (`supervisor::progress_at`,
`ui/src/screens/Main.tsx`).

## D. A service the cluster sees (`host.services`)

```rust
"host.services" => Ok(json!({"services": [{
    "id": "rag", "kind": "rag", "name": "Qdrant",
    "status": snapshot.phase(),          // ready|starting|installing|waiting|error
    "version": snapshot.version, "detail": snapshot.error,
}]})),
```

Slugs are lowercase; at most 8 entries; no secret in `detail`. Kind `rag` is
picked up by the gateway's transparent retrieval when the plugin answers
`peer.search` and `peer.collections` (contract in `plugins-ui.md`,
"Transparent retrieval"). Kind `inference` requires recipe H.

## E. Peer operations (`peer.<op>`)

The node forwards only signed calls from live peers and tells who asks.
Decide in the plugin; never trust announced profiles:

```rust
"peer.connection" => {
    if request["caller"]["head"] != true {
        return Err("only a head of the network gets the connection".into());
    }
    Ok(connection_with_secret())
}
"peer.admin_thing" => match request["caller"]["role"].as_str() {
    Some("principal" | "admin") => Ok(do_it(&request["params"])),
    _ => Ok(json!({"error": "admins only"})),
},
```

Body at most 256 KiB. To call your own plugin on another node, use
`host_api` `POST /peer` (recipe F); `caller.local == true` means this node.

## F. Node API client (`host_api`)

Copy `pom-plugin-ghostly/src/node_api.rs` (`ureq`, no proxy, loopback check,
token hidden from `Debug`). Routes: `GET /node`, `GET /peers`,
`POST /peer {node_id, op, params}`, `POST /sign {statement}` (only the known
identity statements; anything else is 403). `context.rs` in the template
already keeps `host_api` only when it is loopback.

## G. Using the node's models (`gateway`)

`gateway.openai_base_url` + `Authorization: Bearer <gateway.api_key>`:
`/models`, `/chat/completions`, `/systemone` (typed decisions). Call it from
the plugin process (a loopback upstream, a supervisor or a child given the key
through its private environment), never from the browser. Reference:
`pom-plugin-jev/src/lib.rs` (`forward`), the harness launchers
(`runtime/launcher.mjs`, `privateEnv`).

## H. Inference engine (`engine.*`)

Only for a plugin that runs whole models or POM stages. Publish a service of
kind `inference` with `architectures`, optional `settings` (`placement:
"pom"` for the POM card), `stages: true` and `pom_settings`, and implement
`engine.load`, `engine.unload`, `engine.status`, `engine.upstream` (a loopback
`POST /pom/generate` streaming NDJSON) and, with stages, `engine.link`.
The contract is `docs/subsystems/inference.md` 7.1 and
`docs/subsystems/deployment.md` 1.6; the reference is `pom-plugin-strata`
(`src/engine.rs`, `src/proxy.rs`, `src/settings.rs`). Change both sides
together.

## I. Slot inside a native screen

```json
"screens": {"chat-telemetry": {"module": "ui/screens.js", "export": "chatTelemetry", "styles": ["ui/plugin.css"]}},
"slots": {"chat.telemetry": "chat-telemetry"}
```

Export a component taking the slot props (`ChatTelemetrySlotProps` in
`apps/frontend/src/lib/chatTelemetry.ts`). Do not call `ready()` from a slot.
Reference: `pom-plugin-logs/ui/src/telemetry/ChatTelemetry.tsx`.

## J. On-demand process

`"lifecycle": "on_demand"` in `ui/manifest.json`: no process at boot, the
menu stays from the saved manifest, opening the menu starts it, the X badge
stops it. Use for heavy apps an operator opens now and then (code-server,
harness, jev). Keep `icon_image` and i18n small: they are served from the
copy in `pom.db` while stopped. While the process starts, keep the screen
behind `loader.show()` from its first render and open it with `loader.hide()`
once the app answers (Hermes does it on its iframe's `load`).

## K. Extra steps in the POM development rebuild

When `cargo build` alone is not enough (a runtime archive, generated
metadata), add steps after `scripts/dev-ui.sh`:

```json
"development": {"rebuild": [
  {"run": ["scripts/dev-ui.sh"], "os": ["linux", "macos"]},
  {"run": ["scripts/dev-runtime.sh"], "os": ["linux", "macos"]}
]}
```

Steps get `POM_PLUGIN_OUT_DIR`, `POM_PLUGIN_PLATFORM`,
`POM_PLUGIN_SOURCE_DIR`, write only to `POM_PLUGIN_OUT_DIR`, and `build.rs`
reads their output from there (`pom-plugin-code-server/build.rs`,
`scripts/dev-release.sh`; `pom-plugin-harness/scripts/dev-runtime.sh`).

## L. Licensed plugin

Nothing changes in the library: the node installs a licensed release only
with a key whose `licensed_plugins` names the code, and the release workflow
publishes stable builds to the license server. The catalog entry
(`requires_license`, price, plans) is configured on the license server side
(`pom-website`). Data the node itself must refuse without entitlement is
enforced in the node, not in the plugin (`pom-plugin-logs` and
`/api/ui/requests`).

## M. Persistent store

Use `POM_PLUGIN_DB` for a SQLite file (`pom-plugin-logs`, `rusqlite` with
`bundled`); build long indexes on a background thread after `create`, since
the host waits only 30 s for `ready` and 3 s per call. User-visible data that
belongs to projects goes under `workspace_root/.<plugin_code>`.
