import Foundation

enum NativeServiceError: LocalizedError {
  case missingExecutable(String)
  case launch(String)
  case remote(String)
  case invalidResponse

  var errorDescription: String? {
    switch self {
    case .missingExecutable(let path): "The Rust service was not found at \(path)."
    case .launch(let message): "The Rust service could not start: \(message)"
    case .remote(let message): message
    case .invalidResponse: "The Rust service returned an invalid response."
    }
  }
}

final class NativeServiceClient {
  private typealias Reply = (Result<Data, Error>) -> Void

  private let process: Process
  private let input: Pipe
  private let output: Pipe
  private let lock = NSLock()
  private let writeLock = NSLock()
  private var nextID = 0
  private var pending: [Int: Reply] = [:]
  private var outputBuffer = Data()
  var onEvent: ((String, Data) -> Void)?

  init(environment: [String: String]? = nil) throws {
    let executable = Self.serviceURL()
    guard FileManager.default.isExecutableFile(atPath: executable.path) else {
      throw NativeServiceError.missingExecutable(executable.path)
    }

    process = Process()
    input = Pipe()
    output = Pipe()
    process.executableURL = executable
    if let environment { process.environment = environment }
    process.standardInput = input
    process.standardOutput = output
    process.standardError = FileHandle.standardError

    output.fileHandleForReading.readabilityHandler = { [weak self] handle in
      let data = handle.availableData
      guard !data.isEmpty else {
        handle.readabilityHandler = nil
        self?.failPending(NativeServiceError.launch("the service closed its output"))
        return
      }
      self?.receive(data)
    }
    process.terminationHandler = { [weak self] process in
      self?.failPending(
        NativeServiceError.launch("the service exited with status \(process.terminationStatus)")
      )
    }

    do {
      try process.run()
    } catch {
      throw NativeServiceError.launch(error.localizedDescription)
    }
  }

  deinit {
    output.fileHandleForReading.readabilityHandler = nil
    process.terminationHandler = nil
    try? input.fileHandleForWriting.close()
    // EOF lets Rust cancel and join its subprocess-backed operations.
    // Terminating the helper here could orphan a scanner.
  }

  func request<T: Decodable>(
    _ method: String,
    params: [String: Any] = [:],
    as responseType: T.Type
  ) async throws -> T {
    let data = try await requestData(method, params: params)
    return try JSONDecoder().decode(responseType, from: data)
  }

  func requestData(_ method: String, params: [String: Any] = [:]) async throws -> Data {
    try await withCheckedThrowingContinuation { continuation in
      lock.lock()
      nextID += 1
      let id = nextID
      pending[id] = { result in continuation.resume(with: result) }
      lock.unlock()

      var message: [String: Any] = ["id": id, "method": method, "params": params]
      do {
        var data = try JSONSerialization.data(withJSONObject: message)
        data.append(0x0A)
        writeLock.lock()
        defer { writeLock.unlock() }
        try input.fileHandleForWriting.write(contentsOf: data)
      } catch {
        removePending(id, result: .failure(error))
      }
      message.removeAll(keepingCapacity: false)
    }
  }

  private static func serviceURL() -> URL {
    if let override = ProcessInfo.processInfo.environment["NIRMOKA_NATIVE_SERVICE"] {
      return URL(fileURLWithPath: override)
    }
    return Bundle.main.bundleURL
      .appendingPathComponent("Contents/Helpers/nirmoka-native-service")
  }

  private func receive(_ data: Data) {
    lock.lock()
    outputBuffer.append(data)
    let newline = Data([0x0A])
    var lines: [Data] = []
    while let range = outputBuffer.range(of: newline) {
      lines.append(Data(outputBuffer[..<range.lowerBound]))
      outputBuffer.removeSubrange(..<range.upperBound)
    }
    lock.unlock()

    for line in lines where !line.isEmpty {
      receiveLine(line)
    }
  }

  private func receiveLine(_ line: Data) {
    guard let object = try? JSONSerialization.jsonObject(with: line) as? [String: Any] else {
      return
    }

    if let event = object["event"] as? String,
      let payload = object["payload"],
      let payloadData = try? JSONSerialization.data(
        withJSONObject: payload, options: [.fragmentsAllowed])
    {
      DispatchQueue.main.async { [weak self] in self?.onEvent?(event, payloadData) }
      return
    }

    guard let id = (object["id"] as? NSNumber)?.intValue else { return }
    if let error = object["error"] as? String {
      removePending(id, result: .failure(NativeServiceError.remote(error)))
      return
    }
    guard let value = object["result"],
      let result = try? JSONSerialization.data(withJSONObject: value, options: [.fragmentsAllowed])
    else {
      removePending(id, result: .failure(NativeServiceError.invalidResponse))
      return
    }
    removePending(id, result: .success(result))
  }

  private func removePending(_ id: Int, result: Result<Data, Error>) {
    lock.lock()
    let reply = pending.removeValue(forKey: id)
    lock.unlock()
    reply?(result)
  }

  private func failPending(_ error: Error) {
    lock.lock()
    let replies = Array(pending.values)
    pending.removeAll()
    lock.unlock()
    replies.forEach { $0(.failure(error)) }
  }
}
