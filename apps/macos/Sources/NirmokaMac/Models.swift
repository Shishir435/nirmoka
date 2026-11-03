import Foundation

struct Backend: Decodable, Identifiable {
  let id: String
  let displayName: String
  let supportedVersions: String
  let usable: Bool
  let capabilities: Capabilities
  let error: String?
}

struct Capabilities: Decodable {
  let scan: Bool
  let cleanupCategories: Bool
  let appInventory: Bool
  let uninstallApps: Bool
  let optimize: Bool
}

struct BackendSelection: Decodable {
  let chosen: String?
  let scanner: String?
  let scannerInsteadOf: String?
  let persistent: Bool
  let saveError: String?
}

struct ActiveScan: Decodable {
  let scanId: UInt64
  let rootPath: String
}

struct ScanStatus: Decodable {
  let active: ActiveScan?
  let progress: ScanProgress?
  let summary: ScanSummary?
}

struct ScanProgress: Decodable {
  let scanned: UInt64
  let currentPath: String
}

struct ScanSummary: Decodable {
  let scanId: UInt64
  let rootId: UInt32
  let rootPath: String
  let totalBytes: UInt64
  let entries: UInt64
  let directories: UInt64
  let backendId: String
  let backendVersion: String?
  let readErrors: UInt64
  let excluded: UInt64
}

struct ScanFailure: Decodable {
  let message: String
  let cancelled: Bool
}

struct DiskRow: Decodable, Identifiable {
  let id: UInt32
  let name: String
  let kind: NodeKind
  let ownBytes: UInt64
  let apparentBytes: UInt64
  let totalBytes: UInt64
  let readError: Bool
  let hardlink: Bool
  let excluded: Bool
  let childCount: UInt32
  let share: Double
}

enum NodeKind: String, Decodable {
  case directory
  case file
  case symlink
  case other
}

enum SortOrder: String, Codable {
  case largestFirst
  case smallestFirst
  case nameAscending
  case nameDescending
}

struct Crumb: Decodable, Identifiable {
  let id: UInt32
  let name: String
}

struct RowPage: Decodable {
  let scanId: UInt64
  let parentId: UInt32
  let name: String
  let path: String
  let ancestors: [Crumb]
  let readError: Bool
  let sort: SortOrder
  let offset: UInt32
  let total: UInt32
  let rows: [DiskRow]
}

struct VolumeInfo: Decodable {
  let name: String
  let mountPoint: String
  let totalBytes: UInt64
  let usedBytes: UInt64
  let freeBytes: UInt64
}

struct CategoryBreakdown: Decodable {
  let scanId: UInt64
  let rootPath: String
  let scannedBytes: UInt64
  let volume: VolumeInfo?
  let categories: [CategorySummary]
}

struct AppFootprint: Decodable {
  let scanId: UInt64
  let nodeId: UInt32
  let name: String
  let path: String
  let bundleId: String?
  let totalBytes: UInt64
  let relatedBytes: UInt64
  let unmeasuredPaths: UInt32
  let lastUsedMs: Int64?
  let components: [StorageComponent]
}

struct StorageComponent: Decodable, Identifiable {
  var id: String { label }
  let label: String
  let totalBytes: UInt64
  let complete: Bool
  let certain: Bool
  let paths: [FootprintPath]
}

struct FootprintPath: Decodable, Identifiable {
  var id: String { path }
  let path: String
  let totalBytes: UInt64?
  let complete: Bool
  let source: FootprintSource
}

enum FootprintSource: String, Decodable {
  case scan
  case filesystem
  case unavailable
}

struct CategorySummary: Decodable, Identifiable {
  var id: String { category.rawValue }
  let category: StorageCategory
  let totalBytes: UInt64
  let share: Double
  let consumers: [CategoryConsumer]
}

struct CategoryConsumer: Decodable, Identifiable {
  let id: UInt32
  let name: String
  let path: String
  let isDir: Bool
  let parentId: UInt32?
  let totalBytes: UInt64
  let sizeIsPartial: Bool
}

