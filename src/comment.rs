use std::collections::HashMap;
use std::path::Path;

const DEFAULT_COMMENT_TOKENS: &[(&str, &str)] = &[
    ("c", "//"),
    ("cc", "//"),
    ("cpp", "//"),
    ("cs", "//"),
    ("go", "//"),
    ("h", "//"),
    ("hpp", "//"),
    ("java", "//"),
    ("js", "//"),
    ("jsx", "//"),
    ("kt", "//"),
    ("rs", "//"),
    ("swift", "//"),
    ("ts", "//"),
    ("tsx", "//"),
    ("py", "#"),
    ("rb", "#"),
    ("sh", "#"),
    ("bash", "#"),
    ("toml", "#"),
    ("yaml", "#"),
    ("yml", "#"),
    ("ini", "#"),
    ("sql", "--"),
];

#[derive(Debug)]
pub struct CommentTokenResolver {
    lookup: HashMap<String, String>,
}

impl Default for CommentTokenResolver {
    fn default() -> Self {
        Self::new(None)
    }
}

impl CommentTokenResolver {
    pub fn new(custom_mappings: Option<HashMap<String, String>>) -> Self {
        let mut lookup = HashMap::new();
        for (ext, token) in DEFAULT_COMMENT_TOKENS {
            lookup.insert((*ext).to_ascii_lowercase(), (*token).to_string());
        }

        if let Some(custom) = custom_mappings {
            for (key, value) in custom {
                let normalized_token = value.trim().to_string();
                if normalized_token.is_empty() {
                    continue;
                }
                lookup.insert(normalize_extension_key(&key), normalized_token);
            }
        }

        Self { lookup }
    }

    pub fn token_for(&self, file_path: &Path) -> Option<&str> {
        if let Some(ext) = file_path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_ascii_lowercase())
        {
            return self.lookup.get(&ext).map(|token| token.as_str());
        }

        None
    }
}

fn normalize_extension_key(value: &str) -> String {
    value.trim().trim_start_matches('.').to_ascii_lowercase()
}
