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
version = "0.1.0"           # Required, semver
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
   git tag -a v0.1.0 -m "Release v0.1.0"
   git push origin v0.1.0
   ```

2. The `release.yml` workflow will automatically:
   - Build binaries for all platforms
   - Create a GitHub release
   - Upload binaries as release assets

### Manual Release

If needed, create a release manually:

```bash
gh release create v0.1.0 \
  --title "v0.1.0" \
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
docker build -t yazeed/proc:0.1.0 .
docker tag yazeed/proc:0.1.0 yazeed/proc:latest

# Push
docker push yazeed/proc:0.1.0
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

For pre-1.0 releases:
- 0.1.0 → 0.2.0: New features
- 0.1.0 → 0.1.1: Bug fixes

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
