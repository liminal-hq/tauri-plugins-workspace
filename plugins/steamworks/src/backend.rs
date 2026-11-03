// Defines the backend trait that Steam, the mock and the unavailable stub implement
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::{collections::BTreeMap, path::Path, sync::Arc};

use crate::{
    error::{Error, Result},
    models::{
        Achievement, BackendKind, CloudFile, CloudQuota, CloudStatus, DeviceInfo, DlcInfo,
        OverlayPage, Player, StatKind, StatValue, SteamEvent, UnavailableReason,
    },
};

/// Receives events the backends raise.
pub type Emit = Arc<dyn Fn(SteamEvent) + Send + Sync>;

/// One method per capability, so the real client, the mock and the stub share a command surface.
pub trait SteamworksBackend: Send + Sync + 'static {
    fn kind(&self) -> BackendKind;
    fn app_id(&self) -> u32;
    fn player(&self) -> Result<Player>;
    fn achievement(&self, id: &str) -> Result<Achievement>;
    fn unlock_achievement(&self, id: &str) -> Result<()>;
    fn clear_achievement(&self, id: &str) -> Result<()>;
    fn indicate_progress(&self, id: &str, current: u32, max: u32) -> Result<()>;
    fn get_stat(&self, name: &str, kind: StatKind) -> Result<StatValue>;
    fn set_stat(&self, name: &str, value: StatValue) -> Result<()>;
    fn store_stats(&self) -> Result<()>;
    /// Sets or, with `None`, removes rich presence keys. Inputs are already validated.
    fn set_rich_presence(&self, entries: &BTreeMap<String, Option<String>>) -> Result<()>;
    fn clear_rich_presence(&self) -> Result<()>;
    fn activate_overlay(&self, page: OverlayPage) -> Result<()>;
    fn activate_overlay_to_web_page(&self, url: &str) -> Result<()>;
    /// File names are already validated; data is within the size limit.
    fn cloud_write(&self, name: &str, data: &[u8]) -> Result<()>;
    fn cloud_read(&self, name: &str) -> Result<Vec<u8>>;
    fn cloud_delete(&self, name: &str) -> Result<()>;
    fn cloud_list(&self) -> Result<Vec<CloudFile>>;
    fn cloud_quota(&self) -> Result<CloudQuota>;
    fn cloud_status(&self) -> Result<CloudStatus>;
    fn dlc_list(&self) -> Result<Vec<DlcInfo>>;
    fn is_dlc_installed(&self, app_id: u32) -> Result<bool>;
    fn device_info(&self) -> Result<DeviceInfo>;
    fn hook_screenshots(&self, hook: bool) -> Result<()>;
    fn trigger_screenshot(&self) -> Result<()>;
    /// The path is already checked: absolute, an existing image file.
    fn add_screenshot(&self, path: &Path, width: u32, height: u32) -> Result<()>;
}

/// Answers every call with `Error::Unavailable`.
pub struct NullBackend {
    reason: UnavailableReason,
}

impl NullBackend {
    pub fn new(reason: UnavailableReason) -> Self {
        Self { reason }
    }

    fn fail<T>(&self) -> Result<T> {
        Err(Error::Unavailable(self.reason))
    }
}

impl SteamworksBackend for NullBackend {
    fn kind(&self) -> BackendKind {
        // Never reported: the status is `Unavailable` whenever this backend is installed.
        BackendKind::Steam
    }
    fn app_id(&self) -> u32 {
        0
    }
    fn player(&self) -> Result<Player> {
        self.fail()
    }
    fn achievement(&self, _id: &str) -> Result<Achievement> {
        self.fail()
    }
    fn unlock_achievement(&self, _id: &str) -> Result<()> {
        self.fail()
    }
    fn clear_achievement(&self, _id: &str) -> Result<()> {
        self.fail()
    }
    fn indicate_progress(&self, _id: &str, _current: u32, _max: u32) -> Result<()> {
        self.fail()
    }
    fn get_stat(&self, _name: &str, _kind: StatKind) -> Result<StatValue> {
        self.fail()
    }
    fn set_stat(&self, _name: &str, _value: StatValue) -> Result<()> {
        self.fail()
    }
    fn store_stats(&self) -> Result<()> {
        self.fail()
    }
    fn set_rich_presence(&self, _entries: &BTreeMap<String, Option<String>>) -> Result<()> {
        self.fail()
    }
    fn clear_rich_presence(&self) -> Result<()> {
        self.fail()
    }
    fn activate_overlay(&self, _page: OverlayPage) -> Result<()> {
        self.fail()
    }
    fn activate_overlay_to_web_page(&self, _url: &str) -> Result<()> {
        self.fail()
    }
    fn cloud_write(&self, _name: &str, _data: &[u8]) -> Result<()> {
        self.fail()
    }
    fn cloud_read(&self, _name: &str) -> Result<Vec<u8>> {
        self.fail()
    }
    fn cloud_delete(&self, _name: &str) -> Result<()> {
        self.fail()
    }
    fn cloud_list(&self) -> Result<Vec<CloudFile>> {
        self.fail()
    }
    fn cloud_quota(&self) -> Result<CloudQuota> {
        self.fail()
    }
    fn cloud_status(&self) -> Result<CloudStatus> {
        self.fail()
    }
    fn dlc_list(&self) -> Result<Vec<DlcInfo>> {
        self.fail()
    }
    fn is_dlc_installed(&self, _app_id: u32) -> Result<bool> {
        self.fail()
    }
    fn device_info(&self) -> Result<DeviceInfo> {
        self.fail()
    }
    fn hook_screenshots(&self, _hook: bool) -> Result<()> {
        self.fail()
    }
    fn trigger_screenshot(&self) -> Result<()> {
        self.fail()
    }
    fn add_screenshot(&self, _path: &Path, _width: u32, _height: u32) -> Result<()> {
        self.fail()
    }
}
