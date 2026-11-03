import Foundation

struct BrowserLocation: Equatable {
  let parentID: UInt32?
}

struct BrowserHistory {
  private(set) var entries: [BrowserLocation] = []
  private(set) var index = -1
  var canGoBack: Bool { index > 0 }
  var canGoForward: Bool { index >= 0 && index + 1 < entries.count }

  mutating func visit(_ parentID: UInt32?) {
    let next = BrowserLocation(parentID: parentID)
    if index >= 0 && entries[index] == next { return }
    entries = Array(entries.prefix(index + 1)) + [next]
    index = entries.count - 1
  }
  mutating func back() -> BrowserLocation? {
    guard canGoBack else { return nil }
    index -= 1
    return entries[index]
  }
  mutating func forward() -> BrowserLocation? {
    guard canGoForward else { return nil }
    index += 1
    return entries[index]
  }
}
