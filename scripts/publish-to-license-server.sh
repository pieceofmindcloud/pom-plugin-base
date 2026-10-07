#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
version=""
platform=""
artifact=""
metadata=""
min_host_version="0.1.0"
channel="stable"
reason=""
dry_run=false

usage() {
  printf '%s\n' \
    'Usage: POM_RELEASE_TOKEN=... ./scripts/publish-to-license-server.sh --version v0.1.0 --platform linux-x86_64 --artifact <file> --metadata <file> [options]' \
    '  --version <tag>       release tag (required)' \
    '  --platform <name>     linux-x86_64|macos-aarch64|windows-x86_64' \
    '  --artifact <file>     packaged plugin library (required)' \
    '  --metadata <file>     package metadata JSON (required)' \
    '  --min-host <version>  minimum host version (default: 0.1.0)' \
    '  --channel <name>      release channel (default: stable)' \
    '  --reason <text>       publication reason' \
    '  --dry-run             verify package without uploading'
}

die() {
  printf 'publish-to-license-server: %s\n' "$1" >&2
  exit 2
}

while (($# > 0)); do
  case "$1" in
    --version) (($# >= 2)) || die '--version requires a value'; version="$2"; shift 2 ;;
    --platform) (($# >= 2)) || die '--platform requires a value'; platform="$2"; shift 2 ;;
    --artifact) (($# >= 2)) || die '--artifact requires a value'; artifact="$2"; shift 2 ;;
    --metadata) (($# >= 2)) || die '--metadata requires a value'; metadata="$2"; shift 2 ;;
    --min-host) (($# >= 2)) || die '--min-host requires a value'; min_host_version="$2"; shift 2 ;;
    --channel) (($# >= 2)) || die '--channel requires a value'; channel="$2"; shift 2 ;;
    --reason) (($# >= 2)) || die '--reason requires a value'; reason="$2"; shift 2 ;;
    --dry-run) dry_run=true; shift ;;
    -h|--help) usage; exit 0 ;;
    *) die "unknown option: $1" ;;
  esac
done

[[ "$version" =~ ^v[0-9]+\.[0-9]+\.[0-9]+([-+][0-9A-Za-z.-]+)?$ ]] || die '--version must use vX.Y.Z format'
[[ -n "$platform" ]] || die '--platform is required'
[[ -f "$artifact" ]] || die "artifact not found: $artifact"
[[ -f "$metadata" ]] || die "metadata not found: $metadata"
command -v jq >/dev/null 2>&1 || die 'jq is required'

case "$platform" in
  linux-x86_64) os="linux"; arch="x86_64" ;;
  macos-aarch64) os="macos"; arch="aarch64" ;;
  windows-x86_64) os="windows"; arch="x86_64" ;;
  *) die "unsupported platform: $platform" ;;
esac

expected_version="${version#v}"
metadata_code="$(jq -er '.plugin_code | strings' "$metadata")" || die 'metadata has no plugin code'
manifest_code="$(jq -er '.plugin_code | strings' "$root/ui/manifest.json")" || die 'manifest has no plugin code'
metadata_version="$(jq -er '.version | strings' "$metadata")" || die 'metadata has no version'
metadata_platform="$(jq -er '.platform | strings' "$metadata")" || die 'metadata has no platform'
metadata_abi="$(jq -er '.plugin_abi | numbers' "$metadata")" || die 'metadata has no ABI version'
expected_sha256="$(jq -er '.sha256 | strings' "$metadata")" || die 'metadata has no SHA-256'
expected_size="$(jq -er '.size | numbers' "$metadata")" || die 'metadata has no size'
# The metadata's feature set, else the release manifest's; the upload protocol
# needs at least one identifier.
feature_set="$(jq -er '.feature_set | select(type == "array" and length > 0) | join(",")' "$metadata" 2>/dev/null \
  || jq -er '.feature_set | select(type == "array" and length > 0) | join(",")' "$root/release/manifest.json" 2>/dev/null \
  || true)"
[[ "$manifest_code" =~ ^[a-z][a-z0-9_]{1,63}$ ]] || die 'manifest plugin code is invalid'
[[ "$metadata_code" == "$manifest_code" ]] || die 'metadata plugin code does not match the manifest'
reason="${reason:-release ${manifest_code} plugin}"
feature_set="${feature_set:-${manifest_code}.core}"
[[ "$metadata_version" == "$expected_version" ]] || die 'metadata version does not match the release tag'
[[ "$metadata_platform" == "$platform" ]] || die 'metadata platform does not match the selected platform'
[[ "$metadata_abi" == 1 ]] || die 'metadata ABI version is not supported'

declared_artifact="$(jq -r '.artifact // empty' "$metadata")"
if [[ -n "$declared_artifact" && "$declared_artifact" != "$(basename "$artifact")" ]]; then
  die 'metadata artifact name does not match the package file'
fi

if command -v shasum >/dev/null 2>&1; then
  actual_sha256="$(shasum -a 256 "$artifact" | awk '{print $1}')"
elif command -v sha256sum >/dev/null 2>&1; then
  actual_sha256="$(sha256sum "$artifact" | awk '{print $1}')"
else
  die 'shasum or sha256sum is required'
fi
[[ "$actual_sha256" == "$expected_sha256" ]] || die "sha256 mismatch: metadata says ${expected_sha256}, package hashes to ${actual_sha256}"
actual_size="$(wc -c < "$artifact" | tr -d '[:space:]')"
[[ "$actual_size" == "$expected_size" ]] || die "size mismatch: metadata says ${expected_size}, package is ${actual_size} bytes"

release_id="${manifest_code}-${expected_version}-${os}-${arch}"
if [[ "$dry_run" == true ]]; then
  printf 'release_id=%s\nstatus=dry-run\nsha256=%s\nsize=%s\nfeature_set=%s\nartifact=%s\n' \
    "$release_id" "$actual_sha256" "$actual_size" "$feature_set" "$artifact"
  exit 0
fi

command -v curl >/dev/null 2>&1 || die 'curl is required'
[[ -n "${POM_RELEASE_TOKEN:-}" ]] || die 'POM_RELEASE_TOKEN is required'
POM_RELEASE_API="${POM_RELEASE_API:-https://admin-license.pieceofmind.cloud}"
upload_url="${POM_RELEASE_API%/}"
[[ "$upload_url" == */internal/releases/upload ]] || upload_url="${upload_url}/internal/releases/upload"

mkdir -p "$root/target"
response_file="$(mktemp "$root/target/license-publish-response.XXXXXX")"
trap 'rm -f "$response_file"' EXIT

response_detail() {
  head -c 2048 "$response_file" | tr '\r\n' '  ' | sed 's/[[:space:]][[:space:]]*/ /g'
}

if http_status="$(curl --silent --show-error --output "$response_file" --write-out '%{http_code}' \
  -H "Authorization: Bearer ${POM_RELEASE_TOKEN}" \
  -F "plugin=${manifest_code}" \
  -F "version=${expected_version}" \
  -F "os=${os}" \
  -F "arch=${arch}" \
  -F 'plugin_abi=1' \
  -F "min_host_version=${min_host_version}" \
  -F "channel=${channel}" \
  -F "feature_set=${feature_set}" \
  -F "reason=${reason}" \
  -F "artifact=@${artifact}" \
  "$upload_url")"; then
  :
else
  curl_status=$?
  detail="$(response_detail)"
  [[ -n "$detail" ]] && die "upload request failed (curl exit ${curl_status}): ${detail}"
  die "upload request failed (curl exit ${curl_status})"
fi

if [[ ! "$http_status" =~ ^2[0-9][0-9]$ ]]; then
  detail="$(response_detail)"
  [[ -n "$detail" ]] && die "server returned HTTP ${http_status}: ${detail}"
  die "server returned HTTP ${http_status} without an error response body"
fi

printf 'release_id=%s\nstatus=published\nsha256=%s\nsize=%s\n' \
  "$release_id" "$actual_sha256" "$actual_size"
