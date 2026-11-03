// swift-tools-version: 6.0
import PackageDescription

let package = Package(
  name: "NirmokaMac",
  platforms: [.macOS(.v13)],
  products: [
    .executable(name: "NirmokaMac", targets: ["NirmokaMac"])
  ],
  targets: [
    .executableTarget(
      name: "NirmokaMac",
      path: "Sources/NirmokaMac",
      swiftSettings: [.swiftLanguageMode(.v5)]
    ),
    .testTarget(
      name: "NirmokaMacTests", dependencies: ["NirmokaMac"], path: "Tests/NirmokaMacTests"),
  ]
)
