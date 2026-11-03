// Exercises the plugin through Tauri's mock runtime with the in-memory Steam fake
//
// The mock runtime has no capability file, so the IPC layer would refuse plugin commands; these
// tests call the same `Steamworks` methods the commands delegate to, and listen for the events.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::sync::{Arc, Mutex};

use tauri::test::{mock_builder, mock_context, noop_assets, MockRuntime};
use tauri::{App, Listener};
use tauri_plugin_steamworks::{
    Builder, MockConfig, StatKind, StatValue, SteamStatus, SteamworksExt, ACHIEVEMENT_STORED_EVENT,
    STATS_STORED_EVENT,
};

fn app_with(config: MockConfig) -> App<MockRuntime> {
    mock_builder()
        .plugin(Builder::new(480).mock(config).build())
        .build(mock_context(noop_assets()))
        .expect("the plugin should initialise")
}

fn config() -> MockConfig {
    MockConfig {
        achievements: vec!["ACH_WIN_ONE_GAME".into()],
        stats: vec![("NumGames".into(), StatValue::Int(0))],
        ..MockConfig::default()
    }
}

#[test]
fn the_mock_reports_itself_available() {
    let app = app_with(config());
    assert!(app.steamworks().is_available());
    assert_eq!(
        serde_json::to_value(app.steamworks().status()).unwrap(),
        serde_json::json!({"state": "available", "backend": "mock", "appId": 480})
    );
}

#[test]
fn unlocking_stores_and_raises_events() {
    let app = app_with(config());
    let seen = Arc::new(Mutex::new(Vec::<String>::new()));
    for event in [ACHIEVEMENT_STORED_EVENT, STATS_STORED_EVENT] {
        let seen = seen.clone();
        app.listen(event, move |e| {
            seen.lock()
                .unwrap()
                .push(format!("{event} {}", e.payload()));
        });
    }

    app.steamworks()
        .unlock_achievement("ACH_WIN_ONE_GAME")
        .unwrap();

    assert!(
        app.steamworks()
            .achievement("ACH_WIN_ONE_GAME")
            .unwrap()
            .achieved
    );
    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 2, "{seen:?}");
    assert!(seen[0].contains("ACH_WIN_ONE_GAME"), "{seen:?}");
    assert!(seen[1].contains("\"success\":true"), "{seen:?}");
}

#[test]
fn stats_round_trip_through_the_plugin_state() {
    let app = app_with(config());
    app.steamworks()
        .set_stat("NumGames", StatValue::Int(4))
        .unwrap();
    app.steamworks().store_stats().unwrap();
    assert_eq!(
        app.steamworks()
            .get_stat("NumGames", StatKind::Int)
            .unwrap(),
        StatValue::Int(4)
    );
}

#[test]
fn an_undeclared_achievement_is_an_error_not_a_panic() {
    let app = app_with(config());
    let err = app.steamworks().unlock_achievement("NOPE").unwrap_err();
    assert_eq!(err.to_string(), "unknown achievement `NOPE`");
}

#[test]
fn the_plugin_without_a_backend_choice_degrades_instead_of_failing_setup() {
    // No mock and, in this build, no linked SDK or no running Steam: setup must still succeed.
    let app = mock_builder()
        .plugin(Builder::new(480).build())
        .build(mock_context(noop_assets()))
        .expect("an unavailable Steam must not fail setup");
    assert!(matches!(
        app.steamworks().status(),
        SteamStatus::Unavailable { .. }
    ));
    let err = app.steamworks().player().unwrap_err().to_string();
    assert!(err.starts_with("unavailable: "), "{err}");
}
