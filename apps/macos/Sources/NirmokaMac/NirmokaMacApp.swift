import SwiftUI

@main
struct NirmokaMacApp: App {
  @StateObject private var model = AppModel()

  var body: some Scene {
    WindowGroup {
      ContentView()
        .environmentObject(model)
        .frame(minWidth: 900, minHeight: 600)
    }
    .commands {
      CommandGroup(replacing: .newItem) {
        Button("Choose Folder to Scan…") {
          NotificationCenter.default.post(name: .chooseScanFolder, object: nil)
        }
        .keyboardShortcut("o", modifiers: .command)
      }
      CommandMenu("Scan") {
        Button("Scan Home Folder") {
          NotificationCenter.default.post(name: .scanHomeFolder, object: nil)
        }
        .keyboardShortcut("r", modifiers: .command)
      }
    }
  }
}

extension Notification.Name {
  static let chooseScanFolder = Notification.Name("nirmoka.chooseScanFolder")
  static let scanHomeFolder = Notification.Name("nirmoka.scanHomeFolder")
}
