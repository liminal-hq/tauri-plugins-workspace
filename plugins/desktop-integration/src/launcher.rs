// Builds the launcher entry payload and the taskbar plan for a progress request
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::{
    error::ServiceError,
    models::{LauncherProgress, LauncherRequest},
};

/// The interface of the signal docks and shells listen for.
pub const LAUNCHER_INTERFACE: &str = "com.canonical.Unity.LauncherEntry";

/// The number of steps the Windows progress bar is divided into.
pub const TASKBAR_TOTAL: u64 = 1000;

/// The desktop id without a `.desktop` suffix.
pub fn desktop_id_of(raw: &str) -> String {
    raw.strip_suffix(".desktop").unwrap_or(raw).to_string()
}

/// The `application://` URI that identifies the app's launcher to docks.
pub fn app_uri(desktop_id: &str) -> String {
    format!("application://{}.desktop", desktop_id_of(desktop_id))
}

/// The object path the signal is emitted from: the launcher-entry convention is a path under
/// `/com/canonical/unity/launcherentry/` ending in a number, which docks ignore in favour of the
/// app URI, so a stable hash of the id is enough.
pub fn object_path(desktop_id: &str) -> String {
    let hash = desktop_id_of(desktop_id)
        .bytes()
        .fold(0x811c_9dc5_u32, |hash, byte| {
            (hash ^ u32::from(byte)).wrapping_mul(0x0100_0193)
        });
    format!("/com/canonical/unity/launcherentry/{hash}")
}

/// A value of the `Update` signal's property dictionary.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Property {
    Double(f64),
    Bool(bool),
    Int(i64),
}

/// The properties of the `Update` signal for a request.
///
/// Launchers have no indeterminate state, so it shows an empty, visible bar. A request without a
/// count leaves the badge as it was; a count of zero hides it.
pub fn properties(
    request: &LauncherRequest,
) -> Result<Vec<(&'static str, Property)>, ServiceError> {
    let mut properties = match request.progress {
        LauncherProgress::Value { value } => {
            check_fraction(value)?;
            vec![
                ("progress", Property::Double(value)),
                ("progress-visible", Property::Bool(true)),
            ]
        }
        LauncherProgress::Indeterminate => vec![
            ("progress", Property::Double(0.0)),
            ("progress-visible", Property::Bool(true)),
        ],
        LauncherProgress::Cleared => vec![
            ("progress", Property::Double(0.0)),
            ("progress-visible", Property::Bool(false)),
        ],
    };
    if let Some(count) = request.count {
        if count < 0 {
            return Err(ServiceError::invalid("the count must not be negative"));
        }
        properties.push(("count", Property::Int(count)));
        properties.push(("count-visible", Property::Bool(count > 0)));
    }
    Ok(properties)
}

fn check_fraction(value: f64) -> Result<(), ServiceError> {
    if value.is_finite() && (0.0..=1.0).contains(&value) {
        Ok(())
    } else {
        Err(ServiceError::invalid(
            "the progress value must be a number from 0 to 1",
        ))
    }
}

/// `TBPF_NOPROGRESS`.
pub const TBPF_NOPROGRESS: u32 = 0x0;
/// `TBPF_INDETERMINATE`.
pub const TBPF_INDETERMINATE: u32 = 0x1;
/// `TBPF_NORMAL`.
pub const TBPF_NORMAL: u32 = 0x2;

/// What to tell `ITaskbarList3` for a progress request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaskbarPlan {
    /// A `TBPF_*` flag for `SetProgressState`.
    pub state: u32,
    /// The `ullCompleted` of `SetProgressValue`; unused for the states without a value.
    pub completed: u64,
}

pub fn taskbar_plan(progress: &LauncherProgress) -> Result<TaskbarPlan, ServiceError> {
    Ok(match *progress {
        LauncherProgress::Value { value } => {
            check_fraction(value)?;
            TaskbarPlan {
                state: TBPF_NORMAL,
                completed: (value * TASKBAR_TOTAL as f64).round() as u64,
            }
        }
        LauncherProgress::Indeterminate => TaskbarPlan {
            state: TBPF_INDETERMINATE,
            completed: 0,
        },
        LauncherProgress::Cleared => TaskbarPlan {
            state: TBPF_NOPROGRESS,
            completed: 0,
        },
    })
}

