// Exercises the plugin through Tauri's mock runtime on targets without a share UI
//
// The mock runtime has no capability file, so the IPC layer would refuse plugin commands; these
// tests call the same `ShareSheet` methods the commands delegate to.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![cfg(not(any(
    target_os = "android",
    target_os = "ios",
    target_os = "macos",
    target_os = "windows"
)))]

use tauri::test::{mock_builder, mock_context, noop_assets, MockRuntime};
use tauri::{App, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use tauri_plugin_share_sheet::{
    ErrorCode, ShareCapabilities, ShareFileOptions, ShareSheetExt, ShareTextOptions,
};

fn app() -> (App<MockRuntime>, WebviewWindow<MockRuntime>) {
    let app = mock_builder()
        .plugin(tauri_plugin_share_sheet::init())
        .build(mock_context(noop_assets()))
        .expect("the plugin should initialise");
    let window = WebviewWindowBuilder::new(&app, "main", WebviewUrl::default())
        .build()
        .expect("the mock window should open");
    (app, window)
}

#[tokio::test]
async fn can_share_reports_nothing() {
    let (app, window) = app();
    assert_eq!(
        app.share_sheet().can_share(&window).await,
        ShareCapabilities::NONE
    );
}

#[tokio::test]
async fn sharing_a_file_is_unsupported_even_with_bad_arguments() {
    let (app, window) = app();
    for options in [
        ShareFileOptions {
            path: "/tmp/save.zip".to_string(),
            mime: "application/zip".to_string(),
            title: Some("Save".to_string()),
        },
        ShareFileOptions {
            path: "relative.zip".to_string(),
            mime: "nonsense".to_string(),
            title: None,
        },
    ] {
        let error = app
            .share_sheet()
            .share_file(&window, options)
            .await
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::Unsupported);
        #[cfg(target_os = "linux")]
        assert!(error.message.contains("Linux"), "{}", error.message);
    }
}

#[tokio::test]
async fn sharing_text_is_unsupported() {
    let (app, window) = app();
    let error = app
        .share_sheet()
        .share_text(
            &window,
            ShareTextOptions {
                text: "I beat level 3".to_string(),
                title: None,
            },
        )
        .await
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::Unsupported);
    let json = serde_json::to_value(&error).unwrap();
    assert_eq!(json["code"], "unsupported");
    assert!(json["message"].is_string());
}
