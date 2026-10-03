// Combines what each probe found into the plugin's status
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::models::{Feature, FeatureStatus, PluginStatus, UnavailableReason};

/// Every feature, in the order the status lists them.
pub const FEATURES: [Feature; 6] = [
    Feature::Notify,
    Feature::NotificationActions,
    Feature::InhibitSleep,
    Feature::LauncherProgress,
    Feature::FileManager,
    Feature::GlobalShortcuts,
];

/// What probing a Linux session found; each error is the text of why a probe failed.
#[derive(Debug, Clone)]
pub struct LinuxProbes {
    pub session_bus: Result<(), String>,
    /// The name of the notification server.
    pub notify_server: Result<String, String>,
    /// The capabilities the notification server lists.
    pub notify_capabilities: Result<Vec<String>, String>,
    /// Whether logind answers on the system bus.
    pub logind: Result<(), String>,
    /// How global shortcuts would be bound: `wayland-portal` or `x11-grab`; `None` without a
    /// display server.
    pub shortcut_path: Option<&'static str>,
}

fn from_result<T>(
    feature: Feature,
    result: &Result<T, String>,
    reason: UnavailableReason,
) -> FeatureStatus {
    match result {
        Ok(_) => FeatureStatus::available(feature),
        Err(detail) => FeatureStatus::unavailable(feature, reason, Some(detail.clone())),
    }
}

/// The server capability that means it draws action buttons.
pub const ACTIONS_CAPABILITY: &str = "actions";

/// The `notificationActions` status on Linux: available when the notification server lists the
/// `actions` capability.
pub fn linux_actions_status(
    notify: &FeatureStatus,
    capabilities: &Result<Vec<String>, String>,
) -> FeatureStatus {
    let feature = Feature::NotificationActions;
    if !notify.available {
        return FeatureStatus {
            feature,
            ..notify.clone()
        };
    }
    match capabilities {
        Ok(list) if list.iter().any(|name| name == ACTIONS_CAPABILITY) => {
            FeatureStatus::available(feature)
        }
        Ok(_) => FeatureStatus::unavailable(
            feature,
            UnavailableReason::ActionsUnsupported,
            Some(
                "the notification server does not list the actions capability, so only the default click may be offered"
                    .to_string(),
            ),
        ),
        Err(detail) => FeatureStatus::unavailable(
            feature,
            UnavailableReason::ActionsUnsupported,
            Some(format!("the server's capabilities could not be read: {detail}")),
        ),
    }
}

/// The status of a Linux session.
pub fn linux_status(probes: &LinuxProbes, file_manager_owned: bool) -> PluginStatus {
    let session = |feature| {
        from_result(
            feature,
            &probes.session_bus,
            UnavailableReason::NoSessionBus,
        )
    };
    let notify = match (&probes.session_bus, &probes.notify_server) {
        (Err(detail), _) => FeatureStatus::unavailable(
            Feature::Notify,
            UnavailableReason::NoSessionBus,
            Some(detail.clone()),
        ),
        (Ok(()), result) => from_result(
            Feature::Notify,
            result,
            UnavailableReason::NoNotificationServer,
        ),
    };
    let notify_actions = linux_actions_status(&notify, &probes.notify_capabilities);
    let shortcuts = match probes.shortcut_path {
        Some(path) => FeatureStatus {
            detail: Some(path.to_string()),
            ..FeatureStatus::available(Feature::GlobalShortcuts)
        },
        None => FeatureStatus::unavailable(
            Feature::GlobalShortcuts,
            UnavailableReason::NoDisplayServer,
            None,
        ),
    };
    PluginStatus::new(
        vec![
            notify,
            notify_actions,
            from_result(
                Feature::InhibitSleep,
                &probes.logind,
                UnavailableReason::NoLogind,
            ),
            session(Feature::LauncherProgress),
            session(Feature::FileManager),
            shortcuts,
        ],
        file_manager_owned,
    )
}

/// The status of Windows, given the process's AppUserModelID if it has one.
pub fn windows_status(app_user_model_id: Option<&str>) -> PluginStatus {
    let notify = match app_user_model_id {
        Some(_) => FeatureStatus::available(Feature::Notify),
        None => FeatureStatus::unavailable(
            Feature::Notify,
            UnavailableReason::NeedsAppId,
            Some("the process has no explicit AppUserModelID".to_string()),
        ),
    };
    PluginStatus::new(
        vec![
            notify,
            FeatureStatus::unavailable(
                Feature::NotificationActions,
                UnavailableReason::ActionsUnsupported,
                Some("toast buttons are not implemented yet".to_string()),
            ),
            FeatureStatus::available(Feature::InhibitSleep),
            FeatureStatus::available(Feature::LauncherProgress),
            FeatureStatus::unavailable(
                Feature::FileManager,
                UnavailableReason::PlatformUnsupported,
                Some("org.freedesktop.FileManager1 is a D-Bus service".to_string()),
            ),
            FeatureStatus::available(Feature::GlobalShortcuts),
        ],
        false,
    )
}