enum StorageCategory: String, Decodable, CaseIterable, Identifiable {
  case apps
  case personalFiles
  case development
  case system
  case other

  var id: String { rawValue }

  var title: String {
    switch self {
    case .apps: "Applications"
    case .personalFiles: "Personal files"
    case .development: "Development"
    case .system: "System"
    case .other: "Other"
    }
  }
}

struct NativeEvent: Decodable {
  let event: String
  let payload: Data
}

struct EmptyResponse: Decodable {}

struct TrashOperation: Decodable, Identifiable {
  let id: UInt64
  let targetPath: String
  let totalBytes: UInt64
  let trashedAtMs: UInt64
  let logError: String?
}

struct TrashPreparation: Decodable {
  let confirmationToken: UInt64
  let targetPath: String
  let totalBytes: UInt64
  let isDirectory: Bool
  let warning: String
}

struct DeleteOperation: Decodable, Identifiable {
  let id: UInt64
  let backend: String
  let targetPath: String
  let disposition: String
  let recoverable: Bool
  let undone: Bool
  let deletedAtMs: UInt64
  let undoneAtMs: UInt64?
  let logError: String?
}

struct InstalledApplicationInventory: Decodable {
  let backend: String
  let backendInsteadOf: String?
  let total: UInt32
  let rows: [InstalledApplication]
}

struct InstalledApplication: Decodable, Identifiable {
  var id: String { uninstallName }
  let name: String
  let bundleId: String
  let source: String
  let uninstallName: String
  let path: String
  let reportedSize: String
}

struct CleanupPreview: Decodable {
  let backend: String
  let backendVersion: String
  let generatedAt: String
  let categories: [CleanupCategory]
  let potentialCleanup: String?
  let totalItems: UInt64
  let systemScope: CleanupSystemScope
  let warnings: [String]
}

struct CleanupCategory: Decodable, Identifiable {
  var id: String { name }
  let name: String
  let items: [CleanupItem]
}

struct CleanupItem: Decodable, Identifiable {
  var id: String { path }
  let path: String
  let reportedSize: String?
  let itemCount: UInt64
}

enum CleanupSystemScope: String, Decodable {
  case included
  case userOnly
  case unknown
}

struct CleanupProgress: Decodable, Identifiable {
  enum Kind: String, Decodable {
    case category
    case item
    case categoryTotal
  }

  var id: String { "\(kind.rawValue)-\(text)" }
  let kind: Kind
  let text: String
}

struct CleanupPreparation: Decodable {
  let confirmationToken: UInt64
  let backend: String
  let backendVersion: String
  let previewGeneratedAt: String
  let potentialCleanup: String?
  let totalItems: UInt64
  let systemScope: CleanupSystemScope
  let warnings: [String]
  let expiresInSeconds: UInt64
  let requiresConfirmation: Bool
  let warning: String
}

enum CleanupCompletion: String, Decodable {
  case finished
  case partial
  case cancelled
  case failed
}

struct CleanupOperation: Decodable, Identifiable {
  let id: UInt64
  let backend: String
  let backendVersion: String
  let previewGeneratedAt: String
  let reviewedItems: UInt64
  let reviewedPotentialCleanup: String?
  let systemScope: CleanupSystemScope
  let completion: CleanupCompletion
  let warnings: [String]
  let executedAtMs: UInt64
  let logError: String?
}

struct OptimizePreview: Decodable {
  let backendVersion: String
  let actionCount: UInt64
  let sections: [OptimizeSection]
  let transcript: String
}

struct OptimizeSection: Decodable, Identifiable {
  var id: String { name }
  let name: String
  let items: [OptimizeItem]
}

struct OptimizeItem: Decodable, Identifiable {
  var id: String { "\(kind.rawValue)-\(text)" }
  let kind: OptimizeItemKind
  let text: String
}

