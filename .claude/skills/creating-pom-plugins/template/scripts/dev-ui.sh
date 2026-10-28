#!/usr/bin/env bash
# Rebuild step of the POM's local development rebuild (`development.rebuild`
# in ui/manifest.json). The mounted source is read-only there, so the UI is
# built in a copy under $POM_PLUGIN_OUT_DIR; build.rs embeds that ui/dist.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
out="${POM_PLUGIN_OUT_DIR:?dev-ui: POM_PLUGIN_OUT_DIR is not set}"
command -v npm >/dev/null 2>&1 || { echo 'dev-ui: npm is required' >&2; exit 2; }

work="$out/ui-build"
mkdir -p "$work/ui"
# The sources, without the dependencies and the previous output.
(cd "$root/ui" && tar --exclude=./node_modules --exclude=./dist -cf - .) | tar -xf - -C "$work/ui"
rm -rf "$work/i18n" && cp -R "$root/i18n" "$work/i18n"
(
  cd "$work/ui"
  npm ci --no-fund --no-audit >&2
  npm run build >&2
)
[[ -s "$work/ui/dist/screens.js" && -s "$work/ui/dist/plugin.css" ]] \
  || { echo 'dev-ui: generated files are incomplete' >&2; exit 2; }
