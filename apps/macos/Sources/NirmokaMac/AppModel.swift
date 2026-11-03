import AppKit
import Combine
import Foundation

@MainActor
final class AppModel: ObservableObject {
  @Published private(set) var backends: [Backend] = []
  @Published private(set) var backendSelection: BackendSelection?
  @Published private(set) var scanSummary: ScanSummary?
  @Published private(set) var scanProgress: ScanProgress?
  @Published private(set) var isScanning = false
  @Published private(set) var volumeInfo: VolumeInfo?
  @Published private(set) var categories: CategoryBreakdown?
  @Published private(set) var dashboardCategoryFilter: StorageCategory?
  @Published private(set) var appFootprint: AppFootprint?
  @Published private(set) var isLoadingAppFootprint = false
  @Published private(set) var rowPage: RowPage?
  @Published private(set) var selectedRowID: UInt32?
  @Published private(set) var selectedConsumer: CategoryConsumer?
  @Published private(set) var currentParentID: UInt32?
  @Published private(set) var cleanupPreview: CleanupPreview?
  @Published private(set) var cleanupPreparation: CleanupPreparation?
  @Published private(set) var cleanupProgress: [CleanupProgress] = []
  @Published private(set) var trashOperations: [TrashOperation] = []
  @Published private(set) var trashPreparation: TrashPreparation?
  @Published private(set) var isPreparingTrash = false
  @Published private(set) var isMovingToTrash = false
  @Published private(set) var trashedRowIDs: Set<UInt32> = []
  @Published private(set) var isTrashConfirmationPresented = false
  @Published private(set) var trashStatusMessage: String?
  @Published private(set) var deleteOperations: [DeleteOperation] = []
  @Published private(set) var cleanupOperations: [CleanupOperation] = []
  @Published private(set) var cleanupResult: CleanupOperation?
  @Published private(set) var isLoadingCleanupPreview = false
  @Published private(set) var isExecutingCleanup = false
  @Published var isCleanupConfirmationPresented = false
  @Published private(set) var optimizePreview: OptimizePreview?
  @Published private(set) var optimizePreparation: OptimizePreparation?
  @Published private(set) var optimizeResult: OptimizeOperation?
  @Published private(set) var isLoadingOptimizePreview = false
  @Published private(set) var isExecutingOptimize = false
  @Published var isOptimizeConfirmationPresented = false
  @Published private(set) var applicationInventory: InstalledApplicationInventory?
  @Published private(set) var selectedApplications: Set<String> = []
  @Published private(set) var uninstallPreview: UninstallPreview?
  @Published private(set) var uninstallPreparation: UninstallPreparation?
  @Published private(set) var uninstallOperations: [UninstallOperation] = []
  @Published private(set) var uninstallResult: UninstallOperation?
  @Published private(set) var isLoadingApplications = false
  @Published private(set) var isLoadingUninstallPreview = false
  @Published private(set) var isExecutingUninstall = false
  @Published var isUninstallConfirmationPresented = false
  @Published private(set) var systemStatus: SystemStatus?
  @Published private(set) var isLoadingSystemStatus = false
  @Published private(set) var isLoadingActivity = false
  @Published var applicationSearch = ""
  @Published var errorMessage: String?

  @Published private(set) var isCheckingBackends = true
  @Published private(set) var browserSort: SortOrder = .largestFirst
  @Published private(set) var isLoadingRows = false
  @Published private(set) var inspectedApplication: InstalledApplication?
  @Published private(set) var scanNotice: String?
  @Published private(set) var browserHistory = BrowserHistory()
  private var footprintRequestVersion = 0
  private var uninstallRequestVersion = 0

  var scannerReady: Bool { backendSelection?.scanner != nil && !isCheckingBackends }
  var canScan: Bool {
    scannerReady && !isScanning && !isMovingToTrash && !isExecutingCleanup && !isExecutingUninstall
  }
  var canListApplications: Bool { backends.contains { $0.usable && $0.capabilities.appInventory } }
  var canUninstallApplications: Bool {
    backends.contains { $0.usable && $0.capabilities.uninstallApps }
  }
  var canClean: Bool { backends.contains { $0.usable && $0.capabilities.cleanupCategories } }

  private let client: NativeServiceClient?
  private var pollingTask: Task<Void, Never>?
  private var rowRequestVersion = 0
  private var trashRequestVersion = 0
  private var trashPendingRowID: UInt32?

