use serde::Deserialize;

const DEFAULT_KEYWORD: &str = "FREO";

#[derive(Debug, Deserialize)]
#[serde(from = "AppConfigRaw")]
pub struct AppConfig {
    keyword: String,
}

#[derive(Deserialize)]
struct AppConfigRaw {
    keyword: Option<String>,
}

impl From<AppConfigRaw> for AppConfig {
    fn from(raw: AppConfigRaw) -> Self { AppConfig::new(raw.keyword) }
}

impl AppConfig {
    pub fn new(keyword: Option<String>) -> Self {
        let keyword = keyword.unwrap_or_else(|| DEFAULT_KEYWORD.to_string());
        Self { keyword }
    }

    pub fn keyword(&self) -> &str { &self.keyword }
}

pub fn run(config: &AppConfig, base_branch: &str) {
    println!(
        "Running for the reviewers eyes only with keyword: {} and base branch: {}", config.keyword(), base_branch
    );
}
