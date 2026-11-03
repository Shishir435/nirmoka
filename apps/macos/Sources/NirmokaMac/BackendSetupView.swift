import AppKit
import SwiftUI

struct BackendSetupView: View {
  @EnvironmentObject private var model: AppModel
  var scanning = true
  @State private var copied = false

  private var command: String { scanning ? "brew install ncdu" : "brew install mole" }
  private var relevant: [Backend] {
    model.backends.filter {
      scanning
        ? $0.capabilities.scan : ($0.capabilities.cleanupCategories || $0.capabilities.appInventory)
    }
  }

  var body: some View {
    VStack(alignment: .leading, spacing: 14) {
      Label(
        model.isCheckingBackends
          ? "Checking available tools…"
          : scanning ? "Set up scanning" : "Set up cleanup and app management",
        systemImage: "wrench.and.screwdriver"
      )
      .font(.headline)
      Text(
        scanning
          ? "Nirmoka uses an installed disk tool to measure your folders. Scanning only reads files."
          : "Install Mole to review cleanup candidates and application removals. Nothing is removed during setup."
      )
      .font(.callout).foregroundStyle(.secondary)
      if !model.isCheckingBackends {
        Text("If Homebrew is installed, open Terminal and run:").font(.callout)
        HStack {
          Text(command).font(.body.monospaced()).textSelection(.enabled)
          Spacer()
          Button(copied ? "Copied" : "Copy command") {
            NSPasteboard.general.clearContents()
            NSPasteboard.general.setString(command, forType: .string)
            copied = true
          }
        }
        Text(
          "Then return here and choose Check again. If a tool is already installed, its version may be unsupported; details appear below."
        )
        .font(.caption).foregroundStyle(.secondary)
        ForEach(relevant) { backend in
          if !backend.usable {
            Text(
              "\(backend.displayName): \(backend.error ?? "Required version: \(backend.supportedVersions)")"
            )
            .font(.caption).foregroundStyle(.secondary).textSelection(.enabled)
          }
        }
        HStack {
          Link("Homebrew setup instructions", destination: URL(string: "https://brew.sh")!)
          Spacer()
          Button("Check again") { model.refreshBackends() }
        }
      } else {
        ProgressView().controlSize(.small)
      }
    }
    .padding(22)
    .background(NirmokaPalette.card, in: RoundedRectangle(cornerRadius: 14))
  }
}
