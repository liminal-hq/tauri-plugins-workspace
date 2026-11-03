// Defines Workshop item types and the backend trait for Steam Workshop content
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use serde::{Deserialize, Serialize};

use crate::{
    backend::{unsupported, BackendReason, Done},
    error::Result,
};

/// A Workshop item the player is subscribed to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkshopItem {
    /// The published file ID as a decimal string, because JavaScript numbers lose 64-bit precision.
    pub id: String,
    pub subscribed: bool,
    pub installed: bool,
    pub needs_update: bool,
    pub downloading: bool,
    /// The folder the item is installed in, when it is.
    pub install_path: Option<String>,
    pub size_on_disk: Option<u64>,
    /// Unix time the installed copy was last updated.
    pub timestamp: Option<u32>,
}

/// Workshop calls. The defaults report the backend as unable.
pub trait WorkshopBackend: BackendReason {
    fn workshop_items(&self) -> Result<Vec<WorkshopItem>> {
        unsupported(self.unavailable_reason())
    }
    fn subscribe_item(&self, _id: u64, done: Done<()>) {
        done(unsupported(self.unavailable_reason()))
    }
    fn unsubscribe_item(&self, _id: u64, done: Done<()>) {
        done(unsupported(self.unavailable_reason()))
    }
    /// Starts or prioritises a download. Returns whether Steam accepted the request.
    fn download_item(&self, _id: u64, _high_priority: bool) -> Result<bool> {
        unsupported(self.unavailable_reason())
    }
}
