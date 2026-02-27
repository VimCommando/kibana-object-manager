# Release Checklist

## Inputs

- Target version: `<x.y.z>`
- Tag: `v<x.y.z>`
- Release date for changelog section

## Main Repo Commands

```bash
# from kibana-object-manager repo
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo publish --dry-run
cargo publish
git tag -a v<x.y.z> -m "Release version <x.y.z>"
git push origin main --tags
```

## GitHub Release

- Create release for `v<x.y.z>`.
- Use changelog section as release notes.

## Homebrew Formula Update

Formula file:
- `Formula/kibob.rb` in `VimCommando/homebrew-kibob`

Preferred command:

```bash
python skills/publish-kibob/scripts/update_homebrew_formula.py \
  --version <x.y.z> \
  --formula /path/to/homebrew-kibob/Formula/kibob.rb
```

Update fields:
- `url "https://github.com/VimCommando/kibana-object-manager/archive/refs/tags/v<x.y.z>.tar.gz"`
- `sha256 "<sha256>"`

Compute sha256:

```bash
curl -L "https://github.com/VimCommando/kibana-object-manager/archive/refs/tags/v<x.y.z>.tar.gz" | shasum -a 256
```

Optional formula checks:

```bash
brew audit --formula Formula/kibob.rb
brew install --build-from-source ./Formula/kibob.rb
```

## Final Verification

- crates.io shows `kibana-object-manager <x.y.z>`
- GitHub release/tag `v<x.y.z>` exists
- Homebrew formula has matching `url` + `sha256`
- `kibob --version` reports expected version after install
