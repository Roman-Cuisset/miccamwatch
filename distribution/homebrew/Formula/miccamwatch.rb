class Miccamwatch < Formula
  desc "Observe microphone and camera access with a CLI and native menu bar"
  homepage "https://github.com/Roman-Cuisset/miccamwatch"
  url "https://codeload.github.com/Roman-Cuisset/miccamwatch/tar.gz/a26522028eafe02591f58c03ceb5c367f2d5d60b"
  version "0.20.0"
  sha256 "f3b7454d41ea623b3f12e835d9c92117f3e04e7b5f5a9cbf173f54a8b6ce7916"
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

  def caveats
    <<~EOS
      Homebrew owns this installation: upgrade with `brew upgrade miccamwatch`,
      not `mcw update`. No public-installer receipt is created.

      For an existing owned login LaunchAgent, including dormant or disabled
      intent, run these commands as your ordinary user, without sudo:
        HOMEBREW_NO_INSTALL_CLEANUP=1 brew upgrade miccamwatch &&
        mcw autostart refresh &&
        brew cleanup miccamwatch
      Stop if upgrade or refresh fails; do not clean up the old keg until refresh
      succeeds. Homebrew's sandboxed install hooks cannot refresh user autostart.
      The explicit refresh preserves loaded, dormant, and disabled intent.
      Without an existing login registration, ordinary `brew upgrade miccamwatch`
      needs no refresh; refresh never enables an absent registration.

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
