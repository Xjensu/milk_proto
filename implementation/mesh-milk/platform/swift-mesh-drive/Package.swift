// swift-tools-version: 5.7

import PackageDescription

let package = Package(
    name: "MeshDrive",
    products: [
        .library(
            name: "MeshDrive",
            targets: ["MeshDrive"]
        )
    ],
    targets: [
        .target(
            name: "MeshDrive",
            path: "Sources"
        ),
        .testTarget(
            name: "MeshDriveTests",
            dependencies: ["MeshDrive"],
            path: "Tests/MeshDriveTests"
        )
    ]
)