/// The status of a platform with none of the services.
pub fn unsupported_status() -> PluginStatus {
    PluginStatus::new(
        FEATURES
            .iter()
            .map(|feature| {
                FeatureStatus::unavailable(*feature, UnavailableReason::PlatformUnsupported, None)
            })
            .collect(),
        false,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn working() -> LinuxProbes {
        LinuxProbes {
            session_bus: Ok(()),
            notify_server: Ok("GNOME Shell".into()),
            notify_capabilities: Ok(vec!["body".into(), "actions".into()]),
            logind: Ok(()),
            shortcut_path: Some("wayland-portal"),
        }
    }

    #[test]
    fn a_working_linux_session_has_every_feature() {
        let status = linux_status(&working(), true);
        assert!(status.available);
        assert!(status.file_manager_owned);
        assert!(status.features.iter().all(|f| f.available));
        assert_eq!(
            status
                .feature(Feature::GlobalShortcuts)
                .unwrap()
                .detail
                .as_deref(),
            Some("wayland-portal")
        );
    }

    #[test]
    fn explains_each_missing_linux_service() {
        let mut probes = working();
        probes.notify_server = Err("no server".into());
        probes.logind = Err("no logind".into());
        probes.shortcut_path = None;
        let status = linux_status(&probes, false);
        assert_eq!(
            status.feature(Feature::Notify).unwrap().reason,
            Some(UnavailableReason::NoNotificationServer)
        );
        assert_eq!(
            status.feature(Feature::InhibitSleep).unwrap().reason,
            Some(UnavailableReason::NoLogind)
        );
        assert_eq!(
            status.feature(Feature::GlobalShortcuts).unwrap().reason,
            Some(UnavailableReason::NoDisplayServer)
        );
        assert!(status.feature(Feature::FileManager).unwrap().available);
    }

    #[test]
    fn actions_need_the_actions_capability() {
        let mut probes = working();
        probes.notify_capabilities = Ok(vec!["body".into(), "persistence".into()]);
        let status = linux_status(&probes, false);
        let actions = status.feature(Feature::NotificationActions).unwrap();
        assert!(!actions.available);
        assert_eq!(actions.reason, Some(UnavailableReason::ActionsUnsupported));
        assert!(actions.detail.as_deref().unwrap().contains("default click"));
        assert!(status.feature(Feature::Notify).unwrap().available);

        probes.notify_capabilities = Err("timed out".into());
        let actions = linux_status(&probes, false)
            .feature(Feature::NotificationActions)
            .unwrap()
            .clone();
        assert!(!actions.available);
        assert!(actions.detail.unwrap().contains("timed out"));

        probes.notify_server = Err("no server".into());
        let actions = linux_status(&probes, false)
            .feature(Feature::NotificationActions)
            .unwrap()
            .clone();
        assert_eq!(
            actions.reason,
            Some(UnavailableReason::NoNotificationServer)
        );
    }

    #[test]
    fn no_session_bus_takes_the_bus_features_with_it() {
        let mut probes = working();
        probes.session_bus = Err("no bus".into());
        let status = linux_status(&probes, false);
        for feature in [
            Feature::Notify,
            Feature::NotificationActions,
            Feature::LauncherProgress,
            Feature::FileManager,
        ] {
            assert_eq!(
                status.feature(feature).unwrap().reason,
                Some(UnavailableReason::NoSessionBus),
                "{feature:?}"
            );
        }
        assert!(status.feature(Feature::InhibitSleep).unwrap().available);
    }

    #[test]
    fn windows_notifications_need_an_app_id() {
        let without = windows_status(None);
        assert_eq!(
            without.feature(Feature::Notify).unwrap().reason,
            Some(UnavailableReason::NeedsAppId)
        );
        assert!(
            windows_status(Some("ca.liminalhq.waypoint"))
                .feature(Feature::Notify)
                .unwrap()
                .available
        );
        assert_eq!(
            without.feature(Feature::FileManager).unwrap().reason,
            Some(UnavailableReason::PlatformUnsupported)
        );
    }

    #[test]
    fn an_unsupported_platform_reports_one_reason_for_all() {
        let status = unsupported_status();
        assert!(!status.available);
        assert_eq!(status.reason, Some(UnavailableReason::PlatformUnsupported));
        assert_eq!(status.features.len(), FEATURES.len());
    }
}
