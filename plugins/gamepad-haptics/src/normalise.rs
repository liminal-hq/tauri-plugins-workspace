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

/// How long a light motor that only switches on and off must run for a person to feel it. A
/// DualShock 3 does not spin up in 40 ms; 100 ms is felt.
pub const MIN_LIGHT_PULSE_MS: u64 = 100;

/// The time step shaping works in.
const CELL_MS: u64 = 10;

/// Shapes the light motor for a pad whose light motor only switches on and off. Levels in between
/// become pulses of at least `min_pulse_ms` whose share of the time matches the level, a short
/// strong onset becomes one full pulse, and a level too faint to ever fill a pulse is dropped.
/// A pulse that runs past the end of the frames extends them, up to `max_total_ms`. The other
/// motors and the timing of everything else are unchanged.
pub fn shape_binary_light(frames: &[Frame], min_pulse_ms: u64, max_total_ms: u64) -> Vec<Frame> {
    let mut out: Vec<Frame> = Vec::new();
    let mut push = |template: &Frame, duration_ms: u64, light: f64| {
        let next = Frame {
            duration_ms,
            light,
            ..template.clone()
        };
        match out.last_mut() {
            Some(last) if same_levels(last, &next) => last.duration_ms += duration_ms,
            _ => out.push(next),
        }
    };

    let min = min_pulse_ms as f64;
    let mut credit = 0.0_f64;
    let mut on_left: u64 = 0;
    let mut previous_light = 0.0_f64;
    let mut elapsed: u64 = 0;
    let mut last_template = None;

    for frame in frames {
        let mut left = frame.duration_ms;
        while left > 0 {
            let step = left.min(CELL_MS);
            left -= step;
            elapsed += step;
            let want = frame.light;

            if on_left == 0 {
                if want == 0.0 {
                    credit = 0.0;
                } else {
                    // A strong onset starts a pulse at once; weaker levels build up to one.
                    if want >= 0.5 && previous_light < 0.5 {
                        credit = credit.max(min);
                    }
                    credit += want * step as f64;
                    if credit >= min {
                        credit -= min;
                        on_left = min_pulse_ms;
                    }
                }
            } else {
                credit += want * step as f64;
            }

            let light = if on_left > 0 {
                on_left = on_left.saturating_sub(step);
                1.0
            } else {
                0.0
            };
            push(frame, step, light);
            previous_light = want;
        }
        last_template = Some(frame);
    }

    // A pulse still running at the end carries on past the last frame.
    if on_left > 0 {
        if let Some(template) = last_template {
            let room = max_total_ms.saturating_sub(elapsed);
            let extra = on_left.min(room);
            if extra > 0 {
                let silent = Frame {
                    heavy: 0.0,
                    left_trigger: template.left_trigger.map(|_| 0.0),
                    right_trigger: template.right_trigger.map(|_| 0.0),
                    ..template.clone()
                };
                push(&silent, extra, 1.0);
            }
        }
    }
    out
}

/// The shortest heavy pulse a weak heavy motor needs, and the strength below which it applies.
pub const MIN_HEAVY_PULSE_MS: u64 = 90;
pub const HEAVY_FLOOR_BELOW: f64 = 0.7;

/// Lengthens short, soft heavy-motor pulses for a pad whose heavy motor cannot spin up for them.
/// A run of frames with the heavy motor on, shorter than `min_pulse_ms` and never reaching
/// `below`, keeps its last heavy level into the silent heavy time after it until it is long
/// enough. The other motors and the timing of everything else are unchanged. A pulse at the end
/// extends the frames, up to `max_total_ms`.
pub fn lengthen_soft_heavy(
    frames: &[Frame],
    min_pulse_ms: u64,
    below: f64,
    max_total_ms: u64,
) -> Vec<Frame> {
    // Work in cells so a frame can be split where the pulse ends.
    let mut cells: Vec<Frame> = Vec::new();
    for frame in frames {
        let mut left = frame.duration_ms;
        while left > 0 {
            let step = left.min(CELL_MS);
            left -= step;
            cells.push(Frame {
                duration_ms: step,
                ..frame.clone()
            });
        }
    }

    let mut i = 0;
    while i < cells.len() {
        if cells[i].heavy == 0.0 {
            i += 1;
            continue;
        }
        let mut peak = 0.0_f64;
        let mut length = 0;
        while i < cells.len() && cells[i].heavy > 0.0 {
            peak = peak.max(cells[i].heavy);
            length += cells[i].duration_ms;
            i += 1;
        }
        if length >= min_pulse_ms || peak >= below {
            continue;
        }
        let level = cells[i - 1].heavy;
        while length < min_pulse_ms && i < cells.len() && cells[i].heavy == 0.0 {
            cells[i].heavy = level;
            length += cells[i].duration_ms;
            i += 1;
        }
        if length < min_pulse_ms && i == cells.len() {
            let total: u64 = cells.iter().map(|c| c.duration_ms).sum();
            let extra = (min_pulse_ms - length).min(max_total_ms.saturating_sub(total));
            if extra > 0 {
                let template = cells[cells.len() - 1].clone();
                cells.push(Frame {
                    duration_ms: extra,
                    heavy: level,
                    light: 0.0,
                    left_trigger: template.left_trigger.map(|_| 0.0),
                    right_trigger: template.right_trigger.map(|_| 0.0),
                });
                // The added tail is part of this pulse, not a new one.
                i = cells.len();
            }
        }
    }

    let mut out: Vec<Frame> = Vec::new();
    for cell in cells {
        match out.last_mut() {
            Some(last) if same_levels(last, &cell) => last.duration_ms += cell.duration_ms,
            _ => out.push(cell),
        }
    }
    out
}