/// Picks the window for a request that names none, from `(label, focused)` pairs: the focused
/// window, else the first label starting with `main`, else the first label in alphabetical order. The windows
/// arrive in no particular order, so this does not depend on it.
pub fn default_window<'a>(windows: &[(&'a str, bool)]) -> Option<&'a str> {
    let mut labels: Vec<&str> = windows.iter().map(|(label, _)| *label).collect();
    labels.sort_unstable();
    windows
        .iter()
        .filter(|(_, focused)| *focused)
        .map(|(label, _)| *label)
        .min()
        .or_else(|| {
            labels
                .iter()
                .copied()
                .find(|label| label.starts_with("main"))
        })
        .or_else(|| labels.first().copied())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ServiceErrorKind;

    #[test]
    fn the_default_window_does_not_depend_on_the_order_given() {
        let windows = [("settings", false), ("main-2", false), ("main", false)];
        let mut reversed = windows;
        reversed.reverse();
        assert_eq!(default_window(&windows), Some("main"));
        assert_eq!(default_window(&reversed), Some("main"));
    }

    #[test]
    fn the_focused_window_wins_and_the_first_label_is_the_last_resort() {
        assert_eq!(
            default_window(&[("main", false), ("shelf", true)]),
            Some("shelf")
        );
        assert_eq!(
            default_window(&[("zeta", false), ("beta", false)]),
            Some("beta")
        );
        assert_eq!(default_window(&[]), None);
    }

    fn request(progress: LauncherProgress, count: Option<i64>) -> LauncherRequest {
        LauncherRequest {
            progress,
            count,
            desktop_id: None,
            window_label: None,
        }
    }

    #[test]
    fn normalises_the_desktop_id() {
        assert_eq!(
            desktop_id_of("ca.liminalhq.waypoint.desktop"),
            "ca.liminalhq.waypoint"
        );
        assert_eq!(
            desktop_id_of("ca.liminalhq.waypoint"),
            "ca.liminalhq.waypoint"
        );
        assert_eq!(
            app_uri("ca.liminalhq.waypoint"),
            "application://ca.liminalhq.waypoint.desktop"
        );
        assert_eq!(app_uri("a.desktop"), "application://a.desktop");
    }

    #[test]
    fn the_object_path_is_stable_and_numeric() {
        let path = object_path("ca.liminalhq.waypoint");
        assert_eq!(path, object_path("ca.liminalhq.waypoint.desktop"));
        let number = path
            .strip_prefix("/com/canonical/unity/launcherentry/")
            .unwrap();
        assert!(number.parse::<u32>().is_ok(), "{path}");
        assert_ne!(path, object_path("other"));
    }

    #[test]
    fn a_value_shows_a_visible_bar() {
        let p = properties(&request(LauncherProgress::Value { value: 0.25 }, None)).unwrap();
        assert_eq!(
            p,
            vec![
                ("progress", Property::Double(0.25)),
                ("progress-visible", Property::Bool(true))
            ]
        );
    }

    #[test]
    fn cleared_hides_the_bar_and_a_count_is_optional() {
        let p = properties(&request(LauncherProgress::Cleared, Some(0))).unwrap();
        assert_eq!(
            p,
            vec![
                ("progress", Property::Double(0.0)),
                ("progress-visible", Property::Bool(false)),
                ("count", Property::Int(0)),
                ("count-visible", Property::Bool(false)),
            ]
        );
        let p = properties(&request(LauncherProgress::Cleared, Some(3))).unwrap();
        assert!(p.contains(&("count-visible", Property::Bool(true))));
    }

    #[test]
    fn indeterminate_shows_an_empty_visible_bar() {
        let p = properties(&request(LauncherProgress::Indeterminate, None)).unwrap();
        assert_eq!(p[1], ("progress-visible", Property::Bool(true)));
    }

    #[test]
    fn rejects_bad_values_and_counts() {
        for value in [-0.1, 1.1, f64::NAN, f64::INFINITY] {
            let error = properties(&request(LauncherProgress::Value { value }, None)).unwrap_err();
            assert_eq!(error.kind, ServiceErrorKind::InvalidArgument);
            assert!(taskbar_plan(&LauncherProgress::Value { value }).is_err());
        }
        assert!(properties(&request(LauncherProgress::Cleared, Some(-1))).is_err());
    }

    #[test]
    fn plans_the_taskbar_states() {
        assert_eq!(
            taskbar_plan(&LauncherProgress::Value { value: 0.5 }).unwrap(),
            TaskbarPlan {
                state: TBPF_NORMAL,
                completed: 500
            }
        );
        assert_eq!(
            taskbar_plan(&LauncherProgress::Value { value: 1.0 })
                .unwrap()
                .completed,
            1000
        );
        assert_eq!(
            taskbar_plan(&LauncherProgress::Indeterminate)
                .unwrap()
                .state,
            TBPF_INDETERMINATE
        );
        assert_eq!(
            taskbar_plan(&LauncherProgress::Cleared).unwrap().state,
            TBPF_NOPROGRESS
        );
    }
}
