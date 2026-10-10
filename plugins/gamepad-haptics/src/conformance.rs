// Runs the shared frames corpus, which the guest tests read too, through the Rust rules
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use serde::Deserialize;
use serde_json::Value;

use crate::{
    models::*,
    normalise::{plan_play, test_pad, Plan},
    validate::MAX_FRAMES,
    Error,
};

const CORPUS: &str = include_str!("../tests/conformance/frames.json");

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
    max_frames: usize,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Defaults {
    limits: Limits,
    pad: PadSpec,
    master_scale: f64,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct PadSpec {
    top_tier: u8,
    reason: Option<String>,
    #[serde(default)]
    light_binary: bool,
}

#[derive(Deserialize)]
struct Case {
    name: String,
    pad: Option<PadSpec>,
    args: PlayFramesArgs,
    expect: Expect,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
enum Expect {
    Error(String),
    Silent(String),
    Ok(Outcome),
}

#[derive(Deserialize)]
struct Outcome {
    tier: u8,
    frames: Option<Vec<Frame>>,
    reasons: Option<Vec<String>>,
}

#[test]
fn the_corpus_agrees_with_the_rust_rules() {
    let corpus: Corpus = serde_json::from_str(CORPUS).expect("corpus parses");
    assert_eq!(corpus.constants.max_frames, MAX_FRAMES);

    for case in &corpus.cases {
        let spec = case
            .pad
            .clone()
            .unwrap_or_else(|| corpus.defaults.pad.clone());
        let mut pad = test_pad(spec.top_tier);
        pad.reason = spec.reason;
        pad.light_binary = spec.light_binary;
        let result = plan_play(
            &case.args,
            &pad,
            &corpus.defaults.limits,
            corpus.defaults.master_scale,
        );
        match (&case.expect, result) {
            (Expect::Error(fragment), Err(Error::InvalidRequest(m))) => {
                assert!(m.contains(fragment), "{}: {m}", case.name)
            }
            (Expect::Silent(reason), Ok(Plan::Silent(r))) => {
                assert_eq!(&r, reason, "{}", case.name)
            }
            (Expect::Ok(want), Ok(Plan::Play { play, reasons })) => {
                let play = play.into_inner();
                assert_eq!(play.tier, want.tier, "{}", case.name);
                if let Some(frames) = &want.frames {
                    assert_eq!(&play.frames, frames, "{}", case.name);
                }
                if let Some(want_reasons) = &want.reasons {
                    assert_eq!(&reasons, want_reasons, "{}", case.name);
                }
            }
            (_, other) => panic!(
                "{}: unexpected outcome {:?}",
                case.name,
                other.map(|p| match p {
                    Plan::Silent(r) => Value::String(r),
                    Plan::Play { .. } => Value::String("play".into()),
                })
            ),
        }
    }
}
