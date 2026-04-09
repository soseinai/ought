class Ought < Formula
  desc "Behavioral test framework powered by LLMs"
  homepage "https://sosein.ai/products/ought"
  url "https://github.com/soseinai/ought/archive/refs/tags/v0.2.0-rc1.tar.gz"
  # sha256 will be filled in when the release is created
  sha256 "026b8d85f724faeec022f2cdcff187efac553f4eb95a9ca44b95fd8aad2a8a11"
  license any_of: ["MIT", "Apache-2.0"]

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args(path: "crates/ought-cli")
  end

  test do
    assert_match "ought", shell_output("#{bin}/ought --help")
  end
end
