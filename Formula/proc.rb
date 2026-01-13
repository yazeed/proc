class Proc < Formula
  desc "Semantic process management CLI"
  homepage "https://github.com/yazeed/proc"
  version "1.0.1"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/yazeed/proc/releases/download/v#{version}/proc-darwin-aarch64.tar.gz"
      sha256 "f445aad5387c5f97e28d707d5f4862c2309cceda13eeb0e74da9bc0e8477bdfd"
    else
      url "https://github.com/yazeed/proc/releases/download/v#{version}/proc-darwin-x86_64.tar.gz"
      sha256 "e62b847770d703671b83cbc3bf05a55268d87eec8a8ca31d87fe7adf1bd29742"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/yazeed/proc/releases/download/v#{version}/proc-linux-aarch64.tar.gz"
      sha256 "b83f300ef5695c7eed5fa60e6b6c53cbfea046e4aa9f9af7c8de580244425dd4"
    else
      url "https://github.com/yazeed/proc/releases/download/v#{version}/proc-linux-x86_64.tar.gz"
      sha256 "fa0363c245d7223ef8a5d4ca46a5cec8b04214422acb02f30ba51a2d2647e5d4"
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
