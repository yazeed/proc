# Homebrew Tap Setup Guide

This guide explains how to set up the `homebrew-proc` tap repository for distributing proc via Homebrew.

## Repository Structure

Create a new repository named `homebrew-proc` with this structure:

```
homebrew-proc/
├── Formula/
│   └── proc.rb
└── README.md
```

## Steps to Set Up

### 1. Create the Tap Repository

```bash
# Create new repo on GitHub: yazeed/homebrew-proc
gh repo create homebrew-proc --public --description "Homebrew tap for proc CLI"

# Clone and set up
git clone git@github.com:yazeed/homebrew-proc.git
cd homebrew-proc
mkdir Formula
```

### 2. Copy the Formula

Copy `Formula/proc.rb` from the main proc repository to `homebrew-proc/Formula/proc.rb`.

### 3. Update SHA256 Hashes After Release

After creating a GitHub release, download the binaries and generate SHA256 hashes:

```bash
# Download release assets
VERSION="0.1.0"
for ARCH in darwin-aarch64 darwin-x86_64 linux-aarch64 linux-x86_64; do
  curl -LO "https://github.com/yazeed/proc/releases/download/v${VERSION}/proc-${ARCH}.tar.gz"
done

# Generate SHA256 hashes
shasum -a 256 proc-*.tar.gz
```

Update the formula with the actual SHA256 values.

### 4. Commit and Push

```bash
git add Formula/proc.rb
git commit -m "Update proc to v${VERSION}"
git push origin main
```

## User Installation

Once the tap is set up, users can install proc with:

```bash
# Add the tap
brew tap yazeed/proc

# Install proc
brew install proc

# Or in one command
brew install yazeed/proc/proc
```

## Automated Updates

The release workflow in the main proc repository can be configured to automatically update the tap. Add this job to `.github/workflows/release.yml`:

```yaml
update-homebrew:
  needs: build
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
      with:
        repository: yazeed/homebrew-proc
        token: ${{ secrets.TAP_GITHUB_TOKEN }}

    - name: Update formula
      run: |
        # Download and hash binaries
        VERSION="${{ github.ref_name }}"
        VERSION="${VERSION#v}"

        # Update version and hashes in formula
        # (script to update proc.rb)

    - name: Commit and push
      run: |
        git config user.name "github-actions[bot]"
        git config user.email "github-actions[bot]@users.noreply.github.com"
        git commit -am "Update proc to v${VERSION}"
        git push
```

## Required Secrets

Add these secrets to your main proc repository:
- `TAP_GITHUB_TOKEN`: A GitHub token with write access to the homebrew-proc repository

## Troubleshooting

### Formula Audit

Before publishing, audit your formula:

```bash
brew audit --strict --online Formula/proc.rb
```

### Testing Installation

```bash
brew install --build-from-source Formula/proc.rb
```
