import AppKit
import SwiftUI

private enum MainSection {
  case overview
  case explore
  case appInspector
  case clean
  case applications
  case optimize
  case status
  case activity

  var isHome: Bool { self == .overview }
}

private enum AuxiliarySheet: String, Identifiable {
  case settings
  case help

  var id: String { rawValue }
}

@MainActor
private final class ContentNavigationModel: ObservableObject {
  @Published var selection: MainSection? = .overview
  @Published var history: [MainSection] = []
  @Published var presentedSheet: AuxiliarySheet?
}

struct ContentView: View {
  @EnvironmentObject private var model: AppModel
  @StateObject private var navigation = ContentNavigationModel()

  var body: some View {
    VStack(spacing: 0) {
      if model.isScanning {
        HStack(spacing: 12) {
          ProgressView().controlSize(.small)
          Text(
            model.scanProgress.map { "Scanning · \(NirmokaFormat.count($0.scanned)) items" }
              ?? "Starting scan…")
          Text(model.scanProgress?.currentPath ?? "").lineLimit(1).truncationMode(.middle)
            .foregroundStyle(.secondary)
          Spacer()
          Button("Stop") { model.cancelScan() }
        }
        .font(.caption).padding(12).background(.bar)
      }
      if let notice = model.scanNotice {
        Text(notice).font(.caption).foregroundStyle(.secondary).padding(8)
      }
      page(for: navigation.selection ?? .overview)
    }
    .frame(maxWidth: .infinity, maxHeight: .infinity)
    .background(NirmokaPalette.canvas)
    .frame(minWidth: 900, minHeight: 600)
    .background(NirmokaPalette.canvas)
    .navigationTitle("Nirmoka")
    .toolbar {
      ToolbarItem(placement: .navigation) {
        if (navigation.selection ?? .overview).isHome {
          Label("Nirmoka", systemImage: "externaldrive.fill")
            .font(.headline)
        } else {
          Button {
            navigation.selection = navigation.history.popLast() ?? .overview
          } label: {
            Label("Back", systemImage: "chevron.left")
          }
          .help("Return to the previous screen")
        }
      }

      ToolbarItemGroup(placement: .primaryAction) {
        Menu {
          Button("Storage overview") { show(.overview) }
          Button("Browse scanned files") {
            model.openRootBrowser()
            show(.explore)
          }
          .disabled(model.scanSummary == nil || model.isScanning)
          Button("Applications") { show(.applications) }
          Button("Review cleanup") { show(.clean) }
          Divider()
          Button("Activity") { show(.activity) }
          Button("System status") { show(.status) }
          Button("Maintenance") { show(.optimize) }
        } label: {
          Label("Go to", systemImage: "square.grid.2x2")
        }

        Menu {
          Button("Scan Home Folder", systemImage: "house.magnifyingglass") {
            model.scanHome()
          }
          .disabled(!model.canScan)
          Button("Scan Applications", systemImage: "app") {
            model.scan(path: "/Applications")
          }
          .disabled(!model.canScan)
          Button("Choose Folder…", systemImage: "folder.badge.plus") {
            chooseFolder()
          }
          .disabled(!model.canScan)
          if model.isScanning {
            Divider()
            Button("Stop Scan", systemImage: "stop.circle", role: .cancel) {
              model.cancelScan()
            }
          }
        } label: {
          Label(model.isScanning ? "Scanning…" : "Scan", systemImage: "magnifyingglass")
        }
        .help("Choose what to scan")

        Button {
          navigation.presentedSheet = .settings
        } label: {
          Label("Settings", systemImage: "gearshape")
        }
        .help("Tool settings")

        Button {
          navigation.presentedSheet = .help
        } label: {
          Label("Help", systemImage: "questionmark.circle")
        }
        .help("How Nirmoka handles storage")
      }
    }
    .sheet(item: $navigation.presentedSheet) { sheet in
      switch sheet {
      case .settings:
        SettingsView(
          openActivity: { show(.activity) },
          openStatus: { show(.status) },
          openOptimize: { show(.optimize) }
        )
        .environmentObject(model)
        .frame(minWidth: 480, minHeight: 420)
      case .help:
        HelpView()
          .frame(width: 500, height: 360)
      }
    }
    .confirmationDialog(
      model.trashPreparation?.isDirectory == true
        ? "Move this folder to the Trash?"
        : "Move this item to the Trash?",
      isPresented: Binding(
        get: { model.isTrashConfirmationPresented },
        set: { if !$0 { model.cancelTrashConfirmation() } }
      ),
      titleVisibility: .visible,
      presenting: model.trashPreparation
    ) { _ in
      Button("Move to Trash", role: .destructive) { model.confirmTrash() }
      Button("Cancel", role: .cancel) { model.cancelTrashConfirmation() }
    } message: { preparation in
      Text(
        "\(preparation.warning)\n\n\(preparation.targetPath)\n\(NirmokaFormat.bytes(preparation.totalBytes))"
      )
    }
    .alert(
      "Nirmoka couldn’t complete that action",
      isPresented: Binding(
        get: { model.errorMessage != nil },
        set: { if !$0 { model.errorMessage = nil } }
      )
    ) {
      Button("OK", role: .cancel) { model.errorMessage = nil }
    } message: {
      Text(model.errorMessage ?? "Please try again.")
    }
    .onReceive(NotificationCenter.default.publisher(for: .chooseScanFolder)) { _ in
      chooseFolder()
    }
    .onReceive(NotificationCenter.default.publisher(for: .scanHomeFolder)) { _ in
      model.scanHome()
    }
  }

