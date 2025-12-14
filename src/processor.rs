use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;

use regex::{Regex, RegexBuilder};
use tempfile::NamedTempFile;

pub fn remove_matching_comments(
    comment_token: &str,
    keyword: &str,
    file_path: &Path,
) -> io::Result<()> {
    let parent = file_path.parent().unwrap_or(std::path::Path::new("."));
    let temp_file = NamedTempFile::new_in(parent)?;
    
    {
        let reader = BufReader::new(File::open(file_path)?);
        let mut writer = BufWriter::new(&temp_file);

        remove_matching_comments_from_stream(comment_token, keyword, reader, &mut writer)?;
    
        writer.flush()?;
    }

    persist_temp_file(temp_file, file_path, parent)?;

    Ok(())
}

fn persist_temp_file(
    temp_file: NamedTempFile,
    original_path: &Path,
    parent_dir: &Path,
) -> io::Result<()> {
    if let Ok(orig_meta) = fs::metadata(original_path) {
        let mut perms = orig_meta.permissions();
        perms.set_mode(orig_meta.mode());
        let _ = fs::set_permissions(temp_file.path(), perms);
    }

    temp_file.as_file().sync_all()?;

    temp_file
        .persist(original_path)
        .map_err(|err| io::Error::new(io::ErrorKind::Other, err))?;

    if let Ok(dir) = File::open(parent_dir) {
        let _ = dir.sync_all();
    }

    Ok(())
}

fn remove_matching_comments_from_stream<R, W>(
    comment_token: &str,
    keyword: &str,
    mut reader: R,
    writer: &mut W,
) -> io::Result<()>
where
    R: BufRead,
    W: Write,
{
    let pattern: Regex = build_keyword_comment_pattern(comment_token, keyword);

    let mut current_line = String::new();
    while reader.read_line(&mut current_line)? > 0 {
        let processed_line = strip_keyword_comment(&current_line, &pattern);
        if !processed_line.is_empty() {
            write!(writer, "{}", processed_line)?;
        }        
        current_line.clear();
    }

    Ok(())
}

fn build_keyword_comment_pattern(comment_token: &str, keyword: &str) -> Regex {
    let comment_token_literal = regex::escape(comment_token);
    let keyword_literal = regex::escape(keyword);

    let pattern_format = format!(
        r"\s*{}\s*{}\b:?[^\r\n]*",
        comment_token_literal, keyword_literal
    );
    RegexBuilder::new(&pattern_format)
        .case_insensitive(true)
        .build()
        .unwrap_or_else(|e| panic!("Failed to build regex: {}", e))
}

fn strip_keyword_comment(text: &str, pattern: &Regex) -> String {
    let Some(match_) = pattern.find(text) else {
        return text.to_string();
    };

    let mut stripped_text = text.to_string();
    stripped_text.replace_range(match_.start()..match_.end(), "");

    if stripped_text.trim().is_empty() {
        return String::new();
    }

    return stripped_text;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn build_keyword_comment_pattern_matches_common_variants() {
        let pattern = build_keyword_comment_pattern("//", "FREO");

        assert!(pattern.is_match("//FREO"));
        assert!(pattern.is_match("//   FREO fix later"));
        assert!(pattern.is_match("//FREO: something"));
        assert!(!pattern.is_match("//FREOL"));
    }

    #[test]
    fn build_keyword_comment_pattern_escapes_special_characters() {
        let pattern = build_keyword_comment_pattern("/*", "FREO+");

        assert!(pattern.is_match("/* FREO+fix */"));
        assert!(pattern.is_match("/*FREO+fix*/"));
        assert!(!pattern.is_match("// FREO+fix"));
    }

    #[test]
    fn strip_keyword_comment_truncates_trailing_matching_comment() {
        let pattern = build_keyword_comment_pattern("//", "FREO");
        let result = strip_keyword_comment("let x = 5; // FREO: remove debug", &pattern);

        assert_eq!(result, "let x = 5;");
    }

    #[test]
    fn strip_keyword_comment_preserves_newline_when_matching_comment_is_trailing() {
        let pattern = build_keyword_comment_pattern("//", "FREO");
        let result = strip_keyword_comment("let x = 5; // FREO: remove debug\n", &pattern);

        assert_eq!(result, "let x = 5;\n");
    }

    #[test]
    fn strip_keyword_comment_returns_empty_when_only_matching_comment_remains() {
        let pattern = build_keyword_comment_pattern("//", "FREO");

        let result = strip_keyword_comment("// FREO clean up\n", &pattern);
        assert_eq!(result, "");

        let result = strip_keyword_comment("// FREO clean up", &pattern);
        assert_eq!(result, "");
    }

    #[test]
    fn strip_keyword_comment_returns_original_line_if_no_match() {
        let pattern = build_keyword_comment_pattern("//", "FREO");
        let original = "let x = 5; // NOTE keep";
        let result = strip_keyword_comment(original, &pattern);

        assert_eq!(result, original);
    }

    #[test]
    fn strip_keyword_comment_ignores_keyword_inside_string_literal() {
        let pattern = build_keyword_comment_pattern("//", "FREO");
        let original = r#"println!("FREO: keep this string");"#;

        let result = strip_keyword_comment(original, &pattern);

        assert_eq!(result, original);

        let original = r#"//println!("FREO: keep this string");"#;

        let result = strip_keyword_comment(original, &pattern);

        assert_eq!(result, original);
    }

    #[test]
    fn remove_matching_comments_from_stream_filters_matching_lines() {
        let input = Cursor::new(
            b"let x = 5; // FREO remove\nlet y = 6; // keep\n// FREO delete me\n".to_vec(),
        );
        let mut output = Vec::new();

        remove_matching_comments_from_stream("//", "FREO", input, &mut output).unwrap();

        assert_eq!(
            String::from_utf8(output).unwrap(),
            "let x = 5;\nlet y = 6; // keep\n"
        );
    }

    #[test]
    fn remove_matching_comments_from_stream_leaves_input_unchanged_when_there_are_no_matches() {
        let input_text = "let x = 5;\nlet y = 6; // keep";

        let input = Cursor::new(
            input_text.as_bytes().to_vec(),
        );
        let mut output = Vec::new();

        remove_matching_comments_from_stream("//", "FREO", input, &mut output).unwrap();

        assert_eq!(
            String::from_utf8(output).unwrap(),
            input_text
        );
    }

    #[test]
    fn remove_matching_comments_from_stream_does_not_add_trailing_newline_when_input_has_none() {
        let input = Cursor::new(b"let x = 5; // FREO remove".to_vec());
        let mut output = Vec::new();

        remove_matching_comments_from_stream("//", "FREO", input, &mut output).unwrap();

        assert_eq!(String::from_utf8(output).unwrap(), "let x = 5;");
    }
}
