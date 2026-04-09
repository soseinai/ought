class Ought < Formula
  desc "Behavioral test framework powered by LLMs"
  homepage "https://sosein.ai/products/ought"
  url "https://github.com/soseinai/ought/archive/refs/tags/v0.1.0.tar.gz"
  # sha256 will be filled in when the release is created
  sha256 "PLACEHOLDER"
  license any_of: ["MIT", "Apache-2.0"]

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args(path: "crates/ought-cli")
  end

  test do
    assert_match "ought", shell_output("#{bin}/ought --help")
  end
end
