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
            url: "https://github.com/sevenautumns/ghost_layer/releases/download/v0.0.0/GhostLayer.xcframework.zip",
            checksum: "0000000000000000000000000000000000000000000000000000000000000000"
        ),
        .testTarget(
            name: "GhostLayerTests",
            dependencies: ["GhostLayer"],
            path: "Tests/GhostLayerTests"
        ),
    ]
)
