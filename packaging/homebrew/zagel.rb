class Zagel < Formula
  desc "Desktop REST workbench with .http/.env collections and persisted state."
  homepage "https://github.com/sharno/zagel"
  version "0.3.0"
  license "MIT OR Apache-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/sharno/zagel/releases/download/v0.3.0/zagel-v0.3.0-aarch64-apple-darwin.tar.gz"
      sha256 "7388bf2130dd2ce804303f76e3a417e885dd98a287fbf3224add51d9b0dcb313"
    end

    if Hardware::CPU.intel?
      url "https://github.com/sharno/zagel/releases/download/v0.3.0/zagel-v0.3.0-x86_64-apple-darwin.tar.gz"
      sha256 "2e4aa1706847fa6eb2a36cde358107bdd7c39e39dacf1657b882382a20070759"
    end
  end

  on_linux do
    url "https://github.com/sharno/zagel/releases/download/v0.3.0/zagel-v0.3.0-x86_64-unknown-linux-gnu.tar.gz"
    sha256 "34825a3308d806661e684f76327edfdacf6dbdaf8bb7e768072c5a820d628b5e"
  end

  def install
    bin.install "zagel"
  end

  test do
    assert_predicate bin/"zagel", :exist?
  end
end
