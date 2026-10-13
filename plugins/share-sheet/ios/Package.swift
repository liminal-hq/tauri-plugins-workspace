// swift-tools-version:5.3
// Builds the share-sheet plugin's Swift sources against the Tauri iOS API
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import PackageDescription

let package = Package(
    name: "tauri-plugin-share-sheet",
    platforms: [
        .macOS(.v10_13),
        .iOS(.v13),
    ],
    products: [
        .library(
            name: "tauri-plugin-share-sheet",
            type: .static,
            targets: ["tauri-plugin-share-sheet"])
    ],
    dependencies: [
        .package(name: "Tauri", path: "../.tauri/tauri-api")
    ],
    targets: [
        .target(
            name: "tauri-plugin-share-sheet",
            dependencies: [
                .byName(name: "Tauri")
            ],
            path: "Sources")
    ]
)
