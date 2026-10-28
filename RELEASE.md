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

- [ ] CI is green on main, including "Coverage gate" (100% lines across Linux, macOS and Windows; see CONTRIBUTING.md "Coverage"). A tag release does not wait for it. Locally: `scripts/coverage-local.sh`

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

For a breaking release also run the migration cases from `[Unreleased]`: `proc by <part-of-name>` exits 2 and adding `--cmd` matches; `proc kill <pid> --json` exits 4 with `needs_confirmation`; `proc kill <pid> <pid> --dry-run` previews both.

### Install path check (before tagging)

Version numbers alone don't prove a channel works. Install the npm package from a local tarball. Run this **before the version bump in step 3**: the installer downloads the binary for the version in `pkg/npm/package.json`, which at this point is the previous, already-published release, so this tests the installer itself:

```bash
cd pkg/npm && npm pack --pack-destination /tmp/proc-npm && cd -
mkdir -p /tmp/proc-npm/install && cd /tmp/proc-npm/install
npm install /tmp/proc-npm/proc-cli-*.tgz && ./node_modules/.bin/proc --version
```

If anything here fails, fix it in this release, before the tag.

## 3. Version Bump

Update version in ALL of these files:
- [ ] `Cargo.toml` — version field
- [ ] `pkg/npm/package.json` — version field
- [ ] `flake.nix` — version field
- [ ] `.claude-plugin/marketplace.json` — `metadata.version` field
- [ ] `docs/_data/release.yml` — `version` field (the website's header, footer, install page and structured data read it)
- [ ] `Cargo.lock` — rebuild after the bump so its `proc-cli` entry changes, and commit it

(`pkg/aur/PKGBUILD` is not maintained; the README marks AUR as pending.)

## 4. Documentation

Update and verify all docs are complete:

- [ ] `CHANGELOG.md` — Rename `## [Unreleased]` to `## [X.Y.Z] - YYYY-MM-DD` and add a new empty `## [Unreleased]` above it. Everything worked on since the last release is already listed there (see "Unreleased Work" in CLAUDE.md); check that it is complete. Update the links at the bottom: `[Unreleased]` compares `vX.Y.Z...HEAD`, and add `[X.Y.Z]` comparing the previous tag. Merge duplicate headings (`### Added` appears twice in the 2.0.0 entries)
- [ ] `docs/changelog.md` — Add the same section (GitHub Pages)
- [ ] `README.md` — Review thoroughly: hero examples, Quick Start, Commands table, Filters table, Examples section all reflect new features
- [ ] `ROADMAP.md` — Mark completed items, update current release section
- [ ] `src/lib.rs` — Review Rust docs: Features list, Quick Start examples, Commands list all reflect new features. Run `cargo doc --no-deps --open` to preview. This is what shows on docs.rs.
- [ ] `skills/proc-cli/SKILL.md` — Agent Skills: new commands, flags, schemas, playbooks (the "proc 2.0 or later" floor stays unless the skill needs a newer proc)
- [ ] `skills/proc-cli/reference.md` — JSON output schemas match actual output
- [ ] `SECURITY.md` — supported versions table (new major or minor line)
- [ ] `docs/commands/` — GitHub Pages: new command pages, index.md updated
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
npm install --prefix /tmp/proc-npm-check proc-cli && /tmp/proc-npm-check/node_modules/.bin/proc --version  # npm installer works
brew update && brew info yazeed/proc/proc # Homebrew
brew upgrade yazeed/proc/proc && proc --version  # upgrades only proc, nothing else
docker pull yazeed/proc-cli               # Docker (the same image is also pushed as yazeed/proc)

# Linux binaries are static (musl): they must run on an older distro and on Alpine
mkdir -p /tmp/proc-linux && cd /tmp/proc-linux
curl -fsSL https://github.com/yazeed/proc-cli/releases/latest/download/proc-linux-x86_64.tar.gz | tar xz
curl -fsSL https://github.com/yazeed/proc-cli/releases/latest/download/proc-linux-aarch64.tar.gz | tar xz
for img in debian:bullseye-slim alpine; do
  docker run --rm --platform linux/amd64 -v "$PWD":/w $img /w/proc-linux-x86_64 --version
  docker run --rm --platform linux/arm64 -v "$PWD":/w $img /w/proc-linux-aarch64 --version
done
cd -

# Agent skill: skills.sh doesn't watch the repo, and there's no refresh command. It re-reads
# main when the skill is installed with telemetry on (no DISABLE_TELEMETRY / DO_NOT_TRACK):
# on 2026-10-27 one install updated its stored copy and page within minutes. An install on
# 2026-10-25, about an hour after its previous refresh, changed nothing; why is unknown.
mkdir -p /tmp/proc-skill-check && cd /tmp/proc-skill-check
bunx skills add yazeed/proc-cli --skill proc-cli -a claude-code -y   # installs into this folder only
cd - && rm -rf /tmp/proc-skill-check

# Is skills.sh's stored copy current? Prints "current" when its SKILL.md matches main's
curl -s https://skills.sh/api/download/yazeed/proc-cli/proc-cli \
  | uv run python -c 'import json,sys; f={x["path"]:x["contents"] for x in json.load(sys.stdin)["files"]}; print("current" if f["SKILL.md"]==open("skills/proc-cli/SKILL.md").read() else "stale")'
```

Also verify:
- [ ] https://docs.rs/proc-cli — Rust docs updated
- [ ] https://proc-cli.com/install — the header pill and the install page show the new version (GitHub Pages rebuilds a minute or two after the push)
- [ ] `proc --version` — Shows new version
- [ ] `proc manpage` — Includes new commands
- [ ] `proc completions bash` — Includes new commands
- [ ] skills.sh is current: the check above prints `current` (the page at https://www.skills.sh/yazeed/proc-cli/proc-cli is cached for up to 5 minutes). If it's stale, run the install again the next day. If that doesn't help either, ask for a refresh with an issue on [vercel-labs/skills](https://github.com/vercel-labs/skills/issues), which runs skills.sh; many such requests there are unanswered. Agents installing the skill are unaffected either way: `skills add` reads the current SKILL.md from GitHub

## 8. Cleanup

```bash
cargo clean    # Remove build artifacts (~10GB+ after a release session)
```

---

## Automated Publishing

On tag push, GitHub Actions handles:

1. **Build**: Cross-compile for all platforms
2. **GitHub Release**: Create release with binaries
3. **crates.io**: Publish with `--allow-dirty`
4. **npm**: Publish proc-cli package
5. **Homebrew**: Update yazeed/homebrew-proc tap
6. **Scoop**: Update yazeed/scoop-bucket-proc bucket
7. **Docker**: Push to yazeed/proc-cli on Docker Hub, and the same build to yazeed/proc (the image's original name)

## Required Secrets

Configure in GitHub repo settings → Secrets:

| Secret | Purpose |
|--------|---------|
| `HOMEBREW_TAP_TOKEN` | PAT for homebrew-proc repo |
| `SCOOP_BUCKET_TOKEN` | PAT for scoop-bucket-proc repo |
| `DOCKERHUB_USERNAME` | Docker Hub username |
| `DOCKERHUB_TOKEN` | Docker Hub access token |

npm and crates.io need no secret. Both publish through **Trusted Publishing (OIDC)**:
- **npm:** configured on npmjs.com under `proc-cli` → Settings → Trusted Publisher: GitHub Actions, `yazeed` / `proc-cli`, workflow `release.yml`, no environment, **Allow npm publish** checked. The `publish-npm` job needs `id-token: write` and npm 11.5.1 or newer.
- **crates.io:** configured under the `proc-cli` crate → Settings → Trusted Publishing: GitHub, `yazeed` / `proc-cli`, workflow `release.yml`, no environment.

If the repository or the workflow file is renamed, both trusted publishers must be recreated. A new npm trusted publisher expires unless a publish succeeds within 2 days, so add it shortly before tagging.

## Package Distribution

| Channel | Package Name | Install Command |
|---------|-------------|-----------------|
| crates.io | proc-cli | `cargo install proc-cli` |
| npm | proc-cli | `npm install -g proc-cli` |
| Homebrew | proc | `brew install yazeed/proc/proc` |
| Scoop | proc | `scoop bucket add proc https://github.com/yazeed/scoop-bucket-proc && scoop install proc` |
| Docker | yazeed/proc-cli (also yazeed/proc) | `docker run yazeed/proc-cli` |
| Nix | — | `nix profile install github:yazeed/proc-cli` |
