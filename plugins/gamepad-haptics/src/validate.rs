// Rules every request must meet, whatever the pad or platform
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::{models::*, Error, Result};

/// The most frames one `play_frames` call may carry.
pub const MAX_FRAMES: usize = 512;

fn invalid<T>(message: impl Into<String>) -> Result<T> {
    Err(Error::InvalidRequest(message.into()))
}

fn level_ok(level: f64) -> bool {
    level.is_finite() && (0.0..=1.0).contains(&level)
}

/// Rejects frames no pad could play.
pub fn validate_frames(frames: &[Frame], limits: &Limits) -> Result<()> {
    if frames.is_empty() {
        return invalid("frames cannot be empty");
    }
    if frames.len() > MAX_FRAMES {
        return invalid(format!("frames exceeds the maximum of {MAX_FRAMES}"));
    }
    let mut total: u64 = 0;
    for (i, frame) in frames.iter().enumerate() {
        if frame.duration_ms == 0 {
            return invalid(format!("frames[{i}]: durationMs must be positive"));
        }
        let levels = [
            ("heavy", Some(frame.heavy)),
            ("light", Some(frame.light)),
            ("leftTrigger", frame.left_trigger),
            ("rightTrigger", frame.right_trigger),
        ];
        for (name, level) in levels {
            if matches!(level, Some(l) if !level_ok(l)) {
                return invalid(format!("frames[{i}]: {name} must be within 0..1"));
            }
        }
        total = total.saturating_add(frame.duration_ms);
    }
    if total > limits.max_duration_ms {
        return invalid(format!(
            "frames last {total} ms, over the limit of {} ms",
            limits.max_duration_ms
        ));
    }
    Ok(())
}

/// Rejects a master scale outside 0..1.
pub fn validate_scale(scale: Option<f64>) -> Result<()> {
    if matches!(scale, Some(s) if !level_ok(s)) {
        return invalid("scale must be within 0..1");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn limits() -> Limits {
        Limits {
            max_duration_ms: 1_000,
            max_continuous_ms: 500,
        }
    }

    fn frame(duration_ms: u64, heavy: f64, light: f64) -> Frame {
        Frame {
            duration_ms,
            heavy,
            light,
            left_trigger: None,
            right_trigger: None,
        }
    }

    #[test]
    fn accepts_valid_frames() {
        assert!(validate_frames(&[frame(20, 0.5, 1.0), frame(30, 0.0, 0.0)], &limits()).is_ok());
    }

    #[test]
    fn rejects_frames_no_pad_could_play() {
        let nan = frame(10, f64::NAN, 0.0);
        let mut trigger = frame(10, 0.0, 0.0);
        trigger.right_trigger = Some(1.5);
        let too_many = vec![frame(1, 0.1, 0.1); MAX_FRAMES + 1];
        for (frames, fragment) in [
            (vec![], "cannot be empty"),
            (too_many, "exceeds the maximum"),
            (vec![frame(0, 0.5, 0.5)], "durationMs must be positive"),
            (vec![frame(10, 1.5, 0.0)], "heavy must be within"),
            (vec![nan], "heavy must be within"),
            (vec![trigger], "rightTrigger must be within"),
            (vec![frame(1_001, 0.5, 0.5)], "over the limit"),
        ] {
            assert!(
                matches!(validate_frames(&frames, &limits()), Err(Error::InvalidRequest(m)) if m.contains(fragment)),
                "{fragment}"
            );
        }
    }

    #[test]
    fn scale_must_be_within_range() {
        assert!(validate_scale(None).is_ok());
        assert!(validate_scale(Some(0.0)).is_ok());
        assert!(validate_scale(Some(-0.1)).is_err());
        assert!(validate_scale(Some(f64::NAN)).is_err());
    }
}
