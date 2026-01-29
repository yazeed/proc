# Publishing Guide

This guide covers publishing proc to various package managers and registries.

## crates.io

### Prerequisites

1. Create an account at https://crates.io
2. Generate an API token at https://crates.io/settings/tokens
3. Login with cargo:
   ```bash
   cargo login <your-api-token>
   ```

### Pre-publish Checklist

- [ ] Version bumped in `Cargo.toml`
- [ ] CHANGELOG.md updated
- [ ] All tests passing: `cargo test`
- [ ] Clippy clean: `cargo clippy`
- [ ] Formatted: `cargo fmt`
- [ ] Documentation builds: `cargo doc`

### Publishing

```bash
# Dry run first
cargo publish --dry-run

# Publish
cargo publish
```

### Cargo.toml Requirements

The following fields are required/recommended for crates.io:

```toml
[package]
name = "proc-cli"           # Required, unique on crates.io
version = "1.0.0"           # Required, semver
edition = "2021"            # Required
authors = ["..."]           # Recommended
description = "..."         # Required, <100 chars
repository = "..."          # Recommended
homepage = "..."            # Recommended
documentation = "..."       # Recommended, defaults to docs.rs
license = "MIT"             # Required
keywords = ["...", "..."]   # Recommended, max 5
categories = ["..."]        # Recommended, from crates.io list
readme = "README.md"        # Recommended
```

## GitHub Releases

### Creating a Release

1. Tag the release:
   ```bash
   git tag -a v1.0.0 -m "Release v1.0.0"
   git push origin v1.0.0
   ```

2. The `release.yml` workflow will automatically:
   - Build binaries for all platforms
   - Create a GitHub release
   - Upload binaries as release assets

### Manual Release

If needed, create a release manually:

```bash
gh release create v1.0.0 \
  --title "v1.0.0" \
  --notes "See CHANGELOG.md for details" \
  target/release/proc
```

## Docker Hub

### Prerequisites

1. Create account at https://hub.docker.com
2. Login: `docker login`

### Publishing

```bash
# Build
docker build -t yazeed/proc:1.0.0 .
docker tag yazeed/proc:1.0.0 yazeed/proc:latest

# Push
docker push yazeed/proc:1.0.0
docker push yazeed/proc:latest
```

### Automated Publishing

Add to `.github/workflows/release.yml`:

```yaml
publish-docker:
  needs: build
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4

    - name: Login to Docker Hub
      uses: docker/login-action@v3
      with:
        username: ${{ secrets.DOCKER_USERNAME }}
        password: ${{ secrets.DOCKER_TOKEN }}

    - name: Build and push
      uses: docker/build-push-action@v5
      with:
        push: true
        tags: |
          yazeed/proc:${{ github.ref_name }}
          yazeed/proc:latest
```

## Homebrew

See [HOMEBREW_TAP.md](./HOMEBREW_TAP.md) for detailed Homebrew setup instructions.

## Release Process

### Version Bump Steps

When releasing a new version, follow these steps in order:

#### 1. Pre-Release Checks

```bash
# Ensure all tests pass
cargo test

# Ensure no clippy warnings
cargo clippy

# Ensure code is formatted
cargo fmt --check

# Build release to verify it compiles
cargo build --release
```

#### 2. Update Version Numbers

Update the version in these files:

**Cargo.toml** (line ~3):
```toml
version = "X.Y.Z"
```

**Formula/proc.rb** (line ~2):
```ruby
version "X.Y.Z"
```

#### 3. Update CHANGELOG.md

Add a new section at the top following this format:

```markdown
## [X.Y.Z] - YYYY-MM-DD

### Added
- New feature description

### Changed
- Changed behavior description

### Fixed
- Bug fix description

### Removed
- Removed feature description
```

#### 4. Commit and Tag

```bash
# Stage all changes
git add -A

# Commit with conventional commit message
git commit -m "chore: bump version to X.Y.Z"

# Create annotated tag
git tag -a vX.Y.Z -m "Release vX.Y.Z"

# Push commit and tag
git push && git push --tags
```

#### 5. Wait for CI/CD

The GitHub Actions `release.yml` workflow will automatically:
- Build binaries for all platforms (macOS, Linux, Windows)
- Create a GitHub release with the tag
- Upload all binaries as release assets

Monitor the workflow at: https://github.com/yazeed/proc/actions

#### 6. Post-Release Tasks

**Publish to crates.io:**
```bash
cargo publish
```

**Update Homebrew tap:**
1. Download the release binaries and compute SHA256:
   ```bash
   # For each platform binary
   curl -fsSL https://github.com/yazeed/proc/releases/download/vX.Y.Z/proc-darwin-aarch64 | shasum -a 256
   ```
2. Update the SHA256 hashes in the homebrew-proc tap repository
3. Push the tap update

**Announce the release:**
- Update GitHub Discussions if significant changes
- Tweet/post if desired

### Version Numbering (SemVer)

