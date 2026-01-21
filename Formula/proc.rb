class Proc < Formula
  desc "Semantic process management CLI"
  homepage "https://github.com/yazeed/proc"
  version "1.2.2"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/yazeed/proc/releases/download/v#{version}/proc-darwin-aarch64.tar.gz"
      sha256 "77a23e9cac5aaeab34c43d1042e0f257708f1f99cc3934b8b9f7c3ec4932c9d5"
    else
      url "https://github.com/yazeed/proc/releases/download/v#{version}/proc-darwin-x86_64.tar.gz"
      sha256 "612a4e4a3481d1cfb7bee5eeb1cd09b218d334f7130eadf2a111ccabe3df3dbd"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/yazeed/proc/releases/download/v#{version}/proc-linux-aarch64.tar.gz"
      sha256 "a2725914a4ce328e5523f595477f059e13ca8f562de9b45b9c3e064643b04122"
    else
      url "https://github.com/yazeed/proc/releases/download/v#{version}/proc-linux-x86_64.tar.gz"
      sha256 "9bfa25df7983b3af8df137fd34ded2dd35240da2c017a284a25d289103d4010f"
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
