#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
fail() { printf 'FAIL: %s\n' "$1" >&2; exit 1; }
plan() { env -i PATH="$PATH" "$@" "$root/scripts/ci-plan.sh"; }

out="$(plan RELEASE_MACOS=true VERSION_MACOS=v0.2.0)"
[[ "$(jq -r '.include | length' <<<"$out")" == 1 ]] || fail 'single selection'
[[ "$(jq -r '.include[0].platform + " " + .include[0].version + " " + .include[0].target' <<<"$out")" == "macos-aarch64 v0.2.0 aarch64-apple-darwin" ]] || fail 'macOS entry'

out="$(plan RELEASE_LINUX=true VERSION_LINUX=v0.1.0 RELEASE_WINDOWS=true VERSION_WINDOWS=v0.3.1 RELEASE_MACOS=false VERSION_MACOS=invalid)"
[[ "$(jq -r '[.include[] | .name + "=" + .version] | join(",")' <<<"$out")" == "linux=v0.1.0,windows=v0.3.1" ]] || fail 'independent versions'

plan RELEASE_LINUX=false 2>/dev/null && fail 'empty selection must fail'
plan RELEASE_LINUX=true VERSION_LINUX= 2>/dev/null && fail 'empty version must fail'
plan RELEASE_LINUX=true VERSION_LINUX=0.1.0 2>/dev/null && fail 'version without prefix must fail'

workflow="$root/.github/workflows/publish-release.yml"
[[ -f "$workflow" ]] || fail 'workflow is missing'
[[ "$(find "$root/.github/workflows" -maxdepth 1 -name '*.yml' | wc -l | tr -d ' ')" == 1 ]] || fail 'expected one workflow'
grep -q '^name: Publish plugin release$' "$workflow" || fail 'workflow name'
# Stable releases also go to the license server, POM's fallback for GitHub.
grep -q 'POM_RELEASE_TOKEN' "$workflow" || fail 'workflow must publish stable releases to the license server'
grep -q 'softprops/action-gh-release' "$workflow" || fail 'GitHub release step is missing'
grep -q 'pom-plugin-\${{ matrix.platform }}.json' "$workflow" || fail 'public update manifest must be attached'
grep -q 'max-parallel: 1' "$workflow" || fail 'same-tag releases must be serialized'
grep -q 'persist-credentials: false' "$workflow" || fail 'checkout token should not persist'

printf 'test_ci_plan: ok\n'