fn same_levels(a: &Frame, b: &Frame) -> bool {
    a.heavy == b.heavy
        && a.light == b.light
        && a.left_trigger == b.left_trigger
        && a.right_trigger == b.right_trigger
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
    if pad.weak_heavy {
        let lengthened = lengthen_soft_heavy(
            &frames,
            MIN_HEAVY_PULSE_MS,
            HEAVY_FLOOR_BELOW,
            limits.max_duration_ms,
        );
        if lengthened != frames {
            reasons.push(
                "Heavy motor needs a longer pulse, so short soft taps were lengthened".to_string(),
            );
            frames = lengthened;
        }
    }
    if pad.light_binary && pad.top_tier >= 2 {
        let shaped = shape_binary_light(&frames, MIN_LIGHT_PULSE_MS, limits.max_duration_ms);
        if shaped != frames {
            reasons.push(
                "Light motor only switches on and off, so it was pulsed to approximate the strength"
                    .to_string(),
            );
            frames = shaped;
        }
    }
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
        light_binary: false,
        weak_heavy: false,
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

    fn binary_pad() -> PadInfo {
        PadInfo {
            light_binary: true,
            ..test_pad(2)
        }
    }

    fn light_runs(frames: &[Frame]) -> Vec<u64> {
        let mut runs = Vec::new();
        let mut run = 0;
        for f in frames {
            if f.light > 0.0 {
                run += f.duration_ms;
            } else if run > 0 {
                runs.push(run);
                run = 0;
            }
        }
        if run > 0 {
            runs.push(run);
        }
        runs
    }

    #[test]
    fn a_short_sharp_tap_on_a_binary_light_motor_becomes_one_full_pulse() {
        let frames = vec![frame(40, 0.0, 1.0), frame(160, 0.0, 0.0)];
        let (play, reasons) =
            played(plan_play(&args(frames, None), &binary_pad(), &limits(), 1.0).unwrap());
        assert_eq!(
            play.frames,
            vec![frame(100, 0.0, 1.0), frame(100, 0.0, 0.0)]
        );
        assert_eq!(reasons.len(), 1);
    }

    #[test]
    fn a_pulse_at_the_end_extends_the_pattern_within_the_limit() {
        let wide = Limits {
            max_duration_ms: 3_000,
            max_continuous_ms: 2_000,
        };
        let (play, _) = played(
            plan_play(
                &args(vec![frame(40, 0.0, 1.0)], None),
                &binary_pad(),
                &wide,
                1.0,
            )
            .unwrap(),
        );
        assert_eq!(play.frames, vec![frame(100, 0.0, 1.0)]);

        let tight = Limits {
            max_duration_ms: 60,
            max_continuous_ms: 60,
        };
        let (play, _) = played(
            plan_play(
                &args(vec![frame(40, 0.0, 1.0)], None),
                &binary_pad(),
                &tight,
                1.0,
            )
            .unwrap(),
        );
        assert_eq!(play.frames, vec![frame(60, 0.0, 1.0)]);
    }

    #[test]
    fn full_and_off_light_levels_pass_through_without_a_reason() {
        let frames = vec![frame(300, 0.0, 1.0), frame(100, 0.0, 0.0)];
        let wide = Limits {
            max_duration_ms: 3_000,
            max_continuous_ms: 2_000,
        };
        let (play, reasons) =
            played(plan_play(&args(frames.clone(), None), &binary_pad(), &wide, 1.0).unwrap());
        assert_eq!(play.frames, frames);
        assert!(reasons.is_empty());
    }

    #[test]
    fn in_between_light_levels_are_pulsed_in_proportion() {
        let wide = Limits {
            max_duration_ms: 3_000,
            max_continuous_ms: 2_000,
        };
        let (play, _) = played(
            plan_play(
                &args(vec![frame(1_000, 0.0, 0.3)], None),
                &binary_pad(),
                &wide,
                1.0,
            )
            .unwrap(),
        );
        let on: u64 = light_runs(&play.frames).iter().sum();
        assert!((250..=400).contains(&on), "on for {on} ms");
        assert!(play.frames.iter().all(|f| f.light == 0.0 || f.light == 1.0));
        assert!(light_runs(&play.frames)
            .iter()
            .all(|r| *r >= MIN_LIGHT_PULSE_MS));
    }

    #[test]
    fn heavy_motor_and_timing_are_untouched_by_shaping() {
        let wide = Limits {
            max_duration_ms: 3_000,
            max_continuous_ms: 2_000,
        };
        let frames: Vec<Frame> = (0..20)
            .map(|i| frame(50, 1.0 - f64::from(i) / 20.0, f64::from(i) / 20.0))
            .collect();
        let (play, _) =
            played(plan_play(&args(frames.clone(), None), &binary_pad(), &wide, 1.0).unwrap());
        let total: u64 = play.frames.iter().map(|f| f.duration_ms).sum();
        assert!((1_000..=1_000 + MIN_LIGHT_PULSE_MS).contains(&total));
        let mut at = 0;
        for f in &frames {
            let mid = at + 25;
            let shaped = {
                let mut t = 0;
                play.frames.iter().find(|p| {
                    t += p.duration_ms;
                    t > mid
                })
            };
            assert_eq!(shaped.unwrap().heavy, f.heavy);
            at += 50;
        }
    }

    fn weak_heavy_pad() -> PadInfo {
        PadInfo {
            weak_heavy: true,
            ..test_pad(2)
        }
    }

    fn wide() -> Limits {
        Limits {
            max_duration_ms: 3_000,
            max_continuous_ms: 2_000,
        }
    }

    #[test]
    fn a_short_soft_heavy_tap_is_lengthened_into_the_silence_after_it() {
        let frames = vec![frame(60, 0.6, 0.0), frame(200, 0.0, 0.0)];
        let (play, reasons) =
            played(plan_play(&args(frames, None), &weak_heavy_pad(), &wide(), 1.0).unwrap());
        assert_eq!(play.frames, vec![frame(90, 0.6, 0.0), frame(170, 0.0, 0.0)]);
        assert_eq!(reasons.len(), 1);
    }

    #[test]
    fn strong_or_long_heavy_taps_and_other_pads_are_left_alone() {
        for frames in [
            vec![frame(60, 1.0, 0.0), frame(200, 0.0, 0.0)],
            vec![frame(90, 0.6, 0.0), frame(200, 0.0, 0.0)],
        ] {
            let (play, reasons) = played(
                plan_play(&args(frames.clone(), None), &weak_heavy_pad(), &wide(), 1.0).unwrap(),
            );
            assert_eq!(play.frames, frames);
            assert!(reasons.is_empty());
        }
        let frames = vec![frame(60, 0.6, 0.0), frame(200, 0.0, 0.0)];
        let (play, _) =
            played(plan_play(&args(frames.clone(), None), &test_pad(2), &wide(), 1.0).unwrap());
        assert_eq!(play.frames, frames);
    }

    #[test]
    fn a_soft_heavy_tap_at_the_end_extends_the_pattern_within_the_limit() {
        let (play, _) = played(
            plan_play(
                &args(vec![frame(60, 0.6, 0.0)], None),
                &weak_heavy_pad(),
                &wide(),
                1.0,
            )
            .unwrap(),
        );
        assert_eq!(play.frames, vec![frame(90, 0.6, 0.0)]);
        let tight = Limits {
            max_duration_ms: 70,
            max_continuous_ms: 70,
        };
        let (play, _) = played(
            plan_play(
                &args(vec![frame(60, 0.6, 0.0)], None),
                &weak_heavy_pad(),
                &tight,
                1.0,
            )
            .unwrap(),
        );
        assert_eq!(play.frames, vec![frame(70, 0.6, 0.0)]);
    }

    #[test]
    fn lengthening_keeps_the_light_motor_and_stops_at_the_next_heavy_pulse() {
        let frames = vec![
            frame(60, 0.6, 0.0),
            frame(10, 0.0, 1.0),
            frame(10, 0.0, 0.0),
            frame(100, 0.5, 0.0),
        ];
        let out = lengthen_soft_heavy(&frames, MIN_HEAVY_PULSE_MS, HEAVY_FLOOR_BELOW, 3_000);
        assert_eq!(
            out,
            vec![
                frame(60, 0.6, 0.0),
                frame(10, 0.6, 1.0),
                frame(10, 0.6, 0.0),
                frame(100, 0.5, 0.0)
            ]
        );
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
