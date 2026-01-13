class Proc < Formula
  desc "Semantic process management CLI"
  homepage "https://github.com/yazeed/proc"
  version "1.0.2"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/yazeed/proc/releases/download/v#{version}/proc-darwin-aarch64.tar.gz"
      sha256 "da49fa03db5ebe8ceee13d98c188d9dcd12413ac95800e9eea881e6c77c1f9fa"
    else
      url "https://github.com/yazeed/proc/releases/download/v#{version}/proc-darwin-x86_64.tar.gz"
      sha256 "2a30008a6314ef0c24df3009a998ba135e5018289ccfb0db9f5ad60d13e231f0"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/yazeed/proc/releases/download/v#{version}/proc-linux-aarch64.tar.gz"
      sha256 "649d3cce6167b0139099c65d439311482f140c786dc23396143201c0a679890c"
    else
      url "https://github.com/yazeed/proc/releases/download/v#{version}/proc-linux-x86_64.tar.gz"
      sha256 "e0f9b43b5112068e3aea935a74005fa8fc3f8185e22b05577a04d28d34d83370"
    end
  end

  def install
    # Binary is named proc-{platform}-{arch}, rename to proc
    binary = Dir["proc-*"].first
    bin.install binary => "proc"
  end

  test do
    assert_match "proc", shell_output("#{bin}/proc --version")
  end
end
