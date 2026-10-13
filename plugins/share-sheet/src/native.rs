// Payloads sent to the Kotlin and Swift plugins, which share one shape
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct FileArgs<'a> {
    pub path: &'a str,
    pub mime: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<&'a str>,
}

#[derive(Debug, Serialize)]
pub struct TextArgs<'a> {
    pub text: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<&'a str>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_missing_title_is_left_out() {
        let args = FileArgs {
            path: "/data/user/0/app/cache/save.zip",
            mime: "application/zip",
            title: None,
        };
        assert_eq!(
            serde_json::to_value(args).unwrap(),
            json!({ "path": "/data/user/0/app/cache/save.zip", "mime": "application/zip" })
        );
        let text = TextArgs {
            text: "hi",
            title: Some("Fizz"),
        };
        assert_eq!(
            serde_json::to_value(text).unwrap(),
            json!({ "text": "hi", "title": "Fizz" })
        );
    }
}
