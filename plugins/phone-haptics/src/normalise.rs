// Turns a raw request into one a platform can play: validated first, then scaled and capped
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::{
    models::*,
    validate::{validate_request, validate_steps},
    Result,
};

const REASON_SEPARATOR: &str = " · ";

/// The global controls a caller may set: the master scale and a ceiling on the tier that plays.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct RawControls {
    pub scale: Option<f64>,
    pub max_tier: Option<u8>,
}

impl RawControls {
    /// The master scale, where a missing or non-finite value means full strength.
    fn scale(&self) -> f64 {
        match self.scale {
            Some(s) if s.is_finite() => s.clamp(0.0, 1.0),
            _ => 1.0,
        }
    }
}

/// What the device can do, which the tier cap needs.
#[derive(Debug, Clone, Copy)]
pub struct TierInfo {
    pub top_tier: u8,
    pub has_amplitude_control: bool,
}

/// A value that has been validated and capped, with every change made along the way.
///
/// It can only be built here, so a platform implementation cannot be handed raw input.
#[derive(Debug)]
pub struct Normalised<T> {
    // Only the Android bridge forwards the value; the desktop stub just resolves.
    #[cfg_attr(not(target_os = "android"), allow(dead_code))]
    value: T,
    reasons: Vec<String>,
}

impl<T> Normalised<T> {
    #[cfg_attr(not(any(target_os = "android", test)), allow(dead_code))]
    pub fn value(&self) -> &T {
        &self.value
    }

    pub fn reasons(&self) -> &[String] {
        &self.reasons
    }

    #[cfg(test)]
    pub(crate) fn unchecked(value: T) -> Self {
        Self {
            value,
            reasons: Vec::new(),
        }
    }
}

/// Either the request resolves without reaching the platform, or this is what to forward.
#[derive(Debug)]
pub enum Plan<T> {
    Silent(PlayResult),
    Forward(Normalised<T>),
}

/// The tier a request plays at on a device, before any downgrade the hardware forces.
///
/// A one-shot or waveform with no amplitude is the on/off form, which is tier 1 on any device;
/// one that names amplitudes is tier 2 where the motor has amplitude control.
pub fn effect_tier(effect: &Effect, info: &TierInfo) -> u8 {
    let amplitude_tier = if info.has_amplitude_control { 2 } else { 1 };
    match effect {
        Effect::EnvelopeWaveform { .. } => 4,
        Effect::Composition { .. } => 3,
        Effect::Predefined { .. } => info.top_tier.min(3),
        Effect::Oneshot {
            amplitude: None, ..
        }
        | Effect::Waveform {
            amplitudes: None, ..
        } => 1,
        Effect::Oneshot { .. } | Effect::Waveform { .. } => amplitude_tier,
    }
}

/// Applies the master scale to a request's amplitude fields.
pub fn apply_scale(mut req: EffectRequest, scale: f64) -> EffectRequest {
    if (scale - 1.0).abs() < f64::EPSILON {
        return req;
    }
    match &mut req.effect {
        Effect::Oneshot { amplitude, .. } => {
            let scaled = (f64::from(amplitude.unwrap_or(255)) * scale).round();
            *amplitude = Some(scaled.max(1.0) as u16);
        }
        Effect::Waveform {
            amplitudes: Some(amplitudes),
            ..
        } => {
            for a in amplitudes.iter_mut() {
                *a = (f64::from(*a) * scale).round() as u16;
            }
        }
        Effect::Composition { steps } => {
            for step in steps.iter_mut() {
                let CompositionStep::Primitive { scale: s, .. } = step;
                *s = Some((f64::from(s.unwrap_or(1.0)) * scale).clamp(0.0, 1.0) as f32);
            }
        }
        Effect::EnvelopeWaveform { control_points, .. } => {
            for p in control_points.iter_mut() {
                p.amplitude = (f64::from(p.amplitude) * scale).clamp(0.0, 1.0) as f32;
            }
        }
        Effect::Waveform { .. } | Effect::Predefined { .. } => {}
    }
    req
}

