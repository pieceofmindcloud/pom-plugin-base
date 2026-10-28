#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
command -v node >/dev/null 2>&1 || { echo 'build-ui: Node.js is required' >&2; exit 2; }
command -v npm >/dev/null 2>&1 || { echo 'build-ui: npm is required' >&2; exit 2; }
(
  cd "$root/ui"
  npm ci --no-fund >&2
  npm run build >&2
)
[[ -s "$root/ui/dist/screens.js" && -s "$root/ui/dist/plugin.css" \
  && -s "$root/ui/dist/i18n/en.json" && -s "$root/ui/dist/i18n/pt-BR.json" ]] \
  || { echo 'build-ui: generated files are incomplete' >&2; exit 2; }
