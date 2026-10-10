// Hardware-independent rules for raw effects and step lists, applied on every platform
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::{models::*, Error, Result};

/// The most steps one `play_steps` call may schedule.
pub const MAX_STEPS: usize = 512;

fn invalid<T>(message: impl Into<String>) -> Result<T> {
    Err(Error::InvalidRequest(message.into()))
}

/// Rejects a request that no device could play, whatever its hardware.
///
/// `budget_ms` is the time the request may use. A one-shot or waveform longer than that is
/// truncated later; an envelope cannot be shortened safely, so it must already fit.
pub fn validate_request(req: &EffectRequest, budget_ms: u64) -> Result<()> {
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
            repeat,
        } => {
            if timings_ms.is_empty() {
                return invalid("timingsMs cannot be empty");
            }
            if timings_ms.iter().all(|t| *t == 0) {
                return invalid("at least one timing must be non-zero");
            }
            if let Some(amplitudes) = amplitudes {
                if amplitudes.len() != timings_ms.len() {
                    return invalid("amplitudes must have same length as timingsMs");
                }
                if amplitudes.iter().any(|a| *a > 255) {
                    return invalid("amplitudes must be within 0..255");
                }
            }
            if matches!(repeat, Some(r) if *r < -1 || i64::from(*r) >= timings_ms.len() as i64) {
                return invalid("repeat must be -1 or an index into timingsMs");
            }
        }
        Effect::Predefined { effect_id } => {
            let id = effect_id.to_lowercase();
            if !EFFECT_IDS.contains(&id.as_str()) {
                return invalid(format!(
                    "Unknown predefined effect `{id}`. Use one of {}; for a thud use the `thud` composition primitive.",
                    EFFECT_IDS.join(", ")
                ));
            }
        }
        Effect::Composition { steps } => {
            for (i, step) in steps.iter().enumerate() {
                let CompositionStep::Primitive {
                    primitive, scale, ..
                } = step;
                let id = primitive.to_lowercase();
                if !PRIMITIVE_IDS.contains(&id.as_str()) {
                    return invalid(format!("steps[{i}]: unknown primitive `{id}`"));
                }
                if matches!(scale, Some(s) if !(0.0..=1.0).contains(s)) {
                    return invalid(format!("steps[{i}]: scale must be within 0..1"));
                }
            }
        }
        Effect::EnvelopeWaveform {
            initial_frequency_hz,
            control_points,
        } => {
            if matches!(initial_frequency_hz, Some(f) if !f.is_finite() || *f <= 0.0) {
                return invalid("initialFrequencyHz must be positive");
            }
            if control_points.is_empty() {
                return invalid("controlPoints cannot be empty");
            }
            let mut total: u64 = 0;
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
                total = total.saturating_add(p.duration_ms);
            }
            if total > budget_ms {
                return invalid(format!(
                    "envelope duration {total} ms exceeds the limit of {budget_ms} ms"
                ));
            }
        }
    }
    Ok(())
}

