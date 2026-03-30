# Release Process

Follow these steps for every release. Do not skip steps.

## 1. Pre-Release Checks

```bash
cargo fmt --check        # Code formatted
cargo clippy             # No warnings
cargo test               # Tests pass
cargo build --release    # Release builds
cargo update             # Update dependencies
cargo audit              # No security vulnerabilities
```

### Cross-Platform Check

`freeze` and `thaw` use `#[cfg(unix)]` — imports gated behind `cfg(unix)` cause unused-import errors on Windows with `-D warnings`. Always verify:

```bash
rustup target add x86_64-pc-windows-msvc  # One-time setup
cargo check --target x86_64-pc-windows-msvc
```

## 2. Manual Testing

Test new features manually with the release binary:

```bash
./target/release/proc --version    # Verify version
./target/release/proc --help       # Verify help includes new commands
```

Test new commands/flags work as expected before releasing.

## 3. Version Bump

Update version in ALL of these files:
- [ ] `Cargo.toml` — version field
- [ ] `pkg/npm/package.json` — version field
- [ ] `flake.nix` — version field

## 4. Documentation

Update and verify all docs are complete:

- [ ] `CHANGELOG.md` — Add new version section with changes
- [ ] `README.md` — Review thoroughly: hero examples, Quick Start, Commands table, Filters table, Examples section all reflect new features
- [ ] `ROADMAP.md` — Mark completed items, update current release section
- [ ] `src/lib.rs` — Review Rust docs: Features list, Quick Start, Commands list all reflect new features (shows on docs.rs)
- [ ] Verify `proc manpage` output includes new commands
- [ ] Verify `proc completions` includes new commands

## 5. Commit & Tag

```bash
git add -A
git commit -m "chore: release vX.Y.Z"
git tag vX.Y.Z
git push && git push --tags
```

## 6. Verify CI

After tag push, monitor GitHub Actions:
- [ ] Build succeeds for all platforms
- [ ] GitHub Release created with binaries
- [ ] crates.io published
- [ ] npm published
- [ ] Homebrew tap updated
- [ ] Scoop bucket updated
- [ ] Docker image pushed

## 7. Post-Release Verification

```bash
# Verify packages (after CI completes)
cargo install proc-cli                    # crates.io
npm info proc-cli version                 # npm
brew update && brew info yazeed/proc/proc # Homebrew
docker pull yazeed/proc                   # Docker
```

Also verify:
- [ ] https://docs.rs/proc-cli — Rust docs updated
- [ ] `proc --version` — Shows new version
- [ ] `proc manpage` — Includes new commands
- [ ] `proc completions bash` — Includes new commands

---

## Automated Publishing

On tag push, GitHub Actions handles:

1. **Build**: Cross-compile for all platforms
2. **GitHub Release**: Create release with binaries
3. **crates.io**: Publish with `--allow-dirty`
4. **npm**: Publish proc-cli package
5. **Homebrew**: Update yazeed/homebrew-proc tap
6. **Scoop**: Update yazeed/scoop-bucket-proc bucket
7. **Docker**: Push to yazeed/proc on Docker Hub

## Required Secrets

Configure in GitHub repo settings → Secrets:

| Secret | Purpose |
|--------|---------|
| `HOMEBREW_TAP_TOKEN` | PAT for homebrew-proc repo |
| `SCOOP_BUCKET_TOKEN` | PAT for scoop-bucket-proc repo |
| `NPM_TOKEN` | npm automation token |
| `DOCKERHUB_USERNAME` | Docker Hub username |
| `DOCKERHUB_TOKEN` | Docker Hub access token |

## Package Distribution

| Channel | Package Name | Install Command |
|---------|-------------|-----------------|
| crates.io | proc-cli | `cargo install proc-cli` |
| npm | proc-cli | `npm install -g proc-cli` |
| Homebrew | proc | `brew install yazeed/proc/proc` |
| Scoop | proc | `scoop bucket add proc https://github.com/yazeed/scoop-bucket-proc && scoop install proc` |
| Docker | yazeed/proc | `docker run yazeed/proc` |
| Nix | — | `nix profile install github:yazeed/proc` |
