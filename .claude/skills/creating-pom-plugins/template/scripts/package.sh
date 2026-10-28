#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
platform=""
version=""
output="${root}/dist-release"

usage() {
  printf '%s\n' \
    'Usage: scripts/package.sh --platform <linux-x86_64|macos-aarch64|windows-x86_64> --version <semver> [options]' \
    '  --output <dir>   package directory (default: dist-release)'
}

die() {
  printf 'package: %s\n' "$1" >&2
  exit 2
}

while (($# > 0)); do
  case "$1" in
    --platform) (($# >= 2)) || die '--platform requires a value'; platform="$2"; shift 2 ;;
    --version) (($# >= 2)) || die '--version requires a value'; version="$2"; shift 2 ;;
    --output) (($# >= 2)) || die '--output requires a value'; output="$2"; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    *) die "unknown option: $1" ;;
  esac
done

[[ -n "$platform" ]] || die '--platform is required'
[[ -n "$version" ]] || die '--version is required'
command -v jq >/dev/null 2>&1 || die 'jq is required'

plugin_code="$(jq -er '.plugin_code | strings' "$root/ui/manifest.json")" || die 'manifest has no plugin code'
release_code="$(jq -er '.plugin_code | strings' "$root/release/manifest.json")" || die 'release manifest has no plugin code'
[[ "$plugin_code" =~ ^[a-z][a-z0-9_]{1,63}$ ]] || die 'manifest plugin code is invalid'
[[ "$plugin_code" == "$release_code" ]] || die 'release manifest plugin code does not match the UI manifest'
[[ "$(jq -er '.schema | numbers' "$root/release/manifest.json")" == 1 ]] || die 'release manifest schema is unsupported'

case "$platform" in
  linux-x86_64) os="linux"; arch="x86_64" ;;
  macos-aarch64) os="macos"; arch="aarch64" ;;
  windows-x86_64) os="windows"; arch="x86_64" ;;
  *) die "unsupported platform: $platform" ;;
esac

build_output="$("$root/scripts/build.sh" --platform "$platform" --output "$output")"
artifact="$(printf '%s\n' "$build_output" | sed -n 's/^artifact=//p')"
sha256="$(printf '%s\n' "$build_output" | sed -n 's/^sha256=//p')"
size="$(printf '%s\n' "$build_output" | sed -n 's/^size=//p')"
target="$(printf '%s\n' "$build_output" | sed -n 's/^target=//p')"

metadata="${output}/__REPO__-${platform}.metadata.json"
public_manifest="${output}/pom-plugin-${platform}.json"
preferences="$(jq -c '.preferences' "$root/release/manifest.json")" || die 'release manifest preferences are invalid'
feature_set="$(jq -c '.feature_set' "$root/release/manifest.json")" || die 'release manifest feature set is invalid'
cat > "$metadata" <<JSON
{
  "plugin_code": "${plugin_code}",
  "version": "${version}",
  "platform": "${platform}",
  "target": "${target}",
  "plugin_abi": 1,
  "sha256": "${sha256}",
  "size": ${size},
  "artifact": "$(basename "$artifact")",
  "feature_set": ${feature_set},
  "preferences": ${preferences}
}
JSON
jq -n \
  --slurpfile contract "$root/release/manifest.json" \
  --arg release_id "v${version}" \
  --arg version "$version" \
  --arg os "$os" \
  --arg arch "$arch" \
  --arg asset "$(basename "$artifact")" \
  --arg sha256 "$sha256" \
  --argjson size "$size" \
  '($contract[0]) + {
    release_id: $release_id,
    version: $version,
    os: $os,
    arch: $arch,
    asset: $asset,
    sha256: $sha256,
    size: $size
  }' > "$public_manifest" || die 'could not generate the GitHub release manifest'

printf 'artifact=%s\nmetadata=%s\npublic_manifest=%s\nsha256=%s\nsize=%s\nversion=%s\nplatform=%s\n' \
  "$artifact" "$metadata" "$public_manifest" "$sha256" "$size" "$version" "$platform"
