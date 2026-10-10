// Hardware-independent checks for raw effects, matching what the Android plugin rejects
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::{models::*, Error, Result};

const EFFECT_IDS: &[&str] = &["click", "double_click", "tick", "heavy_click"];
const PRIMITIVE_IDS: &[&str] = &[
    "tick",
    "low_tick",
    "click",
    "thud",
    "spin",
    "quick_rise",
    "slow_rise",
];

fn invalid<T>(message: impl Into<String>) -> Result<T> {
    Err(Error::InvalidRequest(message.into()))
}

/// Rejects a request the Android plugin would reject whatever the device can do.
pub fn validate_request(req: &EffectRequest) -> Result<()> {
    match &req.effect {
        Effect::Oneshot {
            duration_ms,
            amplitude,
        } => {
            if *duration_ms == 0 {
                return invalid("durationMs must be positive");
            }
            if matches!(amplitude, Some(a) if !(1..=255).contains(a)) {
                return invalid("amplitude must be within 1..255");
            }
        }
        Effect::Waveform {
            timings_ms,
            amplitudes,
            ..
        } => {
            if timings_ms.is_empty() {
                return invalid("timingsMs cannot be empty");
            }
            if timings_ms.iter().all(|t| *t == 0) {
                return invalid("at least one timing must be non-zero");
            }
            if matches!(amplitudes, Some(a) if a.len() != timings_ms.len()) {
                return invalid("amplitudes must have same length as timingsMs");
            }
        }
        Effect::Predefined { effect_id } => {
            if !EFFECT_IDS.contains(&effect_id.to_lowercase().as_str()) {
                return invalid(format!(
                    "Unknown predefined effect `{}`. Use one of {}",
                    effect_id.to_lowercase(),
                    EFFECT_IDS.join(", ")
                ));
            }
        }
        Effect::Composition { steps } => {
            for (i, step) in steps.iter().enumerate() {
                let CompositionStep::Primitive { primitive, .. } = step;
                if !PRIMITIVE_IDS.contains(&primitive.to_lowercase().as_str()) {
                    return invalid(format!(
                        "steps[{i}]: unknown primitive `{}`",
                        primitive.to_lowercase()
                    ));
                }
            }
        }
        Effect::EnvelopeWaveform { control_points, .. } => {
            if control_points.is_empty() {
                return invalid("controlPoints cannot be empty");
            }
            for (i, p) in control_points.iter().enumerate() {
                if !(0.0..=1.0).contains(&p.amplitude) {
                    return invalid(format!("controlPoints[{i}]: amplitude must be within 0..1"));
                }
                if !p.frequency_hz.is_finite() || p.frequency_hz <= 0.0 {
                    return invalid(format!("controlPoints[{i}]: frequencyHz must be positive"));
                }
                if p.duration_ms == 0 {
                    return invalid(format!("controlPoints[{i}]: durationMs must be positive"));
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(effect: serde_json::Value) -> EffectRequest {
        serde_json::from_value(serde_json::json!({ "effect": effect })).expect("deserialize")
    }

    #[test]
    fn accepts_a_valid_request_of_each_type() {
        for effect in [
            serde_json::json!({ "type": "oneshot", "durationMs": 20, "amplitude": 200 }),
            serde_json::json!({ "type": "waveform", "timingsMs": [0, 20], "amplitudes": [0, 255] }),
            serde_json::json!({ "type": "predefined", "effectId": "click" }),
            serde_json::json!({ "type": "composition", "steps": [{ "kind": "primitive", "primitive": "tick" }] }),
            serde_json::json!({ "type": "envelopeWaveform", "controlPoints": [
                { "amplitude": 0.5, "frequencyHz": 120.0, "durationMs": 20 }
            ] }),
        ] {
            assert!(validate_request(&request(effect)).is_ok());
        }
    }

    #[test]
    fn rejects_input_the_android_plugin_rejects() {
        for effect in [
            serde_json::json!({ "type": "oneshot", "durationMs": 0 }),
            serde_json::json!({ "type": "oneshot", "durationMs": 20, "amplitude": 0 }),
            serde_json::json!({ "type": "waveform", "timingsMs": [] }),
            serde_json::json!({ "type": "waveform", "timingsMs": [0, 0] }),
            serde_json::json!({ "type": "waveform", "timingsMs": [10, 10], "amplitudes": [255] }),
            serde_json::json!({ "type": "predefined", "effectId": "pop" }),
            serde_json::json!({ "type": "composition", "steps": [{ "kind": "primitive", "primitive": "pop" }] }),
            serde_json::json!({ "type": "envelopeWaveform", "controlPoints": [] }),
            serde_json::json!({ "type": "envelopeWaveform", "controlPoints": [
                { "amplitude": 2.0, "frequencyHz": 120.0, "durationMs": 20 }
            ] }),
        ] {
            assert!(matches!(
                validate_request(&request(effect)),
                Err(Error::InvalidRequest(_))
            ));
        }
    }
}
