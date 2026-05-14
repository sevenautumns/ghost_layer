// swift-tools-version: 5.9
import PackageDescription

/// GhostLayer.xcframework is built via CI on each release tag.
/// URL and checksum are updated automatically by the release workflow.
let package = Package(
    name: "GhostLayer",
    products: [
        .library(name: "GhostLayer", targets: ["GhostLayer"]),
    ],
    targets: [
        .target(
            name: "GhostLayer",
            dependencies: ["GhostLayerFFI"],
            path: "Sources/GhostLayer"
        ),
        .binaryTarget(
            name: "GhostLayerFFI",
            url: "https://github.com/sevenautumns/GhostLayer/releases/download/v0.1.0/GhostLayer.xcframework.zip",
            checksum: "4f3f8de23282d1ccbb80e69e0dc8459c1c1d02be8cc3038f7f4fe2a7c503f70b"
        ),
        .testTarget(
            name: "GhostLayerTests",
            dependencies: ["GhostLayer"],
            path: "Tests/GhostLayerTests"
        ),
    ]
)
