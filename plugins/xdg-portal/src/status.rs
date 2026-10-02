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
        .map(|(feature, _)| {
            FeatureStatus::unavailable(*feature, UnavailableReason::PlatformUnsupported, None)
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
        features.push(match result {
            Ok(version) => FeatureStatus::available(feature, Some(version)),
            Err(error) => FeatureStatus::unavailable(
                feature,
                reason_for(&error, sandboxed),
                Some(error.message),
            ),
        });
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
        assert_eq!(status.features.len(), 3);
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
}
