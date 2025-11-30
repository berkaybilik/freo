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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::path::PathBuf;

    #[test]
    fn comment_token_resolver_uses_defaults_when_no_custom_tokens_are_provided() {
        let resolver = CommentTokenResolver::new(None);

        for (ext, token) in DEFAULT_COMMENT_TOKENS {
            let file = PathBuf::from(format!("file.{}", ext));
            assert_eq!(resolver.token_for(file.as_path()), Some(*token));
        }
    }

    #[test]
    fn comment_token_resolver_custom_tokens_are_used_when_provided_and_no_overlap_with_defaults() {
        assert!(
            CommentTokenResolver::new(None).token_for(PathBuf::from("file.txt").as_path()).is_none(),
            "`txt` already exists in the defaults so this test is meaningless"
        );

        let resolver = CommentTokenResolver::new(
            Some(
                HashMap::from([("txt".to_string(), "X".to_string())])
            )
        );

        assert_eq!(resolver.token_for(PathBuf::from("file.txt").as_path()), Some("X"));
    }

    #[test]
    fn comment_token_resolver_custom_tokens_override_defaults() {
        assert!(
            !CommentTokenResolver::new(None).token_for(PathBuf::from("file.rs").as_path()).is_none(),
            "`rs` does not exist in defaults so this test is meaningless"
        );

        let resolver = CommentTokenResolver::new(
            Some(
                HashMap::from([("rs".to_string(), "X".to_string())])
            )
        );

        assert_eq!(resolver.token_for(PathBuf::from("file.rs").as_path()), Some("X"));
    }

    #[test]
    fn comment_token_resolver_custom_tokens_are_normalized() {
        let resolver = CommentTokenResolver::new(
            Some(
                HashMap::from(
                    [
                        (".rs".to_string(), "A".to_string()),
                        ("PY".to_string(), "B".to_string()),
                        ("   txt   ".to_string(), "C".to_string()),
                        ("   YaMl   ".to_string(), "D".to_string()),
                        ("   .Yml   ".to_string(), "E".to_string()),
                        ("sql.".to_string(), "F".to_string()),
                    ]
            )
            )
        );

        assert_eq!(resolver.token_for(PathBuf::from("file.rs").as_path()), Some("A"));
        assert_eq!(resolver.token_for(PathBuf::from("file.py").as_path()), Some("B"));
        assert_eq!(resolver.token_for(PathBuf::from("file.txt").as_path()), Some("C"));
        assert_eq!(resolver.token_for(PathBuf::from("file.yaml").as_path()), Some("D"));
        assert_eq!(resolver.token_for(PathBuf::from("file.yml").as_path()), Some("E"));
        assert_ne!(resolver.token_for(PathBuf::from("file.sql").as_path()), Some("F"));
        assert_ne!(resolver.token_for(PathBuf::from("file.sql.").as_path()), Some("F"));
    }
}