/// Says why the master scale did nothing to a request it cannot scale, if that is the case.
pub(crate) fn scale_note(effect: &Effect, scale: f64) -> Option<&'static str> {
    if (scale - 1.0).abs() < f64::EPSILON {
        return None;
    }
    match effect {
        Effect::Predefined { .. } => Some("Master scale does not apply to predefined effects"),
        Effect::Waveform {
            amplitudes: None, ..
        } => Some("Master scale does not apply to on/off waveforms"),
        _ => None,
    }
}

/// Applies `scale`, unless that would lift the request above `max_tier`, as it does when it gives a
/// one-shot an amplitude. Then the request plays unscaled at the tier that was asked for.
fn scale_within_tier(
    req: EffectRequest,
    scale: f64,
    cap: Option<(u8, &TierInfo)>,
    reasons: &mut Vec<String>,
) -> EffectRequest {
    if let Some(note) = scale_note(&req.effect, scale) {
        push_unique(reasons, note.to_string());
    }
    let scaled = apply_scale(req.clone(), scale);
    match cap {
        Some((max_tier, info)) if effect_tier(&scaled.effect, info) > max_tier => {
            push_unique(
                reasons,
                format!("Master scale does not apply at tier {max_tier}"),
            );
            req
        }
        _ => scaled,
    }
}

/// Brings a valid request within the limits, returning what it changed. It never fails: whatever
/// could not be fixed by shortening was already rejected by validation.
pub(crate) fn cap_request(
    mut req: EffectRequest,
    limits: &Limits,
    budget_ms: u64,
) -> (EffectRequest, Vec<String>) {
    let budget = budget_ms.max(1);
    let mut reasons = Vec::new();
    match &mut req.effect {
        Effect::Oneshot {
            duration_ms,
            amplitude,
        } => {
            if *duration_ms > budget {
                *duration_ms = budget;
                reasons.push(format!("Truncated to {budget} ms"));
            }
            if let Some(a) = amplitude {
                *a = (*a).min(limits.max_amplitude);
            }
        }
        Effect::Waveform {
            timings_ms,
            amplitudes,
            repeat,
        } => {
            if matches!(repeat, Some(r) if *r >= 0) && !limits.allow_repeating_waveforms {
                *repeat = Some(-1);
                reasons.push("Repeat ignored: allowRepeatingWaveforms is false".to_string());
            }
            let total: u64 = timings_ms.iter().fold(0, |t, v| t.saturating_add(*v));
            if total > budget {
                cap_timings(timings_ms, budget);
                reasons.push(format!("Truncated to {budget} ms"));
            }
            if let Some(amplitudes) = amplitudes {
                for a in amplitudes.iter_mut() {
                    *a = (*a).min(limits.max_amplitude);
                }
            }
        }
        Effect::Predefined { .. }
        | Effect::Composition { .. }
        | Effect::EnvelopeWaveform { .. } => {}
    }
    (req, reasons)
}

/// Shortens `timings` so they add up to at most `budget`, zeroing everything after the cut.
fn cap_timings(timings: &mut [u64], budget: u64) {
    let mut total: u64 = 0;
    for timing in timings.iter_mut() {
        let remaining = budget.saturating_sub(total);
        *timing = (*timing).min(remaining);
        total += *timing;
    }
}

const NO_STRENGTH: &str = "Nothing in this request has any strength, so nothing plays";

/// Whether a capped request would leave the motor idle, however long it runs: every amplitude,
/// scale or control point is zero, or the only non-zero timings are off phases.
fn plays_nothing(effect: &Effect) -> bool {
    match effect {
        Effect::Waveform {
            timings_ms,
            amplitudes: Some(amplitudes),
            ..
        } => !timings_ms
            .iter()
            .zip(amplitudes)
            .any(|(timing, amplitude)| *timing > 0 && *amplitude > 0),
        Effect::Waveform {
            timings_ms,
            amplitudes: None,
            ..
        } => !timings_ms
            .iter()
            .skip(1)
            .step_by(2)
            .any(|timing| *timing > 0),
        Effect::Composition { steps } => {
            !steps.is_empty()
                && steps.iter().all(|step| {
                    let CompositionStep::Primitive { scale, .. } = step;
                    matches!(scale, Some(s) if *s <= 0.0)
                })
        }
        Effect::EnvelopeWaveform { control_points, .. } => {
            control_points.iter().all(|point| point.amplitude <= 0.0)
        }
        Effect::Oneshot { .. } | Effect::Predefined { .. } => false,
    }
}

