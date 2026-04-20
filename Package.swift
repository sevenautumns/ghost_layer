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
            url: "https://github.com/sevenautumns/GhostLayer/releases/download/v0.0.5/GhostLayer.xcframework.zip",
            checksum: "a0199f144e40211b1a32e87cb5db29caa0a893ad467bea128597d845b53c22f0"
        )
    ]
)
