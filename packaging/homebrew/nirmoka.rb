# The published copy of this file lives in the tap repository; this is the
# source of truth it is copied from. `version` and `sha256` are the only lines
# that change per release — the release workflow prints both.
#
# A formula rather than a cask, deliberately. Homebrew removed `--no-quarantine`
# (Homebrew/brew#23363), so every cask install quarantines its download and an
# unsigned .app is then refused by Gatekeeper as "damaged". A formula compiles
# on the user's machine, which is never quarantined and launches. See
# docs/adr/0024-distribution-is-a-source-built-homebrew-formula.md.
class Nirmoka < Formula
  desc "Desktop GUI for disk analysis and cleanup"
  homepage "https://github.com/Shishir435/nirmoka"
  url "https://github.com/Shishir435/nirmoka/archive/refs/tags/v0.1.0.tar.gz"
  sha256 "0000000000000000000000000000000000000000000000000000000000000000"
  license "Apache-2.0"
  head "https://github.com/Shishir435/nirmoka.git", branch: "main"

  depends_on "node" => :build
  depends_on "pnpm" => :build
  depends_on "rust" => :build
  # Mole is macOS-only and so is the packaged app — see ADR 0023.
  depends_on :macos

  # Scanning is the floor. Without a backend the window can do nothing but
  # explain that it has no backend, so the baseline one comes along.
  depends_on "ncdu"

  def install
    # Homebrew's build sandbox refuses writes outside the build directory, and
    # every one of these defaults to somewhere under $HOME. pnpm in particular
    # honours the `packageManager` pin by downloading that exact version into
    # $PNPM_HOME/.tools, whichever version brew happens to have installed.
    ENV["CARGO_HOME"] = buildpath/".cargo"
    ENV["PNPM_HOME"] = buildpath/".pnpm"
    ENV["npm_config_store_dir"] = buildpath/".pnpm-store"

    system "pnpm", "install", "--frozen-lockfile"
    # `--bundles app` skips the .dmg: this is an install, not a download.
    system "pnpm", "tauri", "build", "--bundles", "app"

    prefix.install "target/release/bundle/macos/Nirmoka.app"
    # The bundle executable keeps the crate's name, not the product's. Launching
    # it directly is correct: it sits inside the bundle, so it picks up the
    # Info.plist identity and appears in the Dock as Nirmoka.
    bin.install_symlink prefix/"Nirmoka.app/Contents/MacOS/nirmoka-app" => "nirmoka"
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
    app = prefix/"Nirmoka.app/Contents/MacOS/nirmoka-app"
    assert_predicate app, :executable?
    # The app is a GUI and cannot be launched headless, so this asserts the
    # bundle is the architecture that was asked for rather than that it runs.
    assert_match "Mach-O", shell_output("file #{app}")
  end
end
