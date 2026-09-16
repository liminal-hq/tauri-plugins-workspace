use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialYouResponse {
    pub supported: bool,
    pub api_level: i32,
    pub palettes: Palettes,
}

impl MaterialYouResponse {
    /// The response for any platform with no Material You equivalent (desktop, iOS).
    pub fn unsupported() -> Self {
        Self {
            supported: false,
            api_level: 0,
            palettes: Palettes {
                system_accent1: None,
                system_accent2: None,
                system_accent3: None,
                system_neutral1: None,
                system_neutral2: None,
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Palettes {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_accent1: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_accent2: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_accent3: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_neutral1: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_neutral2: Option<HashMap<String, String>>,
}

#[cfg(test)]
mod tests {
    use super::{MaterialYouResponse, Palettes};
    use std::collections::HashMap;

    #[test]
    fn serialises_api_level_as_camel_case() {
        let mut accent = HashMap::new();
        accent.insert("500".to_string(), "#FF123456".to_string());

        let response = MaterialYouResponse {
            supported: true,
            api_level: 34,
            palettes: Palettes {
                system_accent1: Some(accent),
                system_accent2: None,
                system_accent3: None,
                system_neutral1: None,
                system_neutral2: None,
            },
        };

        let json = serde_json::to_value(response).expect("serialisation should succeed");
        assert_eq!(json["supported"], true);
        assert_eq!(json["apiLevel"], 34);
        assert!(json.get("api_level").is_none());
    }

    #[test]
    fn unsupported_reports_supported_false_with_no_palettes() {
        let response = MaterialYouResponse::unsupported();
        assert!(!response.supported);
        assert_eq!(response.api_level, 0);
        assert!(response.palettes.system_accent1.is_none());
        assert!(response.palettes.system_accent2.is_none());
        assert!(response.palettes.system_accent3.is_none());
        assert!(response.palettes.system_neutral1.is_none());
        assert!(response.palettes.system_neutral2.is_none());
    }

    #[test]
    fn unsupported_serialises_palettes_as_an_empty_object_not_null_fields() {
        // The Android implementation's own pre-API-31 fallback serialises `palettes` as `{}` — a caller that checks `Object.keys(palettes).length === 0` for "no data" must see the same shape from every unsupported code path, not `{ system_accent1: null, ... }`.
        let json = serde_json::to_value(MaterialYouResponse::unsupported())
            .expect("serialisation should succeed");
        assert_eq!(json["palettes"], serde_json::json!({}));
    }

    #[test]
    fn deserialises_api_level_from_camel_case() {
        let json = serde_json::json!({
            "supported": false,
            "apiLevel": 0,
            "palettes": {
                "system_accent1": null,
                "system_accent2": null,
                "system_accent3": null,
                "system_neutral1": null,
                "system_neutral2": null
            }
        });

        let response: MaterialYouResponse =
            serde_json::from_value(json).expect("deserialisation should succeed");
        assert!(!response.supported);
        assert_eq!(response.api_level, 0);
        assert!(response.palettes.system_accent1.is_none());
    }
}