fn push_unique(reasons: &mut Vec<String>, reason: String) {
    if !reasons.contains(&reason) {
        reasons.push(reason);
    }
}

/// Validates a `play` request, then applies the controls and the limits.
///
/// Validation runs on the request as it was sent, so neither a zero scale nor a tier cap can hide
/// an invalid request, and scaling can never turn one into a valid one. `tier_info` is only called
/// when a tier cap is set.
pub fn plan_play(
    req: EffectRequest,
    controls: &RawControls,
    limits: &Limits,
    tier_info: impl FnOnce() -> Result<TierInfo>,
) -> Result<Plan<PlayArgs>> {
    validate_request(&req, limits.max_duration_ms)?;

    let scale = controls.scale();
    if scale == 0.0 {
        return Ok(Plan::Silent(PlayResult::silent(
            "Master scale is 0, so nothing plays",
        )));
    }
    let info = match controls.max_tier {
        Some(_) => Some(tier_info()?),
        None => None,
    };
    let cap = controls.max_tier.zip(info.as_ref());
    if let Some((max_tier, info)) = cap {
        if effect_tier(&req.effect, info) > max_tier {
            return Ok(Plan::Silent(PlayResult::silent(&format!(
                "Capped at tier {max_tier} by setMaxTier"
            ))));
        }
    }

    let mut reasons = Vec::new();
    let scaled = scale_within_tier(req, scale, cap, &mut reasons);
    let (req, capped) = cap_request(scaled, limits, limits.max_duration_ms);
    reasons.extend(capped);
    if plays_nothing(&req.effect) {
        return Ok(Plan::Silent(PlayResult::silent(NO_STRENGTH)));
    }
    Ok(Plan::Forward(Normalised {
        value: PlayArgs {
            req,
            budget_ms: limits.max_duration_ms,
        },
        reasons,
    }))
}

/// Validates a `play_steps` list, then applies the controls and gives each step the duration left
/// after its start offset.
pub fn plan_steps(
    steps: Vec<CompiledStep>,
    controls: &RawControls,
    limits: &Limits,
    tier_info: impl FnOnce() -> Result<TierInfo>,
) -> Result<Plan<PlayStepsArgs>> {
    validate_steps(&steps, limits)?;

    let scale = controls.scale();
    if scale == 0.0 {
        return Ok(Plan::Silent(PlayResult::silent(
            "Master scale is 0, so nothing plays",
        )));
    }
    let info = match controls.max_tier {
        Some(_) => Some(tier_info()?),
        None => None,
    };
    let cap = controls.max_tier.zip(info.as_ref());
    if let Some((max_tier, info)) = cap {
        if steps
            .iter()
            .any(|s| effect_tier(&s.request.effect, info) > max_tier)
        {
            return Ok(Plan::Silent(PlayResult::silent(&format!(
                "Capped at tier {max_tier} by setMaxTier"
            ))));
        }
    }

    let mut reasons = Vec::new();
    let planned: Vec<PlannedStep> = steps
        .into_iter()
        .map(|step| {
            let budget_ms = limits.max_duration_ms - step.at_ms;
            let mut changes = Vec::new();
            let scaled = scale_within_tier(step.request, scale, cap, &mut changes);
            let (request, capped) = cap_request(scaled, limits, budget_ms);
            changes.extend(capped);
            for reason in changes {
                push_unique(&mut reasons, reason);
            }
            PlannedStep {
                at_ms: step.at_ms,
                budget_ms,
                request,
            }
        })
        // A step that would leave the motor idle is not scheduled.
        .filter(|step| !plays_nothing(&step.request.effect))
        .collect();
    if planned.is_empty() {
        return Ok(Plan::Silent(PlayResult::silent(NO_STRENGTH)));
    }

    Ok(Plan::Forward(Normalised {
        value: PlayStepsArgs { steps: planned },
        reasons,
    }))
}