  @ViewBuilder
  private func page(for section: MainSection) -> some View {
    switch section {
    case .overview:
      OverviewView(
        showExplorer: { show(.explore) },
        showClean: { show(.clean) },
        showApplications: { show(.applications) },
        showApplicationInspector: { show(.appInspector) },
        showActivity: { show(.activity) }
      )
    case .explore:
      DiskBrowserView()
    case .appInspector:
      AppInspectorView(showExplorer: { show(.explore) }, showApplications: { show(.applications) })
    case .clean:
      CleanView()
    case .applications:
      ApplicationsView(showInspector: { show(.appInspector) })
    case .optimize:
      OptimizeView()
    case .status:
      SystemStatusView()
    case .activity:
      ActivityView()
    }
  }

  private func show(_ section: MainSection) {
    navigation.presentedSheet = nil
    if navigation.selection != section {
      if let current = navigation.selection { navigation.history.append(current) }
      navigation.selection = section
    }
  }

  private func chooseFolder() {
    guard model.canScan else { return }
    let panel = NSOpenPanel()
    panel.title = "Choose a folder or volume to scan"
    panel.prompt = "Scan"
    panel.canChooseFiles = false
    panel.canChooseDirectories = true
    panel.allowsMultipleSelection = false
    panel.directoryURL = URL(fileURLWithPath: model.homePath)

    guard let window = NSApplication.shared.keyWindow else { return }
    panel.beginSheetModal(for: window) { response in
      guard response == .OK, let url = panel.url else { return }
      Task { @MainActor in
        show(.overview)
        model.scan(path: url.path)
      }
    }
  }
}

private struct SettingsView: View {
  @Environment(\.dismiss) private var dismiss
  @EnvironmentObject private var model: AppModel
  let openActivity: () -> Void
  let openStatus: () -> Void
  let openOptimize: () -> Void

  var body: some View {
    VStack(alignment: .leading, spacing: 20) {
      VStack(alignment: .leading, spacing: 5) {
        Text("Settings")
          .font(.title2.weight(.semibold))
        Text("Choose a preferred tool. Scanning falls back to an available scanner.")
          .font(.callout)
          .foregroundStyle(.secondary)
      }

      VStack(alignment: .leading, spacing: 8) {
        Text("PREFERRED TOOL")
          .font(.caption.weight(.bold))
          .tracking(1.1)
          .foregroundStyle(.secondary)
        backendRow(
          title: "Automatic",
          detail: automaticBackendDescription,
          selected: model.backendSelection?.chosen == nil
        ) {
          model.chooseBackend(nil)
        }
        ForEach(model.backends) { backend in
          backendRow(
            title: backend.displayName,
            detail: backend.usable ? backend.supportedVersions : (backend.error ?? "Unavailable"),
            selected: model.backendSelection?.chosen == backend.id,
            disabled: !backend.usable
          ) {
            model.chooseBackend(backend.id)
          }
        }
      }

      Divider()

      Button(model.isCheckingBackends ? "Checking…" : "Check installed tools again") {
        model.refreshBackends()
      }
      .disabled(model.isCheckingBackends)
      HStack {
        Spacer()
        Button("Done") { dismiss() }.keyboardShortcut(.defaultAction)
      }
      Spacer(minLength: 0)
    }
    .padding(26)
  }