Follow [Semantic Versioning](https://semver.org/):

- **MAJOR** (X.0.0): Breaking changes, incompatible API changes
- **MINOR** (0.X.0): New features, backwards compatible
- **PATCH** (0.0.X): Bug fixes, backwards compatible

### Quick Checklist

- [ ] All tests pass (`cargo test`)
- [ ] No clippy warnings (`cargo clippy`)
- [ ] Code formatted (`cargo fmt`)
- [ ] Version updated in `Cargo.toml`
- [ ] Version updated in `Formula/proc.rb`
- [ ] CHANGELOG.md updated
- [ ] Committed with message `chore: bump version to X.Y.Z`
- [ ] Tagged with `vX.Y.Z`
- [ ] Pushed commit and tag
- [ ] CI builds completed successfully
- [ ] Published to crates.io
- [ ] Homebrew tap SHA256 hashes updated

## Scoop (Windows)

Scoop uses a bucket (repository of manifests) to distribute packages.

### Setup (One-time)

1. Create a scoop bucket repo (e.g., `github.com/yazeed/scoop-bucket`)
2. Add `proc.json` manifest to the bucket
3. Users add your bucket: `scoop bucket add yazeed https://github.com/yazeed/scoop-bucket`

### Publishing a New Version

1. **Update the manifest** (`pkg/scoop/proc.json`):
   ```bash
   # Update version
   sed -i 's/"version": ".*"/"version": "X.Y.Z"/' pkg/scoop/proc.json
   
   # Download new release and compute hash
   curl -fsSL https://github.com/yazeed/proc/releases/download/vX.Y.Z/proc-windows-x86_64.exe.zip -o /tmp/proc.zip
   shasum -a 256 /tmp/proc.zip
   
   # Update hash in manifest
   # Update the URL version in the manifest
   ```

2. **Copy to bucket repo** and push:
   ```bash
   cp pkg/scoop/proc.json /path/to/scoop-bucket/proc.json
   cd /path/to/scoop-bucket
   git add proc.json && git commit -m "proc X.Y.Z" && git push
   ```

3. **Auto-update**: The manifest has `"checkver": "github"` and `"autoupdate"` configured, so Scoop can auto-update the URL (but hash must be updated manually or via CI).

### User Installation

```powershell
scoop bucket add yazeed https://github.com/yazeed/scoop-bucket
scoop install proc
```

## AUR (Arch Linux)

The AUR (Arch User Repository) hosts community-maintained packages.

### Setup (One-time)

1. Create an AUR account at https://aur.archlinux.org
2. Set up SSH keys for AUR: https://wiki.archlinux.org/title/AUR_submission_guidelines
3. Clone your package base:
   ```bash
   git clone ssh://aur@aur.archlinux.org/proc.git
   ```

### Publishing a New Version

1. **Update PKGBUILD** (`pkg/aur/PKGBUILD`):
   ```bash
   # Update pkgver
   sed -i 's/pkgver=.*/pkgver=X.Y.Z/' pkg/aur/PKGBUILD
   
   # Download source and compute SHA256
   curl -fsSL https://github.com/yazeed/proc/archive/vX.Y.Z.tar.gz -o /tmp/proc.tar.gz
   shasum -a 256 /tmp/proc.tar.gz
   
   # Update sha256sums in PKGBUILD
   ```

2. **Generate .SRCINFO**:
   ```bash
   cd pkg/aur
   makepkg --printsrcinfo > .SRCINFO
   ```

3. **Push to AUR**:
   ```bash
   cd /path/to/aur-proc-clone
   cp /path/to/proc/pkg/aur/PKGBUILD .
   cp /path/to/proc/pkg/aur/.SRCINFO .
   git add PKGBUILD .SRCINFO
   git commit -m "Update to X.Y.Z"
   git push
   ```

### User Installation

```bash
yay -S proc
# or
paru -S proc
```

## npm (Node.js wrapper)

The npm package is a wrapper that downloads the native binary on install.

### Setup (One-time)

1. Create npm account: https://www.npmjs.com/signup
2. Login: `npm login`
3. Note: Package name is `proc-cli` (not `proc` which is taken)

### Publishing a New Version

1. **Update package.json** (`pkg/npm/package.json`):
   ```bash
   cd pkg/npm
   npm version X.Y.Z --no-git-tag-version
   ```

2. **Test locally**:
   ```bash
   cd pkg/npm
   npm pack  # Creates proc-cli-X.Y.Z.tgz
   npm install -g proc-cli-X.Y.Z.tgz
   proc --version
   ```

3. **Publish**:
   ```bash
   cd pkg/npm
   npm publish
   ```

### User Installation

```bash
npm install -g proc-cli
```

## Nix Flakes

Nix flakes auto-publish from the GitHub repo. No manual steps needed.

### How It Works

The `flake.nix` at the repo root defines the package. Users install directly from GitHub:

```bash
nix profile install github:yazeed/proc
```

### Updating

When you push a new tag, users can update with:
```bash
nix profile upgrade proc
```

Or pin to a specific version:
```bash
nix profile install github:yazeed/proc/v1.3.0
```

## Tier 3 Package Release Checklist

After a new release, update these packages:

- [ ] **Scoop**: Update `pkg/scoop/proc.json` version and hash, push to bucket
- [ ] **AUR**: Update `pkg/aur/PKGBUILD` version and hash, generate .SRCINFO, push to AUR
- [ ] **npm**: Update `pkg/npm/package.json` version, run `npm publish`
- [ ] **Nix**: No action needed (auto-updates from GitHub)
