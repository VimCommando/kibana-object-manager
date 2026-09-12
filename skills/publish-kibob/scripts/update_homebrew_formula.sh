#!/usr/bin/env bash
set -euo pipefail
version='' formula='' check_archive=''
while (( $# )); do
  case "$1" in
    --version|--formula|--check-archive)
      (( $# >= 2 )) || { echo "Missing value for $1" >&2; exit 2; }
      case "$1" in --version) version=$2 ;; --formula) formula=$2 ;; --check-archive) check_archive=$2 ;; esac
      shift 2 ;;
    *) echo "Unknown argument: $1" >&2; exit 2 ;;
  esac
done
numeric_identifier='(0|[1-9][0-9]*)'
prerelease_identifier='(0|[1-9][0-9]*|[0-9A-Za-z-]*[A-Za-z-][0-9A-Za-z-]*)'
build_identifier='[0-9A-Za-z-]+'
printf '%s\n' "$version" | rg -q "^$numeric_identifier\.$numeric_identifier\.$numeric_identifier(-$prerelease_identifier(\.$prerelease_identifier)*)?(\+$build_identifier(\.$build_identifier)*)?$" || {
  echo 'Use --version with a semantic version without a v prefix' >&2; exit 2;
}
[[ -n $check_archive || -f $formula ]] || { echo 'Use --formula with an existing formula' >&2; exit 2; }
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
url="https://github.com/VimCommando/kibana-object-manager/archive/refs/tags/v${version}.tar.gz"
if [[ -n $check_archive ]]; then
  cp "$check_archive" "$work/source.tar.gz"
else
  curl --fail --location --connect-timeout 10 --max-time 120 --max-filesize 134217728 "$url" -o "$work/source.tar.gz"
fi
(( $(wc -c < "$work/source.tar.gz") <= 134217728 )) || { echo 'Archive exceeds 128 MiB' >&2; exit 1; }
# Bound expansion before extraction. Only ordinary files and directories are allowed.
if gzip -dc "$work/source.tar.gz" | head -c 268435457 | wc -c > "$work/expanded"; then
  expansion_status=(0 0 0)
else
  expansion_status=("${PIPESTATUS[@]}")
fi
read -r expanded < "$work/expanded"
# A bounded read may terminate gzip with SIGPIPE. Report oversize first, while
# still rejecting every decompression/read failure for streams below the limit.
(( expanded <= 268435456 )) || { echo 'Expanded archive exceeds 256 MiB' >&2; exit 1; }
(( expansion_status[0] == 0 && expansion_status[1] == 0 && expansion_status[2] == 0 )) || {
  echo 'Invalid gzip archive or failed expansion check' >&2; exit 1;
}
tar -tzf "$work/source.tar.gz" > "$work/members"
[[ -s $work/members ]] || { echo 'Empty archive' >&2; exit 1; }
if rg -q '(^/|(^|/)\.\.(/|$))' "$work/members"; then echo 'Unsafe archive paths' >&2; exit 1; fi
tar -tvzf "$work/source.tar.gz" > "$work/verbose"
cut -c1 "$work/verbose" > "$work/types"
if rg -qv '^[-d]$' "$work/types"; then echo 'Archive links and special files are unsupported' >&2; exit 1; fi
if [[ -n $(sort "$work/members" | uniq -d) ]]; then echo 'Duplicate archive members' >&2; exit 1; fi
cut -d/ -f1 "$work/members" | sort -u > "$work/roots"
[[ $(wc -l < "$work/roots" | tr -d ' ') == 1 ]] || { echo 'Archive must have one root' >&2; exit 1; }
IFS= read -r archive_root < "$work/roots"
tar -xzf "$work/source.tar.gz" -C "$work" --no-same-owner --no-same-permissions
source_root=$work/$archive_root
license=$source_root/LICENCE.md
[[ -f $license ]] || license=$source_root/LICENSE
rg -q 'Apache License' "$license"
rg -q 'Version 2.0' "$license"
# Cargo parses TOML and workspace membership without building downloaded code.
cargo metadata --manifest-path "$source_root/Cargo.toml" --format-version 1 --offline --no-deps | tq -x > "$work/metadata"
for package in kibana-sync kibana-object-manager; do
  # shellcheck disable=SC2016 # tq evaluates its own variable bindings.
  package_version=$(tq -i toon-seq -er --arg name "$package" '.packages[] | select(.name == $name and .license == "Apache-2.0") | .version' "$work/metadata")
  [[ -n $package_version ]] || { echo "Missing workspace package: $package" >&2; exit 1; }
  if [[ $package == kibana-object-manager && $package_version != "$version" ]]; then
    echo 'CLI archive version does not match the selected release' >&2; exit 1
  fi
  # Cargo.lock is Cargo-generated; inspect its package records, not manifests.
  awk -v wanted_name="$package" -v wanted_version="$package_version" '
    function finish() { if(name==wanted_name && version==wanted_version) found=1 }
    /^\[\[package\]\]/ { finish(); name=""; version=""; package=1; next }
    /^\[/ { package=0 }
    package && /^name[ \t]*=/ { name=$0; sub(/^[^"]*"/, "", name); sub(/".*$/, "", name) }
    package && /^version[ \t]*=/ { version=$0; sub(/^[^"]*"/, "", version); sub(/".*$/, "", version) }
    END { finish(); exit(!found) }
  ' "$source_root/Cargo.lock" || { echo "Lockfile version mismatch for $package" >&2; exit 1; }
done
if [[ -n $check_archive ]]; then printf 'Source archive passes\n'; exit 0; fi
digest=$(shasum -a 256 "$work/source.tar.gz" | cut -d ' ' -f1)
# Validate both fields before replacing either. ENVIRON avoids awk escape decoding.
KIBOB_FORMULA_URL=$url KIBOB_FORMULA_HASH=$digest awk '
  /^[ \t]*url[ \t]+"[^"]+"[ \t]*$/ && !url++ { sub(/"[^"]+"/, "\"" ENVIRON["KIBOB_FORMULA_URL"] "\"") }
  /^[ \t]*sha256[ \t]+"[0-9a-fA-F]+"[ \t]*$/ && !hash++ { sub(/"[^"]+"/, "\"" ENVIRON["KIBOB_FORMULA_HASH"] "\"") }
  { print }
  END { if(!url || !hash) { print "Formula needs source URL and SHA256 fields" > "/dev/stderr"; exit 1 } }
' "$formula" > "$work/formula"
cat "$work/formula" > "$formula"
printf 'Updated %s\nurl: %s\nsha256: %s\n' "$formula" "$url" "$digest"
