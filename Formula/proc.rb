# Homebrew formula for proc
# To install from a tap: brew install yazeed/tap/proc
# This file should be placed in a separate homebrew-tap repository

class Proc < Formula
  desc "Semantic process management CLI"
  homepage "https://github.com/yazeed/proc"
  version "1.0.0"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/yazeed/proc/releases/download/v#{version}/proc-darwin-aarch64.tar.gz"
      sha256 "PLACEHOLDER_SHA256_DARWIN_AARCH64"
    else
      url "https://github.com/yazeed/proc/releases/download/v#{version}/proc-darwin-x86_64.tar.gz"
      sha256 "PLACEHOLDER_SHA256_DARWIN_X86_64"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/yazeed/proc/releases/download/v#{version}/proc-linux-aarch64.tar.gz"
      sha256 "PLACEHOLDER_SHA256_LINUX_AARCH64"
    else
      url "https://github.com/yazeed/proc/releases/download/v#{version}/proc-linux-x86_64.tar.gz"
      sha256 "PLACEHOLDER_SHA256_LINUX_X86_64"
    end
  end

  def install
    bin.install "proc"
  end

  test do
    assert_match "proc", shell_output("#{bin}/proc --version")
  end
end

# Note: Update SHA256 hashes after each release
# Generate with: shasum -a 256 proc-*.tar.gz
