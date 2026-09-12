#!/usr/bin/env bash
set -euo pipefail
repo=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
count=0
pass() {
  local label=$1; shift
  if ! "$@" > "$work/output" 2>&1; then cat "$work/output" >&2; echo "FAIL: $label" >&2; exit 1; fi
  count=$((count+1)); printf 'ok %s - %s\n' "$count" "$label"
}
reject() {
  local label=$1 pattern=$2; shift 2
  if "$@" > "$work/output" 2>&1; then echo "Unexpected success: $label" >&2; exit 1; fi
  if ! rg -q "$pattern" "$work/output"; then cat "$work/output" >&2; echo "Wrong failure: $label" >&2; exit 1; fi
  count=$((count+1)); printf 'ok %s - %s\n' "$count" "$label"
}

docs=$work/docs_repo/crates/kibana-object-manager/docs
mkdir -p "$docs/images"
printf '[Guide](crates/kibana-object-manager/docs/guide.md)\n' > "$work/docs_repo/README.md"
: > "$work/docs_repo/AGENTS.md"
printf '1. [Guide](guide.md)\n' > "$docs/index.md"
printf '# Guide\n' > "$docs/guide.md"
: > "$docs/images/example-image.svg"
pass 'lowercase nested docs assets' bash "$repo/scripts/check_docs.sh" "$work/docs_repo"
mv "$docs/images/example-image.svg" "$docs/images/Example.svg"
reject 'uppercase nested asset' 'must be lower-kebab-case' bash "$repo/scripts/check_docs.sh" "$work/docs_repo"
mv "$docs/images/Example.svg" "$docs/images/example.svg"
mv "$docs/images" "$docs/Images"
reject 'uppercase directory' 'must be lower-kebab-case' bash "$repo/scripts/check_docs.sh" "$work/docs_repo"
mv "$docs/Images" "$docs/images"
for name in example_image.svg 'example image.svg'; do
  mv "$docs/images/example.svg" "$docs/images/$name"
  reject "non-kebab filename: $name" 'must be lower-kebab-case' bash "$repo/scripts/check_docs.sh" "$work/docs_repo"
  mv "$docs/images/$name" "$docs/images/example.svg"
done
printf '[Guide](crates/kibana-object-manager/docs/GUIDE.md)\n' > "$work/docs_repo/README.md"
reject 'wrong link case' 'incorrect case' bash "$repo/scripts/check_docs.sh" "$work/docs_repo"

fixture=$work/git_repo
new_repo() {
  rm -rf "$fixture"; mkdir -p "$fixture"; cd "$fixture"
  git init -q
  git config user.name 'Fixture'; git config user.email fixture@example.invalid
  git config commit.gpgsign false; git config core.hooksPath /dev/null
  mkdir -p openspec/changes/unrelated
  echo 'In progress' > openspec/changes/unrelated/proposal.md
  echo 'Fixture' > README.md
  commit
  base=$(git rev-parse HEAD)
}
commit() { git add .; git commit -qm 'test: fixture'; }
event() {
  # shellcheck disable=SC2016 # tq evaluates the bindings; fixtures use GitHub JSON.
  tq -n --arg title "${2:-fix: fixture}" --arg body "${1:-OpenSpec-Changes: none}" \
    '{pull_request: {title: $title, body: $body}}' | tq -x -i toon-seq -o json | tr -d '\036' > "$work/event.json"
}
gate() { bash "$repo/scripts/check_pr.sh" --base "$base" --head HEAD --event "$work/event.json"; }
requirement=$work/requirement
cat > "$requirement" <<'EOF'
### Requirement: Example
The tool SHALL work.

#### Scenario: Run
- **WHEN** run
- **THEN** it works
EOF
archive_change() {
  local archive=openspec/changes/archive/2026-09-07-$1
  mkdir -p "$archive/specs/example"
  echo 'Proposal' > "$archive/proposal.md"
  echo '- [x] Complete' > "$archive/tasks.md"
  { printf '## ADDED Requirements\n\n'; cat "$requirement"; } > "$archive/specs/example/spec.md"
}
event
new_repo
echo 'Updated' >> README.md; commit
pass 'unrelated active change' gate
event 'OpenSpec-Changes: unrelated'
reject 'explicit association without artifact diff' 'active change' gate
event
echo 'Modified' >> openspec/changes/unrelated/proposal.md; commit
reject 'edited active change' 'active change' gate
rm openspec/changes/unrelated/proposal.md; commit
reject 'deletion is not archival' 'preserved archive' gate
new_repo
git mv openspec/changes/unrelated openspec/changes/renamed; commit
reject 'rename checks both paths' 'unrelated: expected' gate
new_repo
archive_change example; commit
reject 'skipped synchronization' 'not synchronized' gate
mkdir -p openspec/specs/example; cp "$requirement" openspec/specs/example/spec.md; commit
pass 'synchronized new archive' gate
new_repo
mkdir -p openspec/specs/example; cp "$requirement" openspec/specs/example/spec.md; commit
base=$(git rev-parse HEAD)
archive_change one; archive_change two; commit
event 'OpenSpec-Changes: one, two'
pass 'multiple changes with already synced specs' gate
event 'No association field'
reject 'missing association' 'Specify one OpenSpec' gate
event 'OpenSpec-Changes: none' 'Update things'
reject 'nonconventional title' 'Conventional Commit' gate
event $'OpenSpec-Changes: none\nOpenSpec-Changes: unrelated'
reject 'duplicate association' 'Specify one OpenSpec' gate
: > "$work/empty"
{ echo '## REMOVED Requirements'; cat "$requirement"; } > "$work/delta"
pass 'removed requirement absent' awk -f "$repo/scripts/compare_requirements.awk" "$work/empty" "$work/delta"
reject 'removed requirement remains' 'removed requirement remains' awk -f "$repo/scripts/compare_requirements.awk" "$requirement" "$work/delta"
# shellcheck disable=SC2016 # Backticks are literal OpenSpec Markdown.
printf '%s\n' '## RENAMED Requirements' '- FROM: `### Requirement: Old`' '- TO: `### Requirement: Example`' > "$work/delta"
pass 'renamed requirement synchronized' awk -f "$repo/scripts/compare_requirements.awk" "$requirement" "$work/delta"
reject 'renamed requirement missing' 'rename not synchronized' awk -f "$repo/scripts/compare_requirements.awk" "$work/empty" "$work/delta"

