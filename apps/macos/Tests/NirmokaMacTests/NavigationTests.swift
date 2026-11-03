import XCTest

@testable import NirmokaMac

final class NavigationTests: XCTestCase {
  func testOpeningFirstFolderCanReturnToRoot() {
    var history = BrowserHistory()
    history.visit(nil)
    history.visit(42)
    XCTAssertTrue(history.canGoBack)
    XCTAssertEqual(history.back(), BrowserLocation(parentID: nil))
    XCTAssertEqual(history.forward(), BrowserLocation(parentID: 42))
  }

  func testBranchingNavigationDropsForwardHistoryAndDuplicateVisits() {
    var history = BrowserHistory()
    history.visit(nil)
    history.visit(42)
    history.visit(43)
    _ = history.back()
    history.visit(99)
    history.visit(99)
    XCTAssertFalse(history.canGoForward)
    XCTAssertEqual(history.entries.count, 3)
    XCTAssertEqual(history.back(), BrowserLocation(parentID: 42))
  }

  func testResetDropsIdsFromPreviousScan() {
    var history = BrowserHistory()
    history.visit(nil)
    history.visit(42)
    history = BrowserHistory()
    history.visit(nil)
    XCTAssertFalse(history.canGoBack)
    XCTAssertFalse(history.canGoForward)
  }

  func testMissingScannerIsAnExplicitSetupState() async throws {
    guard ProcessInfo.processInfo.environment["NIRMOKA_NATIVE_SERVICE"] != nil else {
      throw XCTSkip("Set NIRMOKA_NATIVE_SERVICE to test missing-backend setup")
    }
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: directory) }
    // Mask installed scanners with explicitly unsupported versions. Package
    // manager fallbacks intentionally remain available in production.
    for binary in ["ncdu", "gdu"] {
      let executable = directory.appendingPathComponent(binary)
      try "#!/bin/sh\necho '\(binary) 99.0.0'\n".write(
        to: executable, atomically: true, encoding: .utf8)
      try FileManager.default.setAttributes(
        [.posixPermissions: 0o755], ofItemAtPath: executable.path)
    }
    var environment = ProcessInfo.processInfo.environment
    environment["PATH"] = "\(directory.path):/usr/bin:/bin"
    let client = try NativeServiceClient(environment: environment)
    let selection: BackendSelection = try await client.request(
      "backendSelection", as: BackendSelection.self)
    XCTAssertNil(selection.scanner)
  }

  func testRealScanRowsCategoriesAndFootprintDecode() async throws {
    guard ProcessInfo.processInfo.environment["NIRMOKA_NATIVE_SERVICE"] != nil else {
      throw XCTSkip("Set NIRMOKA_NATIVE_SERVICE to verify the scan boundary")
    }
    let client = try NativeServiceClient()
    let selection: BackendSelection = try await client.request(
      "backendSelection", as: BackendSelection.self)
    guard selection.scanner != nil else {
      throw XCTSkip("Install a supported scanner for the live scan contract")
    }
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    let bundle = root.appendingPathComponent("ContractFixture.app")
    try FileManager.default.createDirectory(at: bundle, withIntermediateDirectories: true)
    try Data(repeating: 120, count: 4096).write(to: bundle.appendingPathComponent("sample"))
    defer { try? FileManager.default.removeItem(at: root) }
    let _: [String: String] = try await client.request(
      "startScan", params: ["rootPath": root.path], as: [String: String].self)
    var summary: ScanSummary?
    for _ in 0..<100 {
      let status: ScanStatus = try await client.request("scanStatus", as: ScanStatus.self)
      if let value = status.summary {
        summary = value
        break
      }
      try await Task.sleep(nanoseconds: 20_000_000)
    }
    let completed = try XCTUnwrap(summary)
    let rows: RowPage = try await client.request(
      "rows",
      params: [
        "scanId": completed.scanId, "parentId": NSNull(), "sort": "largestFirst", "offset": 0,
        "limit": 100,
      ], as: RowPage.self)
    XCTAssertEqual(rows.total, 1)
    let app = try XCTUnwrap(rows.rows.first)
    XCTAssertEqual(app.name, "ContractFixture.app")
    let categories: CategoryBreakdown = try await client.request(
      "categoryBreakdown", params: ["scanId": completed.scanId], as: CategoryBreakdown.self)
    XCTAssertEqual(categories.scanId, completed.scanId)
    let footprint: AppFootprint = try await client.request(
      "appFootprint", params: ["scanId": completed.scanId, "nodeId": app.id], as: AppFootprint.self)
    XCTAssertEqual(footprint.nodeId, app.id)
    XCTAssertFalse(footprint.components.isEmpty)
    XCTAssertEqual(footprint.components.first?.paths.first?.source, .scan)
  }

  func testRustServiceDecodesTheNativeBoundary() async throws {
    guard ProcessInfo.processInfo.environment["NIRMOKA_NATIVE_SERVICE"] != nil else {
      throw XCTSkip("Set NIRMOKA_NATIVE_SERVICE to test the real Rust service contract")
    }
    let client = try NativeServiceClient()
    let backends: [Backend] = try await client.request("listBackends", as: [Backend].self)
    let status: ScanStatus = try await client.request("scanStatus", as: ScanStatus.self)
    let selection: BackendSelection = try await client.request(
      "backendSelection", as: BackendSelection.self)
    let values = (backends, status, selection)
    XCTAssertFalse(values.0.isEmpty)
    XCTAssertNil(values.1.active)
    XCTAssertNil(values.1.summary)
    XCTAssertNil(values.2.saveError)
    do {
      let _: TrashPreparation = try await client.request(
        "prepareTrash", params: ["scanId": 0, "nodeId": 0], as: TrashPreparation.self)
      XCTFail("An inventory footprint must not authorize Trash")
    } catch { /* No scan, no destructive identity. */  }
  }
}