enum OptimizeItemKind: String, Decodable {
  case action
  case complete
  case review
  case note
}

struct OptimizePreparation: Decodable {
  let confirmationToken: UInt64
  let backend: String
  let backendVersion: String
  let expiresInSeconds: UInt64
  let warning: String
}

enum OptimizeCompletion: String, Decodable {
  case finished
  case cancelled
  case failed
}

struct OptimizeOperation: Decodable, Identifiable {
  var id: UInt64 { executedAtMs }
  let backend: String
  let backendVersion: String
  let completion: OptimizeCompletion
  let transcript: String
  let executedAtMs: UInt64
}

struct UninstallPreview: Decodable {
  let backend: String
  let backendVersion: String
  let requested: [String]
  let apps: [UninstallApp]
  let reportedTotal: String?
  let totalItems: UInt64
  let hasReviewOnlyItems: Bool
  let warnings: [String]
  let notes: [String]
  let transcript: String
}

struct UninstallApp: Decodable, Identifiable {
  var id: String { name }
  let name: String
  let homebrewCask: Bool
  let reportedSize: String?
  let items: [UninstallItem]
}

struct UninstallItem: Decodable, Identifiable {
  var id: String { "\(scope.rawValue)-\(displayPath)" }
  let displayPath: String
  let reportedSize: String?
  let scope: Scope

  enum Scope: String, Decodable {
    case removed
    case system
    case reviewOnly
  }
}

struct UninstallPreparation: Decodable {
  let confirmationToken: UInt64
  let backend: String
  let backendVersion: String
  let applications: [String]
  let reportedTotal: String?
  let totalItems: UInt64
  let hasReviewOnlyItems: Bool
  let warnings: [String]
  let expiresInSeconds: UInt64
  let requiresConfirmation: Bool
  let warning: String
}

enum UninstallCompletion: String, Decodable {
  case finished
  case partial
  case cancelled
  case failed
}

struct UninstallOperation: Decodable, Identifiable {
  let id: UInt64
  let backend: String
  let backendVersion: String
  let reviewedApplications: [String]
  let reviewedItems: UInt64
  let reviewedTotal: String?
  let completion: UninstallCompletion
  let removed: [String]
  let failed: [String]
  let reportedFreed: String?
  let warnings: [String]
  let executedAtMs: UInt64
  let logError: String?
}

struct SystemStatus: Decodable {
  let collectedAt: String
  let host: String
  let platform: String
  let uptime: String
  let healthScore: UInt8
  let healthScoreMessage: String
  let hardware: HardwareStatus
  let cpu: CpuStatus
  let memory: MemoryStatus
  let disks: [DiskStatus]
  let batteries: [BatteryStatus]
  let thermal: ThermalStatus
}

struct HardwareStatus: Decodable {
  let model: String
  let cpuModel: String
  let totalRam: String
  let diskSize: String
  let osVersion: String
}

struct CpuStatus: Decodable {
  let usage: Double
  let load1: Double
  let load5: Double
  let load15: Double
  let coreCount: UInt32
  let logicalCpu: UInt32
}

struct MemoryStatus: Decodable {
  let used: UInt64
  let total: UInt64
  let available: UInt64
  let usedPercent: Double
  let swapUsed: UInt64
  let swapTotal: UInt64
  let pressure: String
}

struct DiskStatus: Decodable, Identifiable {
  var id: String { mount }
  let mount: String
  let device: String
  let used: UInt64
  let total: UInt64
  let usedPercent: Double
  let filesystem: String
  let external: Bool
  let smartStatus: String
}

struct BatteryStatus: Decodable, Identifiable {
  var id: String { status }
  let percent: Double
  let status: String
  let timeLeft: String
  let health: String
  let cycleCount: UInt32
  let capacity: Double
}

struct ThermalStatus: Decodable {
  let cpuTemp: Double?
  let gpuTemp: Double?
  let batteryTemp: Double?
  let fanSpeed: Double?
  let fanCount: UInt32?
}
