# Genuine previous public source, installed under the same formula identity so
# brew itself performs the upgrade. This is only a native qualification fixture.
class Miccamwatch < Formula
  desc "Observe microphone and camera access with a CLI and native menu bar"
  homepage "https://github.com/Roman-Cuisset/miccamwatch"
  url "https://codeload.github.com/Roman-Cuisset/miccamwatch/tar.gz/7b134e3500860bd185ddc3955107b8082d414e78"
  version "0.16.1"
  sha256 "2e8d65a16d48e5b70d8c0d8869eee5a051d7f8f2bd7c8ffda4c583f792ff4c4c"
  license "MIT"

  depends_on "rust" => :build
  depends_on :xcode => ["16.0", :build]
  depends_on :macos => :sequoia

  def install
    ENV["MACOSX_DEPLOYMENT_TARGET"] = "15.0"
    system "cargo", "install", *std_cargo_args, "--bin", "mcw"
  end

  test do
    assert_equal "mcw #{version}", shell_output("#{bin}/mcw --version").strip
  end
end
