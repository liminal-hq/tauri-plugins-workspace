// Runs the shared request corpus, which the guest tests read too, through the Rust rules
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use serde::Deserialize;
use serde_json::{json, Value};

use crate::{
    models::*,
    normalise::{plan_play, plan_steps, Plan, RawControls, TierInfo},
    validate::MAX_STEPS,
    Result,
};

const CORPUS: &str = include_str!("../tests/conformance/requests.json");

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Corpus {
    constants: Constants,
    defaults: Defaults,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Constants {
    max_steps: usize,
    effect_ids: Vec<String>,
    primitive_ids: Vec<String>,
}

#[derive(Deserialize)]
struct Defaults {
    limits: Value,
    device: Device,
}

#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "camelCase")]
struct Device {
    top_tier: u8,
    has_amplitude_control: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Case {
    name: String,
    command: String,
    request: Option<EffectRequest>,
    steps: Option<Vec<CompiledStep>>,
    controls: Option<Controls>,
    limits: Option<Value>,
    expect: Expect,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Controls {
    scale: Option<f64>,
    max_tier: Option<u8>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
enum Expect {
    Ok(Outcome),
    Error(String),
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct Outcome {
    normalised: Option<Value>,
    reasons: Option<Vec<String>>,
    silent: Option<String>,
}

fn limits_for(defaults: &Value, over: &Option<Value>) -> Limits {
    let mut merged = defaults.clone();
    if let (Some(base), Some(Value::Object(extra))) = (merged.as_object_mut(), over) {
        for (key, value) in extra {
            base.insert(key.clone(), value.clone());
        }
    }
    serde_json::from_value(merged).expect("limits in the corpus")
}

fn steps_json(args: &PlayStepsArgs) -> Value {
    Value::Array(
        args.steps
            .iter()
            .map(|s| {
                json!({
                    "atMs": s.at_ms,
                    "budgetMs": s.budget_ms,
                    "effect": serde_json::to_value(&s.request.effect).expect("serialise effect"),
                })
            })
            .collect(),
    )
}

#[test]
fn the_corpus_constants_match_the_rules() {
    let corpus: Corpus = serde_json::from_str(CORPUS).expect("corpus parses");
    assert_eq!(corpus.constants.max_steps, MAX_STEPS);
    assert_eq!(corpus.constants.effect_ids, EFFECT_IDS);
    assert_eq!(corpus.constants.primitive_ids, PRIMITIVE_IDS);
}

#[test]
fn every_corpus_case_gets_the_expected_outcome() {
    let corpus: Corpus = serde_json::from_str(CORPUS).expect("corpus parses");
    assert!(!corpus.cases.is_empty());

    for case in &corpus.cases {
        let limits = limits_for(&corpus.defaults.limits, &case.limits);
        let device = corpus.defaults.device;
        let tier_info = move || -> Result<TierInfo> {
            Ok(TierInfo {
                top_tier: device.top_tier,
                has_amplitude_control: device.has_amplitude_control,
            })
        };
        let controls = RawControls {
            scale: case.controls.as_ref().and_then(|c| c.scale),
            max_tier: case.controls.as_ref().and_then(|c| c.max_tier),
        };

        // What came out: a message for a rejection, or the silent reason, or what is forwarded.
        let (silent, normalised, reasons, error) = match case.command.as_str() {
            "play" => {
                let req = case.request.clone().expect("play case has a request");
                match plan_play(req, &controls, &limits, tier_info) {
                    Err(e) => (None, None, Vec::new(), Some(e.to_string())),
                    Ok(Plan::Silent(r)) => (r.reason, None, Vec::new(), None),
                    Ok(Plan::Forward(n)) => (
                        None,
                        Some(serde_json::to_value(&n.value().req.effect).expect("serialise")),
                        n.reasons().to_vec(),
                        None,
                    ),
                }
            }
            "play_steps" => {
                let steps = case.steps.clone().expect("play_steps case has steps");
                match plan_steps(steps, &controls, &limits, tier_info) {
                    Err(e) => (None, None, Vec::new(), Some(e.to_string())),
                    Ok(Plan::Silent(r)) => (r.reason, None, Vec::new(), None),
                    Ok(Plan::Forward(n)) => (
                        None,
                        Some(steps_json(n.value())),
                        n.reasons().to_vec(),
                        None,
                    ),
                }
            }
            other => panic!("{}: unknown command {other}", case.name),
        };

        match &case.expect {
            Expect::Error(fragment) => {
                let message = error.unwrap_or_else(|| panic!("{}: expected an error", case.name));
                assert!(
                    message.contains(fragment.as_str()),
                    "{}: `{message}` does not contain `{fragment}`",
                    case.name
                );
            }
            Expect::Ok(outcome) => {
                assert!(error.is_none(), "{}: unexpected error {error:?}", case.name);
                assert_eq!(silent, outcome.silent, "{}: silent reason", case.name);
                if let Some(expected) = &outcome.normalised {
                    assert_eq!(
                        normalised.as_ref(),
                        Some(expected),
                        "{}: normalised",
                        case.name
                    );
                }
                if let Some(expected) = &outcome.reasons {
                    assert_eq!(&reasons, expected, "{}: reasons", case.name);
                }
            }
        }
    }
}
