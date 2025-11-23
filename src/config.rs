use std::collections::HashMap;

use serde::Deserialize;

const DEFAULT_KEYWORD: &str = "FREO";

#[derive(Debug, Deserialize)]
#[serde(from = "AppConfigRaw")]
pub struct AppConfig {
    keyword: String,
    comment_map: Option<HashMap<String, String>>,
}

#[derive(Deserialize)]
struct AppConfigRaw {
    keyword: Option<String>,
    #[serde(default)]
    comment_map: Option<HashMap<String, String>>,
}

impl From<AppConfigRaw> for AppConfig {
    fn from(raw: AppConfigRaw) -> Self {
        AppConfig::new(raw.keyword, raw.comment_map)
    }
}

impl AppConfig {
    pub fn new(keyword: Option<String>, comment_map: Option<HashMap<String, String>>) -> Self {
        let keyword = keyword.unwrap_or_else(|| DEFAULT_KEYWORD.to_string());
        Self {
            keyword,
            comment_map,
        }
    }

    pub fn keyword(&self) -> &str {
        &self.keyword
    }

    pub fn comment_map(&self) -> Option<&HashMap<String, String>> {
        self.comment_map.as_ref()
    }
}
