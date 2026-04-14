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
            url: "https://github.com/sevenautumns/ghost_layer/releases/download/v0.0.2/GhostLayer.xcframework.zip",
            checksum: "3559f18f970592f895e02981f8a8833f34aa2932617549b0aecdc1e49625590a"
        )
    ]
)