  var homePath: String {
    FileManager.default.homeDirectoryForCurrentUser.path
  }

  var selectedRow: DiskRow? {
    rowPage?.rows.first { $0.id == selectedRowID }
  }

  init() {
    do {
      let client = try NativeServiceClient()
      self.client = client
      client.onEvent = { [weak self] event, payload in
        Task { @MainActor in self?.handle(event: event, payload: payload) }
      }
      Task { await loadInitialState() }
    } catch {
      client = nil
      errorMessage = error.localizedDescription
    }
  }

  func scanHome() {
    scan(path: homePath)
  }

  func scan(path: String) {
    guard let client, canScan else { return }
    invalidateTrashPreparation()
    errorMessage = nil
    isScanning = true
    scanProgress = nil
    scanNotice = scanSummary == nil ? nil : "Showing the previous scan while a new scan runs."

    Task {
      do {
        let _: [String: String] = try await client.request(
          "startScan",
          params: ["rootPath": path],
          as: [String: String].self
        )
        if isScanning { startPolling() }
      } catch {
        isScanning = false
        scanNotice = "The scan did not start. Previous results are unchanged."
        self.errorMessage = error.localizedDescription
      }
    }
  }

  func cancelScan() {
    guard let client else { return }
    Task {
      do {
        let _: Bool = try await client.request("cancelScan", as: Bool.self)
      } catch {
        errorMessage = error.localizedDescription
      }
    }
  }

  func chooseBackend(_ id: String?) {
    guard let client else { return }
    Task {
      do {
        let value: BackendSelection = try await client.request(
          "chooseBackend",
          params: ["id": id as Any? ?? NSNull()],
          as: BackendSelection.self
        )
        backendSelection = value
        if let saveError = value.saveError { errorMessage = saveError }
      } catch {
        errorMessage = error.localizedDescription
      }
    }
  }

  func toggleDashboardCategory(_ category: StorageCategory?) {
    dashboardCategoryFilter = dashboardCategoryFilter == category ? nil : category
  }

  func openRootBrowser() {
    selectedConsumer = nil
    selectedRowID = nil
    loadRows(parentID: nil, offset: 0)
  }

  func inspect(_ consumer: CategoryConsumer) {
    selectedConsumer = consumer
    selectedRowID = nil
    loadRows(parentID: consumer.parentId, offset: 0, keepingConsumerSelection: true)
  }

  func open(_ consumer: CategoryConsumer) {
    selectedConsumer = nil
    selectedRowID = nil
    if consumer.isDir {
      loadRows(parentID: consumer.id, offset: 0)
    } else {
      loadRows(parentID: consumer.parentId, offset: 0, selecting: consumer.id)
    }
  }

  func inspectInstalledApplication(_ application: InstalledApplication) {
    guard let client else { return }
    footprintRequestVersion += 1
    let requestVersion = footprintRequestVersion
    inspectedApplication = application
    appFootprint = nil
    isLoadingAppFootprint = true
    Task {
      defer { if requestVersion == footprintRequestVersion { isLoadingAppFootprint = false } }
      do {
        let value: AppFootprint = try await client.request(
          "installedAppFootprint", params: ["uninstallName": application.uninstallName],
          as: AppFootprint.self)
        guard requestVersion == footprintRequestVersion else { return }
        appFootprint = value
      } catch {
        if requestVersion == footprintRequestVersion { errorMessage = error.localizedDescription }
      }
    }
  }

  func selectForRemoval(_ application: InstalledApplication) {
    guard !isExecutingUninstall else { return }
    uninstallRequestVersion += 1
    selectedApplications = [application.uninstallName]
    uninstallPreview = nil
    uninstallPreparation = nil
  }

  func revealPath(_ path: String) {
    guard let client else { return }
    Task {
      do {
        let _: [String: Bool] = try await client.request(
          "revealPath", params: ["path": path], as: [String: Bool].self)
      } catch { errorMessage = error.localizedDescription }
    }
  }

