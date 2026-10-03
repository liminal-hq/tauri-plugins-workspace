// Probes which portal interfaces answer and explains the ones that do not
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::{
    error::{ServiceError, ServiceErrorKind},
    models::{FeatureStatus, PortalFeature, PortalStatus, UnavailableReason},
};

/// The portal interface behind each feature.
pub const FEATURES: [(PortalFeature, &str); 3] = [
    (
        PortalFeature::Notification,
        "org.freedesktop.portal.Notification",
    ),
    (PortalFeature::Inhibit, "org.freedesktop.portal.Inhibit"),
    (PortalFeature::OpenUri, "org.freedesktop.portal.OpenURI"),
];

/// The first Notification portal interface version with `buttons`.
pub const BUTTONS_MIN_VERSION: u32 = 1;

/// The `notificationActions` status, derived from what the Notification portal reported.
pub fn actions_status(notification: &FeatureStatus) -> FeatureStatus {
    let feature = PortalFeature::NotificationActions;
    if !notification.available {
        return FeatureStatus {
            feature,
            available: false,
            reason: notification.reason,
            detail: notification.detail.clone(),
            version: notification.version,
        };
    }
    match notification.version {
        Some(version) if version < BUTTONS_MIN_VERSION => FeatureStatus::unavailable(
            feature,
            UnavailableReason::ActionsUnsupported,
            Some(format!(
                "the Notification portal is version {version}; buttons need version {BUTTONS_MIN_VERSION}, so only the default click is offered"
            )),
        ),
        version => FeatureStatus::available(feature, version),
    }
}

/// Why a probe failed, in the terms of [`UnavailableReason`].
pub fn reason_for(error: &ServiceError, sandboxed: bool) -> UnavailableReason {
    let lower = error.message.to_ascii_lowercase();
    match error.kind {
        ServiceErrorKind::UnsupportedPlatform => UnavailableReason::PlatformUnsupported,
        ServiceErrorKind::Timeout => UnavailableReason::NoResponse,
        ServiceErrorKind::NotAllowed if !sandboxed => UnavailableReason::NotSandboxed,
        ServiceErrorKind::PortalUnavailable
            if lower.contains("unknowninterface")
                || lower.contains("unknownobject")
                || lower.contains("no such interface") =>
        {
            UnavailableReason::InterfaceMissing
        }
        _ => UnavailableReason::NoPortal,
    }
}

/// The status of a platform without portals: every feature unavailable for that one reason.
pub fn unsupported_status() -> PortalStatus {
    let features = FEATURES
        .iter()
        .map(|(feature, _)| *feature)
        .chain([PortalFeature::NotificationActions])
        .map(|feature| {
            FeatureStatus::unavailable(feature, UnavailableReason::PlatformUnsupported, None)
        })
        .collect();
    PortalStatus::new(false, features)
}

#[cfg(target_os = "linux")]
pub async fn probe(connection: Option<&ashpd::zbus::Connection>) -> PortalStatus {
    use ashpd::zbus::Proxy;

    use crate::{
        linux::{DESKTOP_DESTINATION, DESKTOP_PATH},
        timeout::with_timeout,
    };

    let sandboxed = ashpd::is_sandboxed();
    let mut features = Vec::new();
    for (feature, interface) in FEATURES {
        let result = match connection {
            None => Err(ServiceError::new(
                ServiceErrorKind::PortalUnavailable,
                "the session bus is not reachable",
            )),
            Some(connection) => {
                let version = async {
                    let proxy =
                        Proxy::new(connection, DESKTOP_DESTINATION, DESKTOP_PATH, interface)
                            .await?;
                    proxy.get_property::<u32>("version").await
                };
                with_timeout(interface, version).await
            }
        };
        let status = match result {
            Ok(version) => FeatureStatus::available(feature, Some(version)),
            Err(error) => FeatureStatus::unavailable(
                feature,
                reason_for(&error, sandboxed),
                Some(error.message),
            ),
        };
        let actions = (feature == PortalFeature::Notification).then(|| actions_status(&status));
        features.push(status);
        features.extend(actions);
    }
    PortalStatus::new(sandboxed, features)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn error(kind: ServiceErrorKind, message: &str) -> ServiceError {
        ServiceError::new(kind, message)
    }

    #[test]
    fn maps_errors_onto_reasons() {
        let bus = error(
            ServiceErrorKind::PortalUnavailable,
            "org.freedesktop.DBus.Error.ServiceUnknown",
        );
        assert_eq!(reason_for(&bus, false), UnavailableReason::NoPortal);

        let interface = error(
            ServiceErrorKind::PortalUnavailable,
            "org.freedesktop.DBus.Error.UnknownInterface",
        );
        assert_eq!(
            reason_for(&interface, false),
            UnavailableReason::InterfaceMissing
        );

        let timeout = ServiceError::timeout("x");
        assert_eq!(reason_for(&timeout, false), UnavailableReason::NoResponse);

        let refused = error(ServiceErrorKind::NotAllowed, "NotAllowed");
        assert_eq!(reason_for(&refused, false), UnavailableReason::NotSandboxed);
        assert_eq!(reason_for(&refused, true), UnavailableReason::NoPortal);

        assert_eq!(
            reason_for(&ServiceError::unsupported(), false),
            UnavailableReason::PlatformUnsupported
        );
    }

    #[test]
    fn unsupported_status_names_every_feature() {
        let status = unsupported_status();
        assert!(!status.available);
        assert_eq!(status.reason, Some(UnavailableReason::PlatformUnsupported));
        assert_eq!(status.features.len(), 4);
        assert!(status.feature(PortalFeature::NotificationActions).is_some());
        assert!(status.feature(PortalFeature::Inhibit).is_some());
    }

    #[test]
    fn available_when_any_feature_works() {
        let status = PortalStatus::new(
            false,
            vec![
                FeatureStatus::available(PortalFeature::Notification, Some(2)),
                FeatureStatus::unavailable(
                    PortalFeature::Inhibit,
                    UnavailableReason::InterfaceMissing,
                    None,
                ),
            ],
        );
        assert!(status.available);
        assert_eq!(status.reason, None);
    }

    #[test]
    fn actions_follow_the_notification_portal_version() {
        let with = |version| {
            actions_status(&FeatureStatus::available(
                PortalFeature::Notification,
                version,
            ))
        };
        for version in [Some(1), Some(2), None] {
            let status = with(version);
            assert!(status.available, "{version:?}");
            assert_eq!(status.feature, PortalFeature::NotificationActions);
        }
        let old = with(Some(0));
        assert!(!old.available);
        assert_eq!(old.reason, Some(UnavailableReason::ActionsUnsupported));
        assert!(old.detail.unwrap().contains("default click"));
    }

    #[test]
    fn actions_are_unavailable_with_the_notification_portal() {
        let status = actions_status(&FeatureStatus::unavailable(
            PortalFeature::Notification,
            UnavailableReason::NoPortal,
            Some("no bus".into()),
        ));
        assert!(!status.available);
        assert_eq!(status.reason, Some(UnavailableReason::NoPortal));
        assert_eq!(status.detail.as_deref(), Some("no bus"));
    }
}
