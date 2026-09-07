#!/usr/bin/env bash
set -euo pipefail
script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
base='' head=HEAD event=''
while (( $# )); do
  case "$1" in
    --base|--head|--event)
      (( $# >= 2 )) || { echo "Missing value for $1" >&2; exit 2; }
      case "$1" in --base) base=$2 ;; --head) head=$2 ;; --event) event=$2 ;; esac
      shift 2 ;;
    *) echo "Unknown argument: $1" >&2; exit 2 ;;
  esac
done
[[ -n $base && -f $event ]] || { echo 'Use --base REF --head REF --event FILE' >&2; exit 2; }
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
errors=0
fail() { printf '%s\n' "$*" >&2; errors=1; }
title=$(tq -r '.pull_request.title' "$event")
tq -r '.pull_request.body // ""' "$event" > "$work/body"
printf '%s\n' "$title" | rg -q '^(feat|fix|docs|refactor|perf|test|build|ci|chore|revert)(\([^)]+\))?!?: .+$' || fail 'PR title must be a Conventional Commit squash message'
fields=$(awk '/^OpenSpec-Changes:/ { n++ } END { print n+0 }' "$work/body")
: > "$work/ids"
if [[ $fields != 1 ]]; then
  fail 'Specify one OpenSpec-Changes: none or a comma-separated list of IDs'
else
  declaration=$(awk '/^OpenSpec-Changes:/ { sub(/^OpenSpec-Changes:[ \t]*/, ""); print }' "$work/body")
  if [[ $declaration != none ]]; then
    if printf '%s\n' "$declaration" | rg -q '^[a-z0-9]+(-[a-z0-9]+)*(, *[a-z0-9]+(-[a-z0-9]+)*)*$'; then
      printf '%s\n' "$declaration" | tr ',' '\n' | awk '{$1=$1; print}' >> "$work/ids"
      rg -qx none "$work/ids" && fail 'none cannot be combined with change IDs'
    else fail 'Invalid OpenSpec change ID list'; fi
  fi
fi
merge_base=$(git merge-base "$base" "$head")
git diff --name-only --no-renames "$merge_base" "$head" -- openspec/changes/ |
  awk -F / 'NF>=4 && $3!="archive" { print $3 }
    NF>=5 && $3=="archive" { sub(/^[0-9]+-[0-9]+-[0-9]+-/, "", $4); print $4 }' >> "$work/ids"
sort -u "$work/ids" > "$work/selected"
git ls-tree -r --name-only "$head" > "$work/files"
while IFS= read -r change; do
  rg -q "^openspec/changes/$change/" "$work/files" && fail "$change: active change must be archived"
  awk -F / -v id="$change" '$1=="openspec" && $2=="changes" && $3=="archive" {
    name=$4; sub(/^[0-9]+-[0-9]+-[0-9]+-/, "", name)
    if(name==id) print $1"/"$2"/"$3"/"$4
  }' "$work/files" | sort -u > "$work/archives"
  if [[ $(wc -l < "$work/archives" | tr -d ' ') != 1 ]]; then
    fail "$change: expected exactly one preserved archive"; continue
  fi
  IFS= read -r archive < "$work/archives"
  for artifact in proposal.md tasks.md; do
    if ! git show "$head:$archive/$artifact" > "$work/$artifact" 2>/dev/null; then
      fail "$change: archive missing $artifact"
    fi
  done
  if rg -q '^\s*- \[ \]' "$work/tasks.md"; then fail "$change: unfinished archived tasks"; fi
  awk -v prefix="$archive/specs/" 'index($0,prefix)==1 && /\/spec.md$/ { print }' "$work/files" > "$work/deltas"
  if [[ ! -s $work/deltas ]]; then
    if ! git show "$head:$archive/no-spec-deltas.md" > "$work/explanation" 2>/dev/null || ! rg -q '\S' "$work/explanation"; then
      fail "$change: document and review why this change has no spec deltas"
    fi
  fi
  while IFS= read -r delta; do
    main=openspec/${delta#"$archive/"}
    git show "$head:$delta" > "$work/delta"
    git show "$head:$main" > "$work/main" 2>/dev/null || : > "$work/main"
    awk -f "$script_dir/compare_requirements.awk" "$work/main" "$work/delta" || fail "$change: $delta is not synchronized"
  done < "$work/deltas"
done < "$work/selected"
(( errors == 0 )) || exit 1
printf 'PR title and associated OpenSpec changes pass\n'