cd "$work"
mkdir -p source/crates/{kibana-sync,kibana-object-manager}/src
printf '[workspace]\nmembers = ["crates/kibana-sync", "crates/kibana-object-manager"]\nresolver = "3"\n' > source/Cargo.toml
for package in kibana-sync kibana-object-manager; do
  printf '[package]\nname="%s"\nversion="0.4.0"\nlicense="Apache-2.0"\nedition="2024"\n' "$package" > "source/crates/$package/Cargo.toml"
  : > "source/crates/$package/src/lib.rs"
  printf '[[package]]\nname="%s"\nversion="0.4.0"\n' "$package" >> source/Cargo.lock
done
printf 'Apache License\nVersion 2.0\n' > source/LICENCE.md
updater=$repo/skills/publish-kibob/scripts/update_homebrew_formula.sh
pack() { tar -czf "$work/source.tar.gz" source; }
check_archive() { bash "$updater" --version 0.4.0 --check-archive "$work/source.tar.gz"; }
pack
pass 'source package archive' check_archive
mv source/LICENCE.md source/LICENSE; pack
pass 'historical license archive' check_archive
reject 'wrong release' 'selected release' bash "$updater" --version 0.5.0 --check-archive "$work/source.tar.gz"
ln -s /tmp source/unsafe; pack
reject 'archive link' 'links and special files' check_archive
rm source/unsafe
cp source/Cargo.lock "$work/valid.lock"
sed 's/0.4.0/0.3.0/g' "$work/valid.lock" > source/Cargo.lock; pack
reject 'lockfile disagreement' 'Lockfile version mismatch' check_archive
cp "$work/valid.lock" source/Cargo.lock; pack
reject 'invalid version' 'semantic version' bash "$updater" --version ../../main --check-archive "$work/source.tar.gz"
printf 'server error' > "$work/source.tar.gz"
reject 'not an archive' 'gzip|format|compressed' check_archive
dd if=/dev/zero bs=1048576 count=257 2>/dev/null | gzip > "$work/source.tar.gz"
reject 'oversized expansion has actionable error' 'Expanded archive exceeds 256 MiB' check_archive
pack
head -c "$(( $(wc -c < "$work/source.tar.gz") - 4 ))" "$work/source.tar.gz" > "$work/truncated.gz"
mv "$work/truncated.gz" "$work/source.tar.gz"
reject 'truncated gzip remains rejected' 'gzip|compressed|unexpected|invalid' check_archive
pack
mkdir "$work/bin"
cat > "$work/bin/curl" <<'EOF'
#!/usr/bin/env bash
while (( $# )); do
  if [[ $1 == -o ]]; then cp "$KIBOB_TEST_ARCHIVE" "$2"; exit; fi
  shift
done
exit 1
EOF
chmod +x "$work/bin/curl"
printf 'class Kibob < Formula\n  url "old"\n  sha256 "abcd"\n  license "Apache-2.0"\nend\n' > "$work/formula.rb"
pass 'formula update after verification' env PATH="$work/bin:$PATH" KIBOB_TEST_ARCHIVE="$work/source.tar.gz" bash "$updater" --version 0.4.0 --formula "$work/formula.rb"
rg -q 'v0.4.0.tar.gz' "$work/formula.rb"
rg -q 'license "Apache-2.0"' "$work/formula.rb"
printf 'class Kibob < Formula\nend\n' > "$work/formula.rb"
cp "$work/formula.rb" "$work/original"
reject 'invalid formula is unchanged' 'needs source URL' env PATH="$work/bin:$PATH" KIBOB_TEST_ARCHIVE="$work/source.tar.gz" bash "$updater" --version 0.4.0 --formula "$work/formula.rb"
cmp "$work/original" "$work/formula.rb"
printf '%s Bash maintenance checks passed\n' "$count"
