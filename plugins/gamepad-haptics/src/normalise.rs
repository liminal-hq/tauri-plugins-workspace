// Turns a validated request into what a pad will play: scaled, downgraded to the pad's tier and capped
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::{
    models::*,
    validate::{validate_frames, validate_scale},
    Result,
};

/// A value that has passed validation and capping. Backends accept only this type, so an
/// unchecked request cannot reach them.
#[derive(Debug, Clone, PartialEq)]
pub struct Normalised<T>(T);

impl<T> Normalised<T> {
    pub fn get(&self) -> &T {
        &self.0
    }

    pub fn into_inner(self) -> T {
        self.0
    }

    #[cfg(test)]
    pub fn unchecked(value: T) -> Self {
        Self(value)
    }
}

/// What to play on one pad.
#[derive(Debug, Clone, PartialEq)]
pub struct PlannedPlay {
    pub pad_id: String,
    pub frames: Vec<Frame>,
    /// The tier the frames need, no higher than the pad's own.
    pub tier: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Plan {
    /// Nothing will play; the reason says why.
    Silent(String),
    Play {
        play: Normalised<PlannedPlay>,
        /// Why the play differs from the request. Empty when it played as asked.
        reasons: Vec<String>,
    },
}

/// What a pad needs to play a request: 3 for trigger motors, 2 when both body motors are used and
/// differ somewhere, else 1. A pattern that only ever drives one motor needs one motor.
pub fn request_tier(frames: &[Frame]) -> u8 {
    let uses_triggers = frames
        .iter()
        .any(|f| f.left_trigger.unwrap_or(0.0) > 0.0 || f.right_trigger.unwrap_or(0.0) > 0.0);
    let heavy_used = frames.iter().any(|f| f.heavy > 0.0);
    let light_used = frames.iter().any(|f| f.light > 0.0);
    if uses_triggers {
        3
    } else if heavy_used && light_used && frames.iter().any(|f| f.heavy != f.light) {
        2
    } else {
        1
    }
}

/// Multiplies every level by `scale`.
pub fn apply_scale(frames: &mut [Frame], scale: f64) {
    for frame in frames {
        frame.heavy = (frame.heavy * scale).clamp(0.0, 1.0);
        frame.light = (frame.light * scale).clamp(0.0, 1.0);
        frame.left_trigger = frame.left_trigger.map(|l| (l * scale).clamp(0.0, 1.0));
        frame.right_trigger = frame.right_trigger.map(|l| (l * scale).clamp(0.0, 1.0));
    }
}

fn is_silent(frame: &Frame) -> bool {
    frame.heavy == 0.0
        && frame.light == 0.0
        && frame.left_trigger.unwrap_or(0.0) == 0.0
        && frame.right_trigger.unwrap_or(0.0) == 0.0
}

fn silence(frame: &mut Frame) {
    frame.heavy = 0.0;
    frame.light = 0.0;
    frame.left_trigger = frame.left_trigger.map(|_| 0.0);
    frame.right_trigger = frame.right_trigger.map(|_| 0.0);
}

/// Cuts any run of non-silent frames longer than `max_continuous_ms`, keeping the timing of
/// everything after it. Returns whether anything was cut.
pub fn cap_continuous(frames: Vec<Frame>, max_continuous_ms: u64) -> (Vec<Frame>, bool) {
    let mut out = Vec::with_capacity(frames.len() + 1);
    let mut run: u64 = 0;
    let mut cut = false;
    let mut cutting = false;
    for frame in frames {
        if is_silent(&frame) {
            run = 0;
            cutting = false;
            out.push(frame);
            continue;
        }
        if cutting {
            let mut silent = frame;
            silence(&mut silent);
            out.push(silent);
            continue;
        }
        if run + frame.duration_ms <= max_continuous_ms {
            run += frame.duration_ms;
            out.push(frame);
            continue;
        }
        cut = true;
        cutting = true;
        let allowed = max_continuous_ms - run;
        if allowed > 0 {
            let mut head = frame.clone();
            head.duration_ms = allowed;
            out.push(head);
        }
        let mut tail = frame;
        tail.duration_ms -= allowed;
        silence(&mut tail);
        out.push(tail);
    }
    (out, cut)
}

/// Folds a request down to what a pad with `top_tier` can play.
fn downgrade(frames: &mut [Frame], top_tier: u8, reasons: &mut Vec<String>) {
    if top_tier < 3
        && frames
            .iter()
            .any(|f| f.left_trigger.is_some() || f.right_trigger.is_some())
    {
        for frame in frames.iter_mut() {
            frame.left_trigger = None;
            frame.right_trigger = None;
        }
        reasons.push("Pad has no trigger motors".to_string());
    }
    if top_tier < 2 && frames.iter().any(|f| f.heavy != f.light) {
        for frame in frames.iter_mut() {
            frame.heavy = frame.heavy.max(frame.light);
            frame.light = 0.0;
        }
        reasons.push("Pad has one motor, so both were merged into one".to_string());
    }
}

/// Validates `args`, then decides what `pad` plays. Validation comes first, so a scale of 0 or a
/// pad that cannot play never lets an invalid request through.
pub fn plan_play(
    args: &PlayFramesArgs,
    pad: &PadInfo,
    limits: &Limits,
    default_scale: f64,
) -> Result<Plan> {
    validate_frames(&args.frames, limits)?;
    validate_scale(args.scale)?;

    let scale = default_scale * args.scale.unwrap_or(1.0);
    if scale == 0.0 {
        return Ok(Plan::Silent("Master scale is 0".to_string()));
    }
    if pad.top_tier == 0 {
        return Ok(Plan::Silent(
            pad.reason
                .clone()
                .unwrap_or_else(|| "Pad cannot play rumble".to_string()),
        ));
    }

    let mut reasons = Vec::new();
    let mut frames = args.frames.clone();
    apply_scale(&mut frames, scale);
    downgrade(&mut frames, pad.top_tier, &mut reasons);
    let (frames, cut) = cap_continuous(frames, limits.max_continuous_ms);
    if cut {
        reasons.push(format!(
            "Capped continuous rumble at {} ms",
            limits.max_continuous_ms
        ));
    }
    if frames.iter().all(is_silent) {
        return Ok(Plan::Silent(
            "Nothing left to play after scaling".to_string(),
        ));
    }

    let tier = request_tier(&frames).min(pad.top_tier);
    Ok(Plan::Play {
        play: Normalised(PlannedPlay {
            pad_id: pad.id.clone(),
            frames,
            tier,
        }),
        reasons,
    })
}

/// Merges planner reasons into a result, but only when something played.
pub fn merge_reasons(mut result: PlayResult, reasons: &[String]) -> PlayResult {
    if result.tier > 0 && !reasons.is_empty() {
        result.downgraded = true;
        let joined = reasons.join("; ");
        result.reason = Some(match result.reason {
            Some(existing) => format!("{existing}; {joined}"),
            None => joined,
        });
    }
    result
}

#[cfg(test)]
pub(crate) fn test_pad(top_tier: u8) -> PadInfo {
    PadInfo {
        id: "gamepad:0".into(),
        slot: 0,
        name: "Test pad".into(),
        vendor_id: 0x054c,
        product_id: 0x05c4,
        serial: None,
        transport: Transport::Usb,
        guid: "030000004c050000c405000000000000".into(),
        motors: if top_tier >= 2 { 2 } else { 1 },
        triggers: top_tier >= 3,
        top_tier,
        reason: None,
        backend: "mock".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn limits() -> Limits {
        Limits {
            max_duration_ms: 3_000,
            max_continuous_ms: 100,
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

    fn args(frames: Vec<Frame>, scale: Option<f64>) -> PlayFramesArgs {
        PlayFramesArgs {
            pad_id: "gamepad:0".into(),
            frames,
            scale,
        }
    }

    fn played(plan: Plan) -> (PlannedPlay, Vec<String>) {
        match plan {
            Plan::Play { play, reasons } => (play.into_inner(), reasons),
            Plan::Silent(r) => panic!("expected a play, got silent: {r}"),
        }
    }

    #[test]
    fn plays_a_valid_request_as_asked() {
        let plan = plan_play(
            &args(vec![frame(50, 0.5, 1.0)], None),
            &test_pad(2),
            &limits(),
            1.0,
        )
        .expect("plan");
        let (play, reasons) = played(plan);
        assert_eq!(play.tier, 2);
        assert!(reasons.is_empty());
        assert_eq!(play.frames, vec![frame(50, 0.5, 1.0)]);
    }

    #[test]
    fn a_pattern_that_drives_one_motor_is_tier_one() {
        let (play, _) = played(
            plan_play(
                &args(vec![frame(50, 1.0, 0.0)], None),
                &test_pad(2),
                &limits(),
                1.0,
            )
            .unwrap(),
        );
        assert_eq!(play.tier, 1);
        assert_eq!(play.frames, vec![frame(50, 1.0, 0.0)]);
    }

    #[test]
    fn invalid_input_is_rejected_even_when_it_would_play_nothing() {
        let bad = args(vec![frame(0, 0.5, 0.5)], Some(0.0));
        assert!(plan_play(&bad, &test_pad(2), &limits(), 1.0).is_err());
        assert!(plan_play(&bad, &test_pad(0), &limits(), 1.0).is_err());
    }

    #[test]
    fn scale_zero_and_dead_pads_are_silent() {
        let ok = args(vec![frame(50, 1.0, 1.0)], Some(0.0));
        assert_eq!(
            plan_play(&ok, &test_pad(2), &limits(), 1.0).unwrap(),
            Plan::Silent("Master scale is 0".into())
        );
        let mut dead = test_pad(0);
        dead.reason = Some("No write access".into());
        assert_eq!(
            plan_play(
                &args(vec![frame(50, 1.0, 1.0)], None),
                &dead,
                &limits(),
                1.0
            )
            .unwrap(),
            Plan::Silent("No write access".into())
        );
    }

    #[test]
    fn a_single_motor_pad_merges_the_motors() {
        let (play, reasons) = played(
            plan_play(
                &args(vec![frame(50, 0.25, 0.75)], None),
                &test_pad(1),
                &limits(),
                1.0,
            )
            .unwrap(),
        );
        assert_eq!(play.frames, vec![frame(50, 0.75, 0.0)]);
        assert_eq!(play.tier, 1);
        assert_eq!(reasons.len(), 1);
    }

    #[test]
    fn triggers_are_dropped_on_a_pad_without_them() {
        let mut f = frame(50, 0.5, 0.5);
        f.right_trigger = Some(1.0);
        let (play, reasons) =
            played(plan_play(&args(vec![f], None), &test_pad(2), &limits(), 1.0).unwrap());
        assert_eq!(play.frames[0].right_trigger, None);
        assert_eq!(play.tier, 1);
        assert!(reasons[0].contains("trigger"));
    }

    #[test]
    fn long_runs_are_cut_and_later_timing_kept() {
        let frames = vec![
            frame(80, 1.0, 1.0),
            frame(80, 1.0, 1.0),
            frame(20, 0.0, 0.0),
            frame(30, 1.0, 1.0),
        ];
        let (play, reasons) =
            played(plan_play(&args(frames, None), &test_pad(2), &limits(), 1.0).unwrap());
        let total: u64 = play.frames.iter().map(|f| f.duration_ms).sum();
        assert_eq!(total, 210);
        assert_eq!(play.frames[1], frame(20, 1.0, 1.0));
        assert_eq!(play.frames[2], frame(60, 0.0, 0.0));
        assert_eq!(play.frames[4], frame(30, 1.0, 1.0));
        assert_eq!(
            reasons,
            vec!["Capped continuous rumble at 100 ms".to_string()]
        );
    }

    #[test]
    fn scaling_rounds_nothing_away() {
        let (play, _) = played(
            plan_play(
                &args(vec![frame(50, 0.5, 0.5)], Some(0.5)),
                &test_pad(2),
                &limits(),
                1.0,
            )
            .unwrap(),
        );
        assert_eq!(play.frames[0].heavy, 0.25);
    }

    #[test]
    fn reasons_merge_only_when_something_played() {
        let reasons = vec!["Capped".to_string()];
        let silent = merge_reasons(PlayResult::silent("gamepad:0", "No write access"), &reasons);
        assert_eq!(silent.reason.as_deref(), Some("No write access"));

        let mut played = PlayResult::silent("gamepad:0", "x");
        played.tier = 2;
        played.reason = None;
        let merged = merge_reasons(played, &reasons);
        assert!(merged.downgraded);
        assert_eq!(merged.reason.as_deref(), Some("Capped"));
    }
}
