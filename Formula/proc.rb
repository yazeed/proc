class Proc < Formula
  desc "Semantic process management CLI"
  homepage "https://github.com/yazeed/proc"
  version "1.0.0"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/yazeed/proc/releases/download/v#{version}/proc-darwin-aarch64.tar.gz"
      sha256 "3001746102b8d56164ed3cd1e9004d465aefc290114f32c26306d906b2ea9cc5"
    else
      url "https://github.com/yazeed/proc/releases/download/v#{version}/proc-darwin-x86_64.tar.gz"
      sha256 "cb86e0a10df4e14a3b8b79db8370cbbcb243b98216fc2d5074a0d7a4ef78eb28"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/yazeed/proc/releases/download/v#{version}/proc-linux-aarch64.tar.gz"
      sha256 "3aa4c73fe3f5c7d117696c92d0aaf980e14b81485127c50dea69d9bbd814171f"
    else
      url "https://github.com/yazeed/proc/releases/download/v#{version}/proc-linux-x86_64.tar.gz"
      sha256 "f60851d817b1026534fda1a70df409a6e4ed7565d893113cce22c931ae58c8ea"
    end
  end

  def install
    bin.install "proc"
  end

  test do
    assert_match "proc", shell_output("#{bin}/proc --version")
  end
end
