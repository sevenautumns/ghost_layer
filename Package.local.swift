// swift-tools-version: 5.9
import PackageDescription

/// Local development manifest — used by `make test`.
/// Package.swift points to a remote binary for consumers; this one
/// uses the locally-built xcframework so `swift test` can resolve it.
let package = Package(
    name: "GhostLayer",
    products: [
        .library(name: "GhostLayer", targets: ["GhostLayer"]),
    ],
    targets: [
        .binaryTarget(
            name: "GhostLayerFFI",
            path: "GhostLayer.xcframework"
        ),
        .target(
            name: "GhostLayer",
            dependencies: ["GhostLayerFFI"],
            path: "Sources/GhostLayer"
        ),
        .testTarget(
            name: "GhostLayerTests",
            dependencies: ["GhostLayer"],
            path: "tests/GhostLayerTests"
        ),
    ]
)
