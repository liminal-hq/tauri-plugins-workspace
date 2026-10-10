// Property tests for validation and planning: whatever the input, the rules hold
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use proptest::{prelude::*, test_runner::RngAlgorithm};

use crate::{
    models::*,
    normalise::{cap_continuous, plan_play, test_pad, Plan},
    validate::validate_frames,
};

fn config() -> ProptestConfig {
    ProptestConfig {
        cases: 256,
        failure_persistence: None,
        rng_algorithm: RngAlgorithm::ChaCha,
        ..ProptestConfig::default()
    }
}

fn limits() -> Limits {
    Limits {
        max_duration_ms: 1_000,
        max_continuous_ms: 200,
    }
}

fn level() -> BoxedStrategy<f64> {
    prop_oneof![
        8 => (0u32..=100).prop_map(|n| f64::from(n) / 100.0),
        1 => Just(f64::NAN),
        1 => Just(1.5),
        1 => Just(-0.5),
    ]
    .boxed()
}

fn valid_level() -> BoxedStrategy<f64> {
    (0u32..=100).prop_map(|n| f64::from(n) / 100.0).boxed()
}

fn frame_with(level: BoxedStrategy<f64>) -> impl Strategy<Value = Frame> {
    (
        prop_oneof![9 => 1u64..=120, 1 => Just(0u64)],
        level.clone(),
        level.clone(),
        proptest::option::of(level.clone()),
        proptest::option::of(level),
    )
        .prop_map(
            |(duration_ms, heavy, light, left_trigger, right_trigger)| Frame {
                duration_ms,
                heavy,
                light,
                left_trigger,
                right_trigger,
            },
        )
}

fn args(frames: Vec<Frame>, scale: Option<f64>) -> PlayFramesArgs {
    PlayFramesArgs {
        pad_id: "gamepad:0".into(),
        frames,
        scale,
    }
}

proptest! {
    #![proptest_config(config())]

    // An invalid request is an error for every scale and every pad, so no short-circuit can hide it.
    #[test]
    fn invalid_requests_are_rejected_whatever_the_scale_and_pad(
        frames in proptest::collection::vec(frame_with(level()), 1..12),
        scale in proptest::option::of(0u32..=100),
        tier in 0u8..=3,
    ) {
        let scale = scale.map(|n| f64::from(n) / 100.0);
        let valid = validate_frames(&frames, &limits()).is_ok();
        let planned = plan_play(&args(frames, scale), &test_pad(tier), &limits(), 1.0);
        prop_assert_eq!(valid, planned.is_ok());
    }

    // What plays stays inside the limits and is not cut twice.
    #[test]
    fn planned_frames_stay_within_the_limits(
        frames in proptest::collection::vec(frame_with(valid_level()), 1..12),
        scale in 1u32..=100,
        tier in 1u8..=3,
    ) {
        prop_assume!(validate_frames(&frames, &limits()).is_ok());
        let planned = plan_play(
            &args(frames.clone(), Some(f64::from(scale) / 100.0)),
            &test_pad(tier),
            &limits(),
            1.0,
        ).unwrap();
        if let Plan::Play { play, .. } = planned {
            let play = play.into_inner();
            prop_assert!(play.tier >= 1 && play.tier <= tier);
            let original: u64 = frames.iter().map(|f| f.duration_ms).sum();
            let total: u64 = play.frames.iter().map(|f| f.duration_ms).sum();
            prop_assert_eq!(original, total);

            let mut run = 0;
            for f in &play.frames {
                let silent = f.heavy == 0.0 && f.light == 0.0
                    && f.left_trigger.unwrap_or(0.0) == 0.0
                    && f.right_trigger.unwrap_or(0.0) == 0.0;
                run = if silent { 0 } else { run + f.duration_ms };
                prop_assert!(run <= limits().max_continuous_ms);
                prop_assert!(f.heavy <= 1.0 && f.light <= 1.0);
            }
            let (again, cut) = cap_continuous(play.frames.clone(), limits().max_continuous_ms);
            prop_assert!(!cut);
            prop_assert_eq!(again, play.frames);
        }
    }

    // The bridge shape survives a serde round trip.
    #[test]
    fn bridge_args_survive_a_round_trip(
        frames in proptest::collection::vec(frame_with(valid_level()), 1..6),
        scale in proptest::option::of(valid_level()),
    ) {
        let original = args(frames, scale);
        let json = serde_json::to_string(&original).unwrap();
        let back: PlayFramesArgs = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(original, back);
    }
}
