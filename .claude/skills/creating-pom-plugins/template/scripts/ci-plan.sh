#!/usr/bin/env bash
# Converts manual release inputs into one entry for each selected platform.
set -euo pipefail

entries=()
add() {
  local name="$1" selected="$2" version="$3" runner="$4" ext="$5" platform="$6" target="$7"
  [[ "$selected" == true ]] || return 0
  [[ "$version" =~ ^v[0-9]+\.[0-9]+\.[0-9]+([-+][0-9A-Za-z.-]+)?$ ]] \
    || { printf 'ci-plan: %s is selected but its version %q is not vX.Y.Z\n' "$name" "$version" >&2; exit 2; }
  entries+=("$(jq -cn --arg name "$name" --arg version "$version" --arg runner "$runner" \
    --arg ext "$ext" --arg platform "$platform" --arg target "$target" \
    '{name:$name, version:$version, runner:$runner, ext:$ext, platform:$platform, target:$target}')")
}

add linux   "${RELEASE_LINUX:-false}"   "${VERSION_LINUX:-}"   ubuntu-latest so    linux-x86_64  x86_64-unknown-linux-gnu
add macos   "${RELEASE_MACOS:-false}"   "${VERSION_MACOS:-}"   macos-latest  dylib macos-aarch64 aarch64-apple-darwin
add windows "${RELEASE_WINDOWS:-false}" "${VERSION_WINDOWS:-}" windows-2022  dll   windows-x86_64 x86_64-pc-windows-msvc

((${#entries[@]} > 0)) || { echo 'ci-plan: select at least one platform' >&2; exit 2; }
printf '%s\n' "${entries[@]}" | jq -cs '{include: .}'
