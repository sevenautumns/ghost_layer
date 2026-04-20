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
            url: "https://github.com/sevenautumns/GhostLayer/releases/download/v0.0.6/GhostLayer.xcframework.zip",
            checksum: "b3a8b548b4e121f02184844883b8c9061d0284b1c038f479997a4a59bbea1217"
        )
    ]
)