  func openTrash() {
    NSWorkspace.shared.open(
      FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".Trash"))
  }

  func rescan() {
    guard let path = scanSummary?.rootPath else { return }
    scan(path: path)
  }

  func inspectApplication(_ consumer: CategoryConsumer) {
    guard let client, let scanSummary else { return }
    footprintRequestVersion += 1
    let requestVersion = footprintRequestVersion
    inspectedApplication = applicationInventory?.rows.first { $0.path == consumer.path }
    appFootprint = nil
    isLoadingAppFootprint = true
    errorMessage = nil

    Task {
      defer { if requestVersion == footprintRequestVersion { isLoadingAppFootprint = false } }
      do {
        let footprint: AppFootprint = try await client.request(
          "appFootprint",
          params: ["scanId": scanSummary.scanId, "nodeId": consumer.id],
          as: AppFootprint.self
        )
        guard requestVersion == footprintRequestVersion,
          self.scanSummary?.scanId == footprint.scanId
        else { return }
        appFootprint = footprint
      } catch {
        errorMessage = error.localizedDescription
      }
    }
  }

  func openApplicationInBrowser() {
    guard let appFootprint, appFootprint.scanId != 0 else { return }
    selectedConsumer = nil
    selectedRowID = nil
    loadRows(parentID: appFootprint.nodeId, offset: 0)
  }

  func revealApplicationInFinder() {
    guard let appFootprint else { return }
    revealPath(appFootprint.path)
  }

  func open(_ crumb: Crumb) {
    selectedConsumer = nil
    selectedRowID = nil
    loadRows(parentID: crumb.id, offset: 0)
  }

  func open(_ row: DiskRow) {
    selectedConsumer = nil
    guard row.kind == .directory else {
      selectedRowID = row.id
      return
    }
    guard rowPage != nil else { return }
    selectedRowID = nil
    loadRows(parentID: row.id, offset: 0)
  }

  func goUpInBrowser() {
    guard let parent = rowPage?.ancestors.last else { return }
    open(parent)
  }

  func selectRow(_ id: UInt32?) {
    selectedConsumer = nil
    selectedRowID = id
  }

  func setBrowserSort(_ sort: SortOrder) {
    guard browserSort != sort else { return }
    browserSort = sort
    loadRows(parentID: currentParentID, offset: 0)
  }

  func goBackInBrowser() {
    guard let parent = browserHistory.back() else { return }
    loadRows(parentID: parent.parentID, offset: 0, recordHistory: false)
  }

  func goForwardInBrowser() {
    guard let parent = browserHistory.forward() else { return }
    loadRows(parentID: parent.parentID, offset: 0, recordHistory: false)
  }

  func previousRowsPage() {
    guard let page = rowPage, page.offset > 0 else { return }
    loadRows(parentID: currentParentID, offset: page.offset - 100)
  }

  func nextRowsPage() {
    guard let page = rowPage, page.offset + UInt32(page.rows.count) < page.total else { return }
    loadRows(parentID: currentParentID, offset: page.offset + 100)
  }

  func revealSelectedRow() {
    guard let client, let scanSummary, let selectedRowID else { return }
    Task {
      do {
        let _: [String: Bool] = try await client.request(
          "reveal",
          params: ["scanId": scanSummary.scanId, "nodeId": selectedRowID],
          as: [String: Bool].self
        )
      } catch {
        errorMessage = error.localizedDescription
      }
    }
  }

  func canMoveToTrash(_ row: DiskRow) -> Bool {
    canMoveToTrash(nodeID: row.id)
  }

  func canMoveToTrash(_ consumer: CategoryConsumer) -> Bool {
    canMoveToTrash(nodeID: consumer.id)
  }

  private func canMoveToTrash(nodeID: UInt32) -> Bool {
    !isScanning && !isLoadingRows && !isPreparingTrash && !isMovingToTrash
      && trashPreparation == nil
      && !trashedRowIDs.contains(nodeID)
  }

  func reveal(_ row: DiskRow) {
    reveal(nodeID: row.id)
  }

  func reveal(_ consumer: CategoryConsumer) {
    reveal(nodeID: consumer.id)
  }

  private func reveal(nodeID: UInt32) {
    guard let client, let scanSummary else { return }
    Task {
      do {
        let _: [String: Bool] = try await client.request(
          "reveal",
          params: ["scanId": scanSummary.scanId, "nodeId": nodeID],
          as: [String: Bool].self
        )
      } catch {
        errorMessage = error.localizedDescription
      }
    }
  }

  func prepareTrash(_ row: DiskRow) {
    selectedConsumer = nil
    prepareTrash(nodeID: row.id)
  }

  func prepareTrash(_ consumer: CategoryConsumer) {
    selectedConsumer = consumer
    prepareTrash(nodeID: consumer.id)
  }

  private func prepareTrash(nodeID: UInt32) {
    guard let client, let scanSummary, canMoveToTrash(nodeID: nodeID) else { return }
    selectedRowID = nodeID
    trashPendingRowID = nodeID
    errorMessage = nil
    trashStatusMessage = nil
    trashRequestVersion += 1
    let requestVersion = trashRequestVersion
    let scanID = scanSummary.scanId
    isPreparingTrash = true

    Task {
      do {
        let preparation: TrashPreparation = try await client.request(
          "prepareTrash",
          params: ["scanId": scanID, "nodeId": nodeID],
          as: TrashPreparation.self
        )
        guard requestVersion == trashRequestVersion else { return }
        guard self.scanSummary?.scanId == scanID else {
          isPreparingTrash = false
          trashPendingRowID = nil
          return
        }
        trashPreparation = preparation
        isTrashConfirmationPresented = true
      } catch {
        guard requestVersion == trashRequestVersion else { return }
        trashPendingRowID = nil
        errorMessage = error.localizedDescription
      }
      if requestVersion == trashRequestVersion { isPreparingTrash = false }
    }
  }

  func cancelTrashConfirmation() {
    invalidateTrashPreparation()
  }

  func confirmTrash() {
    guard let client,
      let scanSummary,
      let preparation = trashPreparation,
      let rowID = trashPendingRowID
    else { return }
    let scanID = scanSummary.scanId
    trashPreparation = nil
    isTrashConfirmationPresented = false
    trashPendingRowID = nil
    isMovingToTrash = true
    errorMessage = nil
    trashStatusMessage = nil

    Task {
      defer { isMovingToTrash = false }
      do {
        let operation: TrashOperation = try await client.request(
          "confirmTrash",
          params: ["confirmationToken": preparation.confirmationToken],
          as: TrashOperation.self
        )
        if self.scanSummary?.scanId == scanID {
          trashedRowIDs.insert(rowID)
        }
        trashStatusMessage = "Moved to Trash. Rescan to update sizes."
        trashOperations.removeAll { $0.id == operation.id }
        trashOperations.insert(operation, at: 0)
      } catch {
        errorMessage = error.localizedDescription
      }
    }
  }

  func loadCleanupPreview() {
    guard let client, canClean, !isScanning, !isLoadingCleanupPreview, !isExecutingCleanup else {
      return
    }
    isLoadingCleanupPreview = true
    cleanupProgress = []
    cleanupPreview = nil
    cleanupPreparation = nil
    cleanupResult = nil
    errorMessage = nil

    Task {
      defer { isLoadingCleanupPreview = false }
      do {
        cleanupPreview = try await client.request("cleanupPreview", as: CleanupPreview.self)
      } catch {
        errorMessage = error.localizedDescription
      }
    }
  }

  func cancelCleanupPreview() {
    guard let client else { return }
    Task {
      do {
        let _: Bool = try await client.request("cancelCleanupPreview", as: Bool.self)
      } catch {
        errorMessage = error.localizedDescription
      }
    }
  }

  func prepareCleanup() {
    guard let client, cleanupPreview != nil else { return }
    Task {
      do {
        cleanupPreparation = try await client.request("prepareCleanup", as: CleanupPreparation.self)
        isCleanupConfirmationPresented = true
      } catch {
        errorMessage = error.localizedDescription
      }
    }
  }

  func confirmCleanup() {
    guard let client, let preparation = cleanupPreparation else { return }
    isCleanupConfirmationPresented = false
    isExecutingCleanup = true
    cleanupPreview = nil
    cleanupPreparation = nil
    errorMessage = nil

    Task {
      defer { isExecutingCleanup = false }
      do {
        let operation: CleanupOperation = try await client.request(
          "confirmCleanup",
          params: ["confirmationToken": preparation.confirmationToken],
          as: CleanupOperation.self
        )
        cleanupResult = operation
        cleanupPreview = nil
        cleanupPreparation = nil
        await loadActivity()
      } catch {
        errorMessage = error.localizedDescription
      }
    }
  }

  func cancelCleanup() {
    guard let client else { return }
    Task {
      do {
        let _: Bool = try await client.request("cancelCleanup", as: Bool.self)
      } catch {
        errorMessage = error.localizedDescription
      }
    }
  }

  func loadOptimizePreview() {
    guard let client, !isLoadingOptimizePreview, !isExecutingOptimize else { return }
    isLoadingOptimizePreview = true
    optimizePreview = nil
    optimizePreparation = nil
    optimizeResult = nil
    errorMessage = nil

    Task {
      defer { isLoadingOptimizePreview = false }
      do {
        optimizePreview = try await client.request("optimizePreview", as: OptimizePreview.self)
      } catch {
        errorMessage = error.localizedDescription
      }
    }
  }

  func cancelOptimizePreview() {
    guard let client else { return }
    Task {
      do {
        let _: Bool = try await client.request("cancelOptimizePreview", as: Bool.self)
      } catch {
        errorMessage = error.localizedDescription
      }
    }
  }

  func prepareOptimize() {
    guard let client, optimizePreview != nil else { return }
    Task {
      do {
        optimizePreparation = try await client.request(
          "prepareOptimize", as: OptimizePreparation.self)
        isOptimizeConfirmationPresented = true
      } catch {
        errorMessage = error.localizedDescription
      }
    }
  }

  func confirmOptimize() {
    guard let client, let preparation = optimizePreparation else { return }
    isOptimizeConfirmationPresented = false
    isExecutingOptimize = true
    errorMessage = nil

    Task {
      defer { isExecutingOptimize = false }
      do {
        optimizeResult = try await client.request(
          "confirmOptimize",
          params: ["confirmationToken": preparation.confirmationToken],
          as: OptimizeOperation.self
        )
        optimizePreview = nil
        optimizePreparation = nil
      } catch {
        errorMessage = error.localizedDescription
      }
    }
  }

  func cancelOptimize() {
    guard let client else { return }
    Task {
      do {
        let _: Bool = try await client.request("cancelOptimize", as: Bool.self)
      } catch {
        errorMessage = error.localizedDescription
      }
    }
  }

  func loadApplications(force: Bool = false) {
    guard let client, !isLoadingApplications else { return }
    guard force || applicationInventory == nil else { return }
    isLoadingApplications = true
    errorMessage = nil

    Task {
      defer { isLoadingApplications = false }
      do {
        applicationInventory = try await client.request(
          "installedApplicationInventory",
          as: InstalledApplicationInventory.self
        )
        selectedApplications = []
        uninstallPreview = nil
      } catch {
        errorMessage = error.localizedDescription
      }
    }
  }

  func toggleApplication(_ application: InstalledApplication) {
    guard !isExecutingUninstall else { return }
    uninstallRequestVersion += 1
    if selectedApplications.contains(application.uninstallName) {
      selectedApplications.remove(application.uninstallName)
    } else {
      selectedApplications.insert(application.uninstallName)
    }
    uninstallPreview = nil
    uninstallPreparation = nil
  }

  func previewUninstall() {
    guard let client, canUninstallApplications, !selectedApplications.isEmpty,
      !isLoadingUninstallPreview, !isExecutingUninstall
    else { return }
    uninstallRequestVersion += 1
    let requestVersion = uninstallRequestVersion
    let names = selectedApplications.sorted()
    isLoadingUninstallPreview = true
    uninstallPreview = nil
    uninstallPreparation = nil
    uninstallResult = nil
    errorMessage = nil

    Task {
      defer { isLoadingUninstallPreview = false }
      do {
        let value: UninstallPreview = try await client.request(
          "uninstallPreview",
          params: ["names": names],
          as: UninstallPreview.self
        )
        guard requestVersion == uninstallRequestVersion, names == selectedApplications.sorted()
        else { return }
        uninstallPreview = value
      } catch {
        errorMessage = error.localizedDescription
      }
    }
  }

  func cancelUninstallPreview() {
    uninstallRequestVersion += 1
    guard let client else { return }
    Task {
      do {
        let _: Bool = try await client.request("cancelUninstallPreview", as: Bool.self)
      } catch {
        errorMessage = error.localizedDescription
      }
    }
  }

  func prepareUninstall() {
    guard let client, let preview = uninstallPreview, Set(preview.requested) == selectedApplications
    else { return }
    let requestVersion = uninstallRequestVersion
    Task {
      do {
        let preparation: UninstallPreparation = try await client.request(
          "prepareUninstall", as: UninstallPreparation.self)
        guard requestVersion == uninstallRequestVersion,
          Set(preview.requested) == selectedApplications
        else { return }
        uninstallPreparation = preparation
        isUninstallConfirmationPresented = true
      } catch {
        errorMessage = error.localizedDescription
      }
    }
  }

  func confirmUninstall() {
    guard let client, let preparation = uninstallPreparation else { return }
    isUninstallConfirmationPresented = false
    isExecutingUninstall = true
    uninstallPreview = nil
    uninstallPreparation = nil
    errorMessage = nil

    Task {
      defer { isExecutingUninstall = false }
      do {
        let operation: UninstallOperation = try await client.request(
          "confirmUninstall",
          params: ["confirmationToken": preparation.confirmationToken],
          as: UninstallOperation.self
        )
        uninstallResult = operation
        uninstallPreview = nil
        uninstallPreparation = nil
        selectedApplications = []
        await loadActivity()
        loadApplications(force: true)
      } catch {
        errorMessage = error.localizedDescription
      }
    }
  }

  func cancelUninstall() {
    guard let client else { return }
    Task {
      do {
        let _: Bool = try await client.request("cancelUninstall", as: Bool.self)
      } catch {
        errorMessage = error.localizedDescription
      }
    }
  }

  func loadSystemStatus(force: Bool = false) {
    guard let client, !isLoadingSystemStatus else { return }
    guard force || systemStatus == nil else { return }
    isLoadingSystemStatus = true
    errorMessage = nil

    Task {
      defer { isLoadingSystemStatus = false }
      do {
        systemStatus = try await client.request("systemStatus", as: SystemStatus.self)
      } catch {
        errorMessage = error.localizedDescription
      }
    }
  }

  func loadActivity() async {
    guard let client, !isLoadingActivity else { return }
    isLoadingActivity = true
    defer { isLoadingActivity = false }
    do {
      async let trash: [TrashOperation] = client.request("trashLog", as: [TrashOperation].self)
      async let deletions: [DeleteOperation] = client.request(
        "operationLog", as: [DeleteOperation].self)
      async let cleanups: [CleanupOperation] = client.request(
        "cleanupLog", as: [CleanupOperation].self)
      async let uninstalls: [UninstallOperation] = client.request(
        "uninstallLog", as: [UninstallOperation].self)
      let (loadedTrash, loadedDeletes, loadedCleanups, loadedUninstalls) = try await (
        trash,
        deletions,
        cleanups,
        uninstalls
      )
      trashOperations = loadedTrash
      deleteOperations = loadedDeletes
      cleanupOperations = loadedCleanups
      uninstallOperations = loadedUninstalls
    } catch {
      errorMessage = error.localizedDescription
    }
  }

  func refreshBackends() {
    guard !isCheckingBackends else { return }
    Task { await loadBackendState() }
  }

  private func loadBackendState() async {
    guard let client else {
      isCheckingBackends = false
      return
    }
    isCheckingBackends = true
    defer { isCheckingBackends = false }
    do {
      backends = try await client.request("listBackends", as: [Backend].self)
      backendSelection = try await client.request("backendSelection", as: BackendSelection.self)
    } catch { errorMessage = error.localizedDescription }
  }

  private func loadInitialState() async {
    defer { isCheckingBackends = false }
    guard let client else { return }
    do {
      async let loadedBackends: [Backend] = client.request("listBackends", as: [Backend].self)
      async let loadedSelection: BackendSelection = client.request(
        "backendSelection", as: BackendSelection.self)
      async let status: ScanStatus = client.request("scanStatus", as: ScanStatus.self)
      async let loadedVolume: VolumeInfo? = try? client.request(
        "volumeInfo",
        params: ["path": homePath],
        as: VolumeInfo.self
      )
      backends = try await loadedBackends
      backendSelection = try await loadedSelection
      volumeInfo = await loadedVolume
      let current = try await status
      scanSummary = current.summary
      scanProgress = current.progress
      isScanning = current.active != nil
      if isScanning { startPolling() }
      if current.summary != nil { await loadDashboard() }
    } catch {
      errorMessage = error.localizedDescription
    }
  }

  private func startPolling() {
    pollingTask?.cancel()
    pollingTask = Task { [weak self] in
      while !Task.isCancelled {
        try? await Task.sleep(nanoseconds: 750_000_000)
        guard !Task.isCancelled else { return }
        await self?.refreshScanStatus()
      }
    }
  }

  private func refreshScanStatus() async {
    guard let client else { return }
    do {
      let status: ScanStatus = try await client.request("scanStatus", as: ScanStatus.self)
      scanProgress = status.progress
      if let summary = status.summary, summary.scanId != scanSummary?.scanId {
        acceptScan(summary)
        isScanning = false
        pollingTask?.cancel()
        await loadDashboard()
      } else if status.active == nil {
        isScanning = false
        pollingTask?.cancel()
      }
    } catch {
      errorMessage = error.localizedDescription
      pollingTask?.cancel()
    }
  }

  private func acceptScan(_ summary: ScanSummary) {
    guard summary.scanId != scanSummary?.scanId else { return }
    rowRequestVersion += 1
    footprintRequestVersion += 1
    scanSummary = summary
    scanNotice = nil
    trashedRowIDs = []
    trashStatusMessage = nil
    categories = nil
    dashboardCategoryFilter = nil
    appFootprint = nil
    isLoadingAppFootprint = false
    rowPage = nil
    selectedRowID = nil
    selectedConsumer = nil
    currentParentID = nil
    browserHistory = BrowserHistory()
  }

  private func loadDashboard() async {
    guard let client, let scanSummary else { return }
    do {
      let value: CategoryBreakdown = try await client.request(
        "categoryBreakdown",
        params: ["scanId": scanSummary.scanId],
        as: CategoryBreakdown.self
      )
      guard self.scanSummary?.scanId == scanSummary.scanId else { return }
      categories = value
      loadRows(parentID: nil, offset: 0)
    } catch {
      errorMessage = error.localizedDescription
    }
  }

  private func loadRows(
    parentID: UInt32?,
    offset: UInt32,
    selecting rowID: UInt32? = nil,
    keepingConsumerSelection: Bool = false,
    recordHistory: Bool = true
  ) {
    guard let client, let scanSummary else { return }
    invalidateTrashPreparation()
    rowRequestVersion += 1
    let requestVersion = rowRequestVersion
    if recordHistory { browserHistory.visit(parentID) }
    currentParentID = parentID
    isLoadingRows = true
    selectedRowID = rowID
    if !keepingConsumerSelection { selectedConsumer = nil }
    let requestedSort = browserSort
    let parent: Any = parentID.map { $0 as Any } ?? NSNull()
    Task {
      defer { if requestVersion == rowRequestVersion { isLoadingRows = false } }
      do {
        let page: RowPage = try await client.request(
          "rows",
          params: [
            "scanId": scanSummary.scanId,
            "parentId": parent,
            "sort": requestedSort.rawValue,
            "offset": offset,
            "limit": 100,
          ],
          as: RowPage.self
        )
        guard requestVersion == rowRequestVersion,
          self.scanSummary?.scanId == scanSummary.scanId
        else { return }
        rowPage = page
        selectedRowID = rowID
      } catch {
        errorMessage = error.localizedDescription
      }
    }
  }

  private func invalidateTrashPreparation() {
    trashRequestVersion += 1
    isPreparingTrash = false
    trashPreparation = nil
    isTrashConfirmationPresented = false
    trashPendingRowID = nil
  }

  private func handle(event: String, payload: Data) {
    switch event {
    case "scan://progress":
      scanProgress = try? JSONDecoder().decode(ScanProgress.self, from: payload)
    case "scan://finished":
      guard let summary = try? JSONDecoder().decode(ScanSummary.self, from: payload) else { return }
      acceptScan(summary)
      isScanning = false
      pollingTask?.cancel()
      Task { await loadDashboard() }
    case "scan://failed":
      let failure = try? JSONDecoder().decode(ScanFailure.self, from: payload)
      isScanning = false
      pollingTask?.cancel()
      scanProgress = nil
      scanNotice =
        scanSummary == nil ? "Scan stopped." : "Scan stopped. Showing the previous results."
      if let failure, !failure.cancelled { errorMessage = failure.message }
    case "cleanup://progress":
      if let progress = try? JSONDecoder().decode(CleanupProgress.self, from: payload) {
        cleanupProgress.append(progress)
        if cleanupProgress.count > 80 { cleanupProgress.removeFirst(cleanupProgress.count - 80) }
      }
    default:
      break
    }
  }
}
