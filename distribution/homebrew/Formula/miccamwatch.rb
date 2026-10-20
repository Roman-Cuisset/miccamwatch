class Miccamwatch < Formula
  desc "Observe microphone and camera access with a CLI and native menu bar"
  homepage "https://github.com/Roman-Cuisset/miccamwatch"
  url "https://codeload.github.com/Roman-Cuisset/miccamwatch/tar.gz/3dd70fc6e659187f6f4b9f74c9b26773257291b9"
  version "0.19.1"
  sha256 "b0447571ae5dfe037c6af083d1c97ebf7c6d9701c216da550f33eef57799763a"
  license "MIT"

  depends_on "rust" => :build
  depends_on :xcode => ["16.0", :build]
  depends_on :macos => :sequoia

  def install
    ENV["MACOSX_DEPLOYMENT_TARGET"] = "15.0"
    # std_cargo_args includes --locked. build.rs compiles and embeds the native
    # Swift/AppKit helper for this architecture; there is no runtime Swift package.
    system "cargo", "install", *std_cargo_args, "--bin", "mcw"
  end

  def post_install
    unless quiet_system(opt_bin/"mcw", "autostart", "refresh")
      opoo "Autostart was not refreshed; existing registration was preserved."
      opoo "Run `mcw autostart refresh` to inspect the ownership/session refusal."
    end
  end

  def caveats
    <<~EOS
      Homebrew owns this installation: upgrade with `brew upgrade miccamwatch`,
      not `mcw update`. No public-installer receipt is created.

      Installation does not enable autostart or launch the native menu bar.
      Use `mcw tray run` to launch it, and `mcw autostart enable` only when you
      explicitly want a login LaunchAgent. Before uninstalling, stop your owned
      menu bar with `mcw tray stop`. To remove login intent as well, explicitly
      run `mcw autostart disable` before `brew uninstall miccamwatch`.
      Uninstall preserves preferences, activity history, and login intent.

      The helper is locally ad-hoc sealed during its upstream build; this is
      neither Apple Developer ID signing nor notarization. Gatekeeper and
      quarantine are not modified by this formula.
    EOS
  end

  test do
    assert_equal "mcw #{version}", shell_output("#{bin}/mcw --version").strip
    # An authentic brew keg has no curl ownership receipt: self-update must
    # refuse locally, before any release request can be made.
    before = Digest::SHA256.file(bin/"mcw").hexdigest
    ENV["HTTPS_PROXY"] = "http://127.0.0.1:9"
    ENV["HTTP_PROXY"] = "http://127.0.0.1:9"
    shell_output("#{bin}/mcw update 2>&1", 2)
    assert_equal before, Digest::SHA256.file(bin/"mcw").hexdigest
    refute_path_exists prefix/".miccamwatch-install"
  end
end
