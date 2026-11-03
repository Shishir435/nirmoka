# The published copy of this file lives in the tap repository; this is the
# source of truth it is copied from. The `url` tag and the `sha256` are the only
# lines that differ, and the release workflow rewrites both on its way to the
# tap — see docs/releasing.md.
#
# The sha256 below is a placeholder and stays one. It is the hash of the source
# tarball GitHub generates for a tag, so it cannot be known until the tag exists,
# and a stale real-looking hash here would be worse than an obvious placeholder.
# The `url` tag, however, must match apps/macos/Info.plist:
# scripts/check-invariants.sh fails when it does not.
#
# A formula rather than a cask, deliberately. Homebrew removed `--no-quarantine`
# (Homebrew/brew#23363), so every cask install quarantines its download and an
# unsigned .app is then refused by Gatekeeper as "damaged". A formula compiles
# on the user's machine, which is never quarantined and launches. See
# docs/adr/0024-distribution-is-a-source-built-homebrew-formula.md.
class Nirmoka < Formula
  desc "Desktop GUI for disk analysis and cleanup"
  homepage "https://github.com/Shishir435/nirmoka"
  url "https://github.com/Shishir435/nirmoka/archive/refs/tags/v0.2.0.tar.gz"
  sha256 "0000000000000000000000000000000000000000000000000000000000000000"
  license "Apache-2.0"
  head "https://github.com/Shishir435/nirmoka.git", branch: "main"

  depends_on xcode: ["16.0", :build]
  depends_on macos: :ventura
  depends_on "rust" => :build
  # Mole is macOS-only and so is the packaged app — see ADR 0023.

  # Scanning is the floor. Without a backend the window can do nothing but
  # explain that it has no backend, so the baseline one comes along.
  depends_on "ncdu"

  def install
    # Keep compiler caches in Homebrew’s build directory.
    ENV["CARGO_HOME"] = buildpath/".cargo"
    ENV["CLANG_MODULE_CACHE_PATH"] = buildpath/".clang-cache"
    ENV["SWIFTPM_MODULECACHE_OVERRIDE"] = buildpath/".swift-cache"

    system "apps/macos/build-app.sh"

    prefix.install "target/macos/Nirmoka.app"
    # The executable lives inside the native application bundle.
    bin.install_symlink prefix/"Nirmoka.app/Contents/MacOS/Nirmoka" => "nirmoka"
  end

  def caveats
    <<~EOS
      ncdu came with this formula, which covers scanning. The Clean page needs
      Mole, which is macOS-only and optional:
        brew install mole

      Run `nirmoka` from the terminal, or put it in Launchpad and Spotlight:
        ln -sfn #{opt_prefix}/Nirmoka.app /Applications/Nirmoka.app
    EOS
  end

  test do
    app = prefix/"Nirmoka.app/Contents/MacOS/Nirmoka"
    assert_predicate app, :executable?
    # The app is a GUI and cannot be launched headless, so this asserts the
    # bundle is the architecture that was asked for rather than that it runs.
    assert_match "Mach-O", shell_output("file #{app}")
  end
end
