// Defines lobby types and the backend trait for Steam lobbies
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{
    backend::{unsupported, BackendReason, Done},
    error::Result,
};

pub const LOBBY_CHAT_UPDATE_EVENT: &str = "steamworks://lobby-chat-update";
pub const LOBBY_DATA_UPDATE_EVENT: &str = "steamworks://lobby-data-update";

/// Steam's limit on lobby size.
pub const MAX_LOBBY_MEMBERS: u32 = 250;
/// Steam's limits on lobby data keys and values.
pub const MAX_LOBBY_KEY_BYTES: usize = 255;
pub const MAX_LOBBY_VALUE_BYTES: usize = 8192;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LobbyKind {
    Private,
    FriendsOnly,
    Public,
    Invisible,
}

/// A snapshot of a lobby. Steam IDs are decimal strings, because JavaScript numbers lose 64-bit
/// precision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LobbyInfo {
    pub id: String,
    pub owner: String,
    pub members: Vec<String>,
    pub member_limit: Option<u32>,
    pub data: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LobbyMemberChange {
    Entered,
    Left,
    Disconnected,
    Kicked,
    Banned,
}

/// Lobby calls. Calls Steam answers later take a `Done` that runs from the callback pump. The
/// defaults report the backend as unable, so only backends that support lobbies override them.
pub trait LobbyBackend: BackendReason {
    fn create_lobby(&self, _kind: LobbyKind, _max_members: u32, done: Done<String>) {
        done(unsupported(self.unavailable_reason()))
    }
    fn join_lobby(&self, _id: u64, done: Done<String>) {
        done(unsupported(self.unavailable_reason()))
    }
    fn list_lobbies(&self, done: Done<Vec<LobbyInfo>>) {
        done(unsupported(self.unavailable_reason()))
    }
    fn leave_lobby(&self, _id: u64) -> Result<()> {
        unsupported(self.unavailable_reason())
    }
    fn lobby_info(&self, _id: u64) -> Result<LobbyInfo> {
        unsupported(self.unavailable_reason())
    }
    /// Sets a key, or removes it with `None`. Only the lobby owner may change lobby data.
    fn set_lobby_data(&self, _id: u64, _key: &str, _value: Option<&str>) -> Result<()> {
        unsupported(self.unavailable_reason())
    }
    fn set_lobby_joinable(&self, _id: u64, _joinable: bool) -> Result<()> {
        unsupported(self.unavailable_reason())
    }
}
