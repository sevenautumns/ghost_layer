// swift-tools-version: 5.9
import PackageDescription

// GhostLayer.xcframework is built via CI on each release tag.
// URL and checksum are updated automatically by the release workflow.
let package = Package(
    name: "GhostLayer",
    products: [
        .library(name: "GhostLayer", targets: ["GhostLayer"]),
    ],
    targets: [
        .binaryTarget(
            name: "GhostLayer",
            url: "https://github.com/sevenautumns/ghost_layer/releases/download/v0.0.4/GhostLayer.xcframework.zip",
            checksum: "4cc7541f277a9c304c5c4fb9576df2396737547923c5ed7b1f1b3339a6a4ffe1"
        )
    ]
)
