#!/usr/bin/env bash
set -euo pipefail
cd "${1:-$(dirname "${BASH_SOURCE[0]}")/..}"
bundle=crates/kibana-object-manager/docs
errors=0
fail() { printf '%s\n' "$*" >&2; errors=1; }

# Walk each component using directory entries, so macOS cannot hide case errors.
exact_path() {
  local path=$1 component current=. entry found
  [[ $path != /* ]] || current=/
  while [[ -n $path ]]; do
    component=${path%%/*}
    if [[ $path == */* ]]; then path=${path#*/}; else path=; fi
    case "$component" in ''|.) continue ;; ..) current=$current/..; continue ;; esac
    found=false
    for entry in "$current"/* "$current"/.[!.]* "$current"/..?*; do
      if [[ ${entry##*/} == "$component" && -e $entry ]]; then found=true; break; fi
    done
    "$found" || return 1
    current=$current/$component
  done
}

while IFS= read -r -d '' path; do
  name=${path##*/}
  [[ $name != *[A-Z]* ]] || fail "$path: docs filenames and directories must be lowercase"
done < <(find "$bundle" -mindepth 1 -print0)

files=(README.md AGENTS.md)
while IFS= read -r -d '' path; do files+=("$path"); done < <(find "$bundle" -type f -name '*.md' -print0)
for path in "${files[@]}"; do
  while IFS= read -r target; do
    target=${target%% \"*}; target=${target#<}; target=${target%>}
    case "$target" in *://*|mailto:*|\#*|//*) continue ;; esac
    target=${target%%#*}; target=${target%%\?*}
    [[ -n $target ]] || continue
    # Decode URL percent escapes without treating backslashes as shell escapes.
    target=$(printf '%s\n' "$target" | awk '
      function hex(c) { return index("0123456789abcdef", tolower(c))-1 }
      { while (match($0, /%[[:xdigit:]][[:xdigit:]]/)) {
          printf "%s%c", substr($0,1,RSTART-1), 16*hex(substr($0,RSTART+1,1))+hex(substr($0,RSTART+2,1))
          $0=substr($0,RSTART+3)
        }; print }
    ')
    exact_path "$(dirname "$path")/$target" || fail "$path: missing link or incorrect case: $target"
  done < <(awk '
    /^[ \t]*(```|~~~)/ { fence=!fence; next }
    !fence { while (match($0, /\]\([^)]*\)/)) {
      print substr($0,RSTART+2,RLENGTH-3); $0=substr($0,RSTART+RLENGTH)
    } }
  ' "$path")
done
for path in "$bundle"/*.md; do
  name=${path##*/}
  [[ $name == index.md ]] && continue
  rg -Fq "]($name)" "$bundle/index.md" || fail "Bundle index omits $name"
done
(( errors == 0 )) || exit 1
printf 'Documentation filenames, links, and bundle index pass\n'