  private var automaticBackendDescription: String {
    if let backend = model.backends.first(where: { $0.usable && $0.capabilities.cleanupCategories })
    {
      return "Uses \(backend.displayName) when available."
    }
    return "Uses the recommended backend when one is available."
  }

  private func backendRow(
    title: String,
    detail: String,
    selected: Bool,
    disabled: Bool = false,
    action: @escaping () -> Void
  ) -> some View {
    Button(action: action) {
      HStack(spacing: 12) {
        Image(systemName: selected ? "largecircle.fill.circle" : "circle")
          .foregroundStyle(selected ? NirmokaPalette.accentDeep : .secondary)
        VStack(alignment: .leading, spacing: 3) {
          Text(title).font(.body.weight(.medium))
          Text(detail).font(.caption).foregroundStyle(.secondary)
        }
        Spacer()
      }
      .contentShape(Rectangle())
    }
    .buttonStyle(.plain)
    .disabled(disabled)
    .opacity(disabled ? 0.55 : 1)
    .padding(9)
    .background(
      selected ? NirmokaPalette.accent.opacity(0.1) : NirmokaPalette.card,
      in: RoundedRectangle(cornerRadius: 9))
  }
}

private struct HelpView: View {
  @Environment(\.dismiss) private var dismiss
  var body: some View {
    VStack(alignment: .leading, spacing: 18) {
      Label("Understand your storage", systemImage: "externaldrive")
        .font(.title2.weight(.semibold))
      Text(
        "Start with a scan to see which folders and apps account for the space. A scan reports what it can read; it does not change files."
      )
      Text(
        "Large does not mean safe to remove. Nirmoka only runs cleanup after showing the backend’s own preview and asking you to confirm."
      )
      Label("Trash recovery", systemImage: "trash")
        .font(.headline)
        .padding(.top, 4)
      Text(
        "Items shown as moved to Trash can be restored there with Finder’s Put Back command. The cleanup and uninstall review explains the recovery options for those operations."
      )
      .foregroundStyle(.secondary)
      HStack {
        Spacer()
        Button("Done") { dismiss() }.keyboardShortcut(.defaultAction)
      }
    }
    .font(.callout)
    .padding(28)
  }
}

struct NirmokaEmptyState: View {
  let title: String
  let symbol: String
  let description: String

  var body: some View {
    VStack(spacing: 11) {
      Image(systemName: symbol)
        .font(.system(size: 30, weight: .regular))
        .foregroundStyle(NirmokaPalette.accentDeep)
      Text(title)
        .font(.headline)
        .foregroundStyle(NirmokaPalette.ink)
      Text(description)
        .font(.callout)
        .foregroundStyle(.secondary)
        .multilineTextAlignment(.center)
        .frame(maxWidth: 390)
    }
    .padding(28)
  }
}

enum NirmokaPalette {
  static let canvas = Color(nsColor: .windowBackgroundColor)
  static let card = Color(nsColor: .controlBackgroundColor)
  static let ink = Color.primary
  static let mutedInk = Color.secondary
  static let accent = Color(red: 0.24, green: 0.68, blue: 0.52)
  static let accentDeep = Color.accentColor
  static let line = Color.primary.opacity(0.08)
  static let heroTop = Color(red: 0.055, green: 0.16, blue: 0.15)
  static let heroBottom = Color(red: 0.075, green: 0.34, blue: 0.27)

  static func color(for category: StorageCategory) -> Color {
    switch category {
    case .apps: Color(red: 0.38, green: 0.63, blue: 0.91)
    case .personalFiles: Color(red: 0.91, green: 0.64, blue: 0.28)
    case .development: Color(red: 0.57, green: 0.48, blue: 0.87)
    case .system: Color(red: 0.52, green: 0.60, blue: 0.62)
    case .other: accent
    }
  }
}

enum NirmokaFormat {
  static func bytes(_ value: UInt64) -> String {
    guard value > 0 else { return "0 bytes" }
    return ByteCountFormatter.string(fromByteCount: Int64(clamping: value), countStyle: .file)
  }

  static func count(_ value: UInt64) -> String {
    value.formatted(.number)
  }

  static func items(_ value: UInt64) -> String {
    "\(count(value)) \(value == 1 ? "item" : "items")"
  }

  static func percent(_ share: Double) -> String {
    share.formatted(.percent.precision(.fractionLength(0)))
  }

  static func leaf(_ path: String) -> String {
    URL(fileURLWithPath: path).lastPathComponent.isEmpty
      ? path : URL(fileURLWithPath: path).lastPathComponent
  }
}
