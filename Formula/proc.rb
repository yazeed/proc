class Proc < Formula
  desc "Semantic process management CLI"
  homepage "https://github.com/yazeed/proc"
  version "1.2.0"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/yazeed/proc/releases/download/v#{version}/proc-darwin-aarch64.tar.gz"
      sha256 "9a1dae3e91051330b39f187066ab0a0fb9820f7d38540fec63343b76d36c286e"
    else
      url "https://github.com/yazeed/proc/releases/download/v#{version}/proc-darwin-x86_64.tar.gz"
      sha256 "d9c46d2bc9b3a031c4f495498c4e1c4301237346c9f38f45c07064d8b1f5d805"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/yazeed/proc/releases/download/v#{version}/proc-linux-aarch64.tar.gz"
      sha256 "61a99f7339d5a2929fa19b78e80f769353817c05d580f8143e7faee93dfc3923"
    else
      url "https://github.com/yazeed/proc/releases/download/v#{version}/proc-linux-x86_64.tar.gz"
      sha256 "e0c95955bc0e900372aef6b2866e7dcb941387428d79b384389da7756cc85bf7"
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
