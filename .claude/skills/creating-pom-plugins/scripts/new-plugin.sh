#!/usr/bin/env bash
# Creates a POM plugin repository from the skill's template.
#
#   bash new-plugin.sh <dest>/pom-plugin-<name> --code <plugin_code> --name "<Display Name>"
#
# The destination's directory name is the repository and crate name and must
# start with `pom-plugin-`; the POM's development rebuild only discovers
# sources named that way.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
template="$here/template"
dest=""
code=""
name=""

usage() {
  printf '%s\n' \
    'Usage: new-plugin.sh <dir>/pom-plugin-<name> --code <plugin_code> --name "<Display Name>"' \
    '  --code   ^[a-z][a-z0-9_]{1,63}$, the plugin_code (entry pom_<code>_plugin_v1)' \
    '  --name   the name shown in the menu and on the Plugins card'
}

die() {
  printf 'new-plugin: %s\n' "$1" >&2
  exit 2
}

while (($# > 0)); do
  case "$1" in
    --code) (($# >= 2)) || die '--code requires a value'; code="$2"; shift 2 ;;
    --name) (($# >= 2)) || die '--name requires a value'; name="$2"; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    -*) die "unknown option: $1" ;;
    *) [[ -z "$dest" ]] || die "unexpected argument: $1"; dest="$1"; shift ;;
  esac
done

[[ -n "$dest" && -n "$code" && -n "$name" ]] || { usage >&2; exit 2; }
[[ "$code" =~ ^[a-z][a-z0-9_]{1,63}$ ]] || die "invalid plugin_code: $code"
[[ "$name" != *[\"\\\|]* ]] || die 'the name may not contain quotes, backslashes or |'
repo="$(basename "$dest")"
[[ "$repo" =~ ^pom-plugin-[a-z0-9][a-z0-9.-]*$ ]] || die "the directory must be named pom-plugin-<name>: $repo"
[[ ! -e "$dest" ]] || [[ -z "$(ls -A "$dest")" ]] || die "destination is not empty: $dest"
crate="${repo//[-.]/_}"

mkdir -p "$dest"
cp -a "$template/." "$dest/"

# Replace the placeholders in every text file (the PNGs stay as they are).
while IFS= read -r -d '' file; do
  grep -Iq . "$file" || continue
  sed -i.bak \
    -e "s|__CODE__|${code}|g" \
    -e "s|__REPO__|${repo}|g" \
    -e "s|__CRATE__|${crate}|g" \
    -e "s|__NAME__|${name}|g" \
    "$file"
  rm -f "$file.bak"
done < <(find "$dest" -type f ! -name '*.png' -print0)

if grep -rIl -e '__CODE__' -e '__REPO__' -e '__CRATE__' -e '__NAME__' "$dest" >/dev/null; then
  die 'placeholders left behind'
fi

printf 'created %s (plugin_code=%s, crate=%s)\n' "$dest" "$code" "$crate"
printf 'next:\n  cd %s\n  git init -b main\n  scripts/build-ui.sh && POM_UI_REQUIRE_DIST=1 cargo test\n' "$dest"