/// Adds what the Rust layer changed to the platform's result, but only when something played, so a
/// silent result keeps the reason the platform gave.
pub fn merge_reasons(rust: &[String], mut native: PlayResult) -> PlayResult {
    if rust.is_empty() || native.tier == 0 {
        return native;
    }
    let mut merged: Vec<String> = Vec::new();
    for reason in rust {
        push_unique(&mut merged, reason.clone());
    }
    if let Some(existing) = &native.reason {
        for reason in existing.split(REASON_SEPARATOR) {
            push_unique(&mut merged, reason.to_string());
        }
    }
    let joined = merged.join(REASON_SEPARATOR);
    native.downgraded = true;
    native.reason = Some(joined.clone());
    native.downgrade_reason = Some(joined);
    native
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

    fn device() -> Result<TierInfo> {
        Ok(TierInfo {
            top_tier: 4,
            has_amplitude_control: true,
        })
    }

    fn forwarded(plan: Plan<PlayArgs>) -> Normalised<PlayArgs> {
        match plan {
            Plan::Forward(n) => n,
            Plan::Silent(r) => panic!("expected a forwarded request, got silent: {:?}", r.reason),
        }
    }

    #[test]
    fn a_one_shot_is_truncated_to_the_limit_and_says_so() {
        let req = request(serde_json::json!({ "type": "oneshot", "durationMs": 5_000 }));
        let n = forwarded(plan_play(req, &RawControls::default(), &limits(), device).unwrap());
        assert!(matches!(
            n.value().req.effect,
            Effect::Oneshot {
                duration_ms: 1_000,
                ..
            }
        ));
        assert_eq!(n.reasons(), ["Truncated to 1000 ms"]);
    }

    #[test]
    fn repeat_is_dropped_unless_the_config_allows_it() {
        let req =
            request(serde_json::json!({ "type": "waveform", "timingsMs": [10, 10], "repeat": 0 }));
        let n =
            forwarded(plan_play(req.clone(), &RawControls::default(), &limits(), device).unwrap());
        assert!(matches!(
            n.value().req.effect,
            Effect::Waveform {
                repeat: Some(-1),
                ..
            }
        ));
        assert_eq!(
            n.reasons(),
            ["Repeat ignored: allowRepeatingWaveforms is false"]
        );

        let allowed = Limits {
            allow_repeating_waveforms: true,
            ..limits()
        };
        let n = forwarded(plan_play(req, &RawControls::default(), &allowed, device).unwrap());
        assert!(matches!(
            n.value().req.effect,
            Effect::Waveform {
                repeat: Some(0),
                ..
            }
        ));
    }

    #[test]
    fn a_waveform_is_cut_at_the_budget_and_the_rest_zeroed() {
        let req =
            request(serde_json::json!({ "type": "waveform", "timingsMs": [600, 300, 600, 50] }));
        let n = forwarded(plan_play(req, &RawControls::default(), &limits(), device).unwrap());
        match &n.value().req.effect {
            Effect::Waveform { timings_ms, .. } => assert_eq!(timings_ms, &[600, 300, 100, 0]),
            _ => panic!("expected a waveform"),
        }
    }

    #[test]
    fn invalid_input_rejects_whatever_the_controls_are() {
        let bad = request(serde_json::json!({ "type": "envelopeWaveform", "controlPoints": [] }));
        for controls in [
            RawControls {
                scale: Some(0.0),
                max_tier: None,
            },
            RawControls {
                scale: Some(0.5),
                max_tier: None,
            },
            RawControls {
                scale: None,
                max_tier: Some(1),
            },
        ] {
            assert!(plan_play(bad.clone(), &controls, &limits(), device).is_err());
        }
    }

    #[test]
    fn scaling_cannot_make_an_invalid_amplitude_valid() {
        let bad =
            request(serde_json::json!({ "type": "oneshot", "durationMs": 20, "amplitude": 0 }));
        let controls = RawControls {
            scale: Some(0.5),
            max_tier: None,
        };
        assert!(plan_play(bad, &controls, &limits(), device).is_err());
    }

    #[test]
    fn the_on_off_form_of_an_effect_is_tier_one_on_any_device() {
        let info = TierInfo {
            top_tier: 3,
            has_amplitude_control: true,
        };
        let tier = |effect: serde_json::Value| effect_tier(&request(effect).effect, &info);

        assert_eq!(
            tier(serde_json::json!({ "type": "oneshot", "durationMs": 20 })),
            1
        );
        assert_eq!(
            tier(serde_json::json!({ "type": "waveform", "timingsMs": [0, 20] })),
            1
        );
        assert_eq!(
            tier(serde_json::json!({ "type": "oneshot", "durationMs": 20, "amplitude": 200 })),
            2
        );
        assert_eq!(
            tier(
                serde_json::json!({ "type": "waveform", "timingsMs": [0, 20], "amplitudes": [0, 200] })
            ),
            2
        );
        let no_amplitude = TierInfo {
            top_tier: 1,
            has_amplitude_control: false,
        };
        assert_eq!(
            effect_tier(
                &request(
                    serde_json::json!({ "type": "oneshot", "durationMs": 20, "amplitude": 200 })
                )
                .effect,
                &no_amplitude
            ),
            1
        );
    }

    #[test]
    fn a_zero_scale_and_a_tier_cap_resolve_silently() {
        let click = request(serde_json::json!({ "type": "predefined", "effectId": "click" }));
        let zero = RawControls {
            scale: Some(0.0),
            max_tier: None,
        };
        match plan_play(click, &zero, &limits(), device).unwrap() {
            Plan::Silent(r) => assert_eq!(
                r.reason.as_deref(),
                Some("Master scale is 0, so nothing plays")
            ),
            Plan::Forward(_) => panic!("expected silent"),
        }

        let composition = request(serde_json::json!({
            "type": "composition", "steps": [{ "kind": "primitive", "primitive": "click" }]
        }));
        let capped = RawControls {
            scale: None,
            max_tier: Some(2),
        };
        match plan_play(composition, &capped, &limits(), device).unwrap() {
            Plan::Silent(r) => {
                assert_eq!(r.reason.as_deref(), Some("Capped at tier 2 by setMaxTier"))
            }
            Plan::Forward(_) => panic!("expected silent"),
        }
    }

    #[test]
    fn scaling_never_lifts_a_request_above_the_tier_cap() {
        // An on/off one-shot has no amplitude to scale; giving it one would make it tier 2.
        let req = request(serde_json::json!({ "type": "oneshot", "durationMs": 40 }));
        let controls = RawControls {
            scale: Some(0.5),
            max_tier: Some(1),
        };
        let n = forwarded(plan_play(req, &controls, &limits(), device).unwrap());
        assert!(matches!(
            n.value().req.effect,
            Effect::Oneshot {
                amplitude: None,
                ..
            }
        ));
        assert_eq!(n.reasons(), ["Master scale does not apply at tier 1"]);
    }

    #[test]
    fn the_master_scale_says_when_it_cannot_apply() {
        let controls = RawControls {
            scale: Some(0.5),
            max_tier: None,
        };
        for (effect, note) in [
            (
                serde_json::json!({ "type": "predefined", "effectId": "heavy_click" }),
                "Master scale does not apply to predefined effects",
            ),
            (
                serde_json::json!({ "type": "waveform", "timingsMs": [10, 20] }),
                "Master scale does not apply to on/off waveforms",
            ),
        ] {
            let n = forwarded(plan_play(request(effect), &controls, &limits(), device).unwrap());
            assert_eq!(n.reasons(), [note]);
        }
        // Nothing to say at full strength.
        let full = RawControls::default();
        let n = forwarded(
            plan_play(
                request(serde_json::json!({ "type": "predefined", "effectId": "click" })),
                &full,
                &limits(),
                device,
            )
            .unwrap(),
        );
        assert!(n.reasons().is_empty());
    }

    #[test]
    fn the_master_scale_rounds_like_the_guest_did() {
        let req =
            request(serde_json::json!({ "type": "oneshot", "durationMs": 20, "amplitude": 200 }));
        let controls = RawControls {
            scale: Some(0.5),
            max_tier: None,
        };
        let n = forwarded(plan_play(req, &controls, &limits(), device).unwrap());
        assert!(matches!(
            n.value().req.effect,
            Effect::Oneshot {
                amplitude: Some(100),
                ..
            }
        ));
    }

    #[test]
    fn a_request_with_no_strength_resolves_silently() {
        let silent = |effect: serde_json::Value, scale: Option<f64>| {
            let controls = RawControls {
                scale,
                max_tier: None,
            };
            match plan_play(request(effect), &controls, &limits(), device).unwrap() {
                Plan::Silent(r) => r.reason,
                Plan::Forward(_) => None,
            }
        };
        let reason = Some(NO_STRENGTH.to_string());
        let waveform = serde_json::json!({
            "type": "waveform", "timingsMs": [0, 100], "amplitudes": [0, 0]
        });
        assert_eq!(silent(waveform, None), reason);
        let rounds_to_zero = serde_json::json!({
            "type": "waveform", "timingsMs": [0, 100], "amplitudes": [0, 1]
        });
        assert_eq!(silent(rounds_to_zero, Some(0.4)), reason);
        let off_only = serde_json::json!({ "type": "waveform", "timingsMs": [100, 0] });
        assert_eq!(silent(off_only, None), reason);
        let composition = serde_json::json!({
            "type": "composition", "steps": [{ "kind": "primitive", "primitive": "click", "scale": 0.0 }]
        });
        assert_eq!(silent(composition, None), reason);
        let playable = serde_json::json!({
            "type": "waveform", "timingsMs": [0, 100], "amplitudes": [0, 200]
        });
        assert_eq!(silent(playable, None), None);
    }

    #[test]
    fn steps_with_no_strength_are_left_out() {
        let step = |at_ms: u64, effect: serde_json::Value| CompiledStep {
            at_ms,
            request: request(effect),
        };
        let quiet =
            serde_json::json!({ "type": "waveform", "timingsMs": [0, 50], "amplitudes": [0, 0] });
        let click = serde_json::json!({ "type": "predefined", "effectId": "click" });

        match plan_steps(
            vec![step(0, quiet.clone()), step(20, click)],
            &RawControls::default(),
            &limits(),
            device,
        )
        .unwrap()
        {
            Plan::Forward(n) => assert_eq!(n.value().steps.len(), 1),
            Plan::Silent(_) => panic!("expected one scheduled step"),
        }
        assert!(matches!(
            plan_steps(
                vec![step(0, quiet)],
                &RawControls::default(),
                &limits(),
                device
            )
            .unwrap(),
            Plan::Silent(_)
        ));
    }

    #[test]
    fn each_step_gets_what_is_left_after_its_offset() {
        let steps = vec![CompiledStep {
            at_ms: 990,
            request: request(serde_json::json!({ "type": "oneshot", "durationMs": 50 })),
        }];
        match plan_steps(steps, &RawControls::default(), &limits(), device).unwrap() {
            Plan::Forward(n) => {
                let step = &n.value().steps[0];
                assert_eq!(step.budget_ms, 10);
                assert!(matches!(
                    step.request.effect,
                    Effect::Oneshot {
                        duration_ms: 10,
                        ..
                    }
                ));
                assert_eq!(n.reasons(), ["Truncated to 10 ms"]);
            }
            Plan::Silent(_) => panic!("expected forwarded steps"),
        }
    }

    #[test]
    fn rust_reasons_join_the_platform_reason_only_when_something_played() {
        let rust = vec!["Truncated to 10 ms".to_string()];

        let mut played = PlayResult::silent("Device lacks amplitude control");
        played.tier = 1;
        let merged = merge_reasons(&rust, played);
        assert_eq!(
            merged.reason.as_deref(),
            Some("Truncated to 10 ms · Device lacks amplitude control")
        );
        assert_eq!(merged.reason, merged.downgrade_reason);

        let silent = PlayResult::silent("No vibrator on this platform");
        let kept = merge_reasons(&rust, silent);
        assert_eq!(kept.reason.as_deref(), Some("No vibrator on this platform"));
    }
}