/// Rejects a step list the platform could not schedule: empty, too long, a step that starts at or
/// past the duration limit, or a step whose request is invalid within what is left of the limit.
pub fn validate_steps(steps: &[CompiledStep], limits: &Limits) -> Result<()> {
    if steps.is_empty() {
        return invalid("steps cannot be empty");
    }
    if steps.len() > MAX_STEPS {
        return invalid(format!("steps exceeds the maximum of {MAX_STEPS}"));
    }
    for (i, step) in steps.iter().enumerate() {
        if step.at_ms >= limits.max_duration_ms {
            return invalid(format!(
                "steps[{i}]: atMs {} is not below the limit of {} ms",
                step.at_ms, limits.max_duration_ms
            ));
        }
        validate_request(&step.request, limits.max_duration_ms - step.at_ms).map_err(
            |e| match e {
                Error::InvalidRequest(message) => {
                    Error::InvalidRequest(format!("steps[{i}]: {message}"))
                }
                other => other,
            },
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(effect: serde_json::Value) -> EffectRequest {
        serde_json::from_value(serde_json::json!({ "effect": effect })).expect("deserialize")
    }

    fn limits() -> Limits {
        Limits {
            max_duration_ms: 1_000,
            max_amplitude: 255,
            allow_repeating_waveforms: false,
        }
    }

    #[test]
    fn accepts_a_valid_request_of_each_type() {
        for effect in [
            serde_json::json!({ "type": "oneshot", "durationMs": 20, "amplitude": 200 }),
            serde_json::json!({ "type": "waveform", "timingsMs": [0, 20], "amplitudes": [0, 255], "repeat": 1 }),
            serde_json::json!({ "type": "predefined", "effectId": "click" }),
            serde_json::json!({ "type": "composition", "steps": [{ "kind": "primitive", "primitive": "tick", "scale": 0.5 }] }),
            serde_json::json!({ "type": "envelopeWaveform", "controlPoints": [
                { "amplitude": 0.5, "frequencyHz": 120.0, "durationMs": 20 }
            ] }),
        ] {
            assert!(validate_request(&request(effect), 1_000).is_ok());
        }
    }

    #[test]
    fn rejects_input_no_device_could_play() {
        for effect in [
            serde_json::json!({ "type": "oneshot", "durationMs": 0 }),
            serde_json::json!({ "type": "oneshot", "durationMs": 20, "amplitude": 0 }),
            serde_json::json!({ "type": "waveform", "timingsMs": [] }),
            serde_json::json!({ "type": "waveform", "timingsMs": [0, 0] }),
            serde_json::json!({ "type": "waveform", "timingsMs": [10, 10], "amplitudes": [255] }),
            serde_json::json!({ "type": "waveform", "timingsMs": [10, 10], "amplitudes": [300, 0] }),
            serde_json::json!({ "type": "waveform", "timingsMs": [10, 10], "repeat": 2 }),
            serde_json::json!({ "type": "waveform", "timingsMs": [10, 10], "repeat": -2 }),
            serde_json::json!({ "type": "predefined", "effectId": "pop" }),
            serde_json::json!({ "type": "composition", "steps": [{ "kind": "primitive", "primitive": "pop" }] }),
            serde_json::json!({ "type": "composition", "steps": [{ "kind": "primitive", "primitive": "tick", "scale": 1.5 }] }),
            serde_json::json!({ "type": "envelopeWaveform", "controlPoints": [] }),
            serde_json::json!({ "type": "envelopeWaveform", "initialFrequencyHz": 0.0, "controlPoints": [
                { "amplitude": 0.5, "frequencyHz": 120.0, "durationMs": 20 }
            ] }),
            serde_json::json!({ "type": "envelopeWaveform", "controlPoints": [
                { "amplitude": 2.0, "frequencyHz": 120.0, "durationMs": 20 }
            ] }),
        ] {
            assert!(matches!(
                validate_request(&request(effect), 1_000),
                Err(Error::InvalidRequest(_))
            ));
        }
    }

    #[test]
    fn an_envelope_longer_than_the_budget_is_rejected() {
        let envelope = request(
            serde_json::json!({ "type": "envelopeWaveform", "controlPoints": [
            { "amplitude": 0.5, "frequencyHz": 120.0, "durationMs": 600 },
            { "amplitude": 0.5, "frequencyHz": 120.0, "durationMs": 600 }
        ] }),
        );
        assert!(validate_request(&envelope, 1_200).is_ok());
        assert!(matches!(
            validate_request(&envelope, 1_199),
            Err(Error::InvalidRequest(m)) if m.contains("exceeds the limit")
        ));
    }

    #[test]
    fn step_lists_follow_the_android_rules() {
        let step = |at_ms: u64, effect: serde_json::Value| CompiledStep {
            at_ms,
            request: request(effect),
        };
        let click = serde_json::json!({ "type": "predefined", "effectId": "click" });

        assert!(validate_steps(&[step(0, click.clone())], &limits()).is_ok());
        assert!(matches!(
            validate_steps(&[], &limits()),
            Err(Error::InvalidRequest(m)) if m.contains("cannot be empty")
        ));
        assert!(matches!(
            validate_steps(&[step(1_000, click.clone())], &limits()),
            Err(Error::InvalidRequest(m)) if m.contains("steps[0]: atMs 1000")
        ));

        let many = vec![step(0, click.clone()); MAX_STEPS + 1];
        assert!(matches!(
            validate_steps(&many, &limits()),
            Err(Error::InvalidRequest(m)) if m.contains("exceeds the maximum")
        ));

        let bad = serde_json::json!({ "type": "oneshot", "durationMs": 0 });
        assert!(matches!(
            validate_steps(&[step(0, click), step(10, bad)], &limits()),
            Err(Error::InvalidRequest(m)) if m.starts_with("steps[1]: ")
        ));
    }
}
