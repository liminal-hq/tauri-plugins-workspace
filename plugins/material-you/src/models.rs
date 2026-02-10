use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialYouResponse {
    pub supported: bool,
    pub api_level: i32,
    pub palettes: Palettes,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Palettes {
    pub system_accent1: Option<HashMap<String, String>>,
    pub system_accent2: Option<HashMap<String, String>>,
    pub system_accent3: Option<HashMap<String, String>>,
    pub system_neutral1: Option<HashMap<String, String>>,
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
