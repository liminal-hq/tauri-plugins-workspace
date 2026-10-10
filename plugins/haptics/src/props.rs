// Property tests for validation and capping: whatever the input, the rules hold
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use proptest::{prelude::*, test_runner::RngAlgorithm};

use crate::{
    models::*,
    normalise::{apply_scale, cap_request, plan_play, plan_steps, Plan, RawControls, TierInfo},
    validate::{validate_request, validate_steps, MAX_STEPS},
    Result,
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
        max_amplitude: 200,
        allow_repeating_waveforms: false,
    }
}

fn device() -> Result<TierInfo> {
    Ok(TierInfo {
        top_tier: 4,
        has_amplitude_control: true,
    })
}

fn wrap(effect: Effect) -> EffectRequest {
    EffectRequest {
        id: None,
        usage: None,
        respect_system_settings: None,
        stop_before_play: None,
        effect,
    }
}

/// Requests of every shape, including ones that are invalid in each way the rules know about.
fn effect() -> impl Strategy<Value = Effect> {
    let ids = prop_oneof![
        Just("click".to_string()),
        Just("double_click".to_string()),
        Just("pop".to_string()),
    ];
    let primitive = prop_oneof![
        Just("tick".to_string()),
        Just("thud".to_string()),
        Just("pop".to_string())
    ];
    prop_oneof![
        (0u64..3_000, proptest::option::of(0u16..400)).prop_map(|(duration_ms, amplitude)| {
            Effect::Oneshot {
                duration_ms,
                amplitude,
            }
        }),
        (
            proptest::collection::vec(0u64..800, 0..8),
            any::<bool>(),
            proptest::option::of(-3i32..10),
        )
            .prop_flat_map(|(timings_ms, with_amplitudes, repeat)| {
                let len = timings_ms.len();
                proptest::collection::vec(0u16..320, len..=len).prop_map(move |amps| {
                    Effect::Waveform {
                        timings_ms: timings_ms.clone(),
                        amplitudes: with_amplitudes.then_some(amps),
                        repeat,
                    }
                })
            }),
        ids.prop_map(|effect_id| Effect::Predefined { effect_id }),
        proptest::collection::vec(
            (
                primitive,
                proptest::option::of(-0.5f32..1.5),
                proptest::option::of(0u64..2_000)
            ),
            0..6,
        )
        .prop_map(|steps| Effect::Composition {
            steps: steps
                .into_iter()
                .map(|(primitive, scale, delay_ms)| CompositionStep::Primitive {
                    primitive,
                    scale,
                    delay_ms
                })
                .collect(),
        }),
        (
            proptest::option::of(-10.0f32..300.0),
            proptest::collection::vec((-0.5f32..1.5, -10.0f32..300.0, 0u64..700), 0..6),
        )
            .prop_map(|(initial_frequency_hz, points)| Effect::EnvelopeWaveform {
                initial_frequency_hz,
                control_points: points
                    .into_iter()
                    .map(|(amplitude, frequency_hz, duration_ms)| EnvelopePoint {
                        amplitude,
                        frequency_hz,
                        duration_ms
                    })
                    .collect(),
            }),
    ]
}

fn valid_request() -> impl Strategy<Value = EffectRequest> {
    effect().prop_map(wrap).prop_filter("valid", |r| {
        validate_request(r, limits().max_duration_ms).is_ok()
    })
}

fn controls() -> impl Strategy<Value = RawControls> {
    (
        proptest::option::of(0.0f64..=1.0),
        proptest::option::of(0u8..=4),
    )
        .prop_map(|(scale, max_tier)| RawControls { scale, max_tier })
}

/// The time a capped request keeps playing for, where it is bounded by the cap.
fn capped_length(effect: &Effect) -> Option<u64> {
    match effect {
        Effect::Oneshot { duration_ms, .. } => Some(*duration_ms),
        Effect::Waveform { timings_ms, .. } => Some(timings_ms.iter().sum()),
        _ => None,
    }
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn a_capped_request_fits_the_budget(req in valid_request(), budget in 1u64..1_500) {
        let (capped, _) = cap_request(req, &limits(), budget);
        if let Some(length) = capped_length(&capped.effect) {
            prop_assert!(length <= budget);
        }
    }

    #[test]
    fn capping_twice_changes_nothing_more(req in valid_request(), budget in 1u64..1_500) {
        let (once, _) = cap_request(req, &limits(), budget);
        let (twice, reasons) = cap_request(once.clone(), &limits(), budget);
        prop_assert_eq!(once, twice);
        prop_assert!(reasons.is_empty());
    }

    #[test]
    fn capping_keeps_a_valid_request_valid(req in valid_request(), budget in 1u64..1_500) {
        let (capped, _) = cap_request(req, &limits(), budget);
        prop_assert!(validate_request(&capped, budget.max(limits().max_duration_ms)).is_ok());
    }

    #[test]
    fn an_invalid_request_rejects_whatever_the_controls(req in effect().prop_map(wrap), c in controls()) {
        let valid = validate_request(&req, limits().max_duration_ms).is_ok();
        let planned = plan_play(req, &c, &limits(), device);
        prop_assert_eq!(planned.is_ok(), valid);
    }

    #[test]
    fn scaling_keeps_a_request_valid_and_never_raises_an_amplitude(req in valid_request(), scale in 0.0f64..=1.0) {
        let scaled = apply_scale(req.clone(), scale);
        prop_assert!(validate_request(&scaled, limits().max_duration_ms).is_ok() || scale == 0.0);
        if let (Effect::Oneshot { amplitude: Some(before), .. }, Effect::Oneshot { amplitude: Some(after), .. }) =
            (&req.effect, &scaled.effect)
        {
            prop_assert!(after <= before || *after == 1);
        }
    }

    #[test]
    fn a_step_list_is_planned_exactly_when_every_rule_holds(
        steps in proptest::collection::vec((0u64..1_200, effect()), 0..6)
    ) {
        let steps: Vec<CompiledStep> = steps
            .into_iter()
            .map(|(at_ms, effect)| CompiledStep { at_ms, request: wrap(effect) })
            .collect();
        let accepted = validate_steps(&steps, &limits()).is_ok();
        let expected = !steps.is_empty()
            && steps.len() <= MAX_STEPS
            && steps.iter().all(|s| {
                s.at_ms < limits().max_duration_ms
                    && validate_request(&s.request, limits().max_duration_ms - s.at_ms).is_ok()
            });
        prop_assert_eq!(accepted, expected);

        if let Ok(Plan::Forward(n)) = plan_steps(steps, &RawControls::default(), &limits(), device) {
            for step in &n.value().steps {
                prop_assert_eq!(step.budget_ms, limits().max_duration_ms - step.at_ms);
                if let Some(length) = capped_length(&step.request.effect) {
                    prop_assert!(length <= step.budget_ms);
                }
            }
        }
    }

    #[test]
    fn bridge_arguments_survive_a_round_trip(req in valid_request(), budget in 1u64..1_500) {
        let args = PlayArgs { req, budget_ms: budget };
        let json = serde_json::to_value(&args).expect("serialise");
        let back: PlayArgs = serde_json::from_value(json).expect("deserialise");
        prop_assert_eq!(args, back);
    }
}
