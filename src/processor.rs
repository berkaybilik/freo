use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};

use regex::{Regex, RegexBuilder};
use tempfile::NamedTempFile;

#[must_use = "a prepared file is only written back by `commit`"]
pub struct PreparedFile {
    temp_file: NamedTempFile,
    original_path: PathBuf,
}

impl PreparedFile {
    pub fn path(&self) -> &Path {
        &self.original_path
    }
}

pub fn prepare(comment_token: &str, keyword: &str, file_path: &Path) -> io::Result<PreparedFile> {
    let parent = file_path.parent().unwrap_or(Path::new("."));
    let temp_file = NamedTempFile::new_in(parent)?;

    {
        let reader = BufReader::new(File::open(file_path)?);
        let mut writer = BufWriter::new(&temp_file);

        remove_matching_comments_from_stream(comment_token, keyword, reader, &mut writer)?;

        writer.flush()?;
    }

    Ok(PreparedFile {
        temp_file,
        original_path: file_path.to_path_buf(),
    })
}

pub fn commit(prepared: PreparedFile) -> io::Result<()> {
    let PreparedFile {
        temp_file,
        original_path,
    } = prepared;
    let parent = original_path.parent().unwrap_or(Path::new(".")).to_owned();

    persist_temp_file(temp_file, &original_path, &parent)
}

pub fn remove_matching_comments(
    comment_token: &str,
    keyword: &str,
    file_path: &Path,
) -> io::Result<()> {
    commit(prepare(comment_token, keyword, file_path)?)
}

fn persist_temp_file(
    temp_file: NamedTempFile,
    original_path: &Path,
    parent_dir: &Path,
) -> io::Result<()> {
    if let Ok(orig_meta) = fs::metadata(original_path) {
        let _ = fs::set_permissions(temp_file.path(), orig_meta.permissions());
    }

    temp_file.as_file().sync_all()?;

    temp_file.persist(original_path).map_err(io::Error::other)?;

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
    let keyword_pattern = build_keyword_comment_pattern(comment_token, keyword);
    let begin_pattern = build_block_marker_pattern(comment_token, keyword, BLOCK_BEGIN_SUFFIX);
    let end_pattern = build_block_marker_pattern(comment_token, keyword, BLOCK_END_SUFFIX);
    let inline_end_pattern =
        build_inline_block_marker_pattern(comment_token, keyword, BLOCK_END_SUFFIX);

    let mut open_block_line: Option<usize> = None;
    let mut line_number = 0usize;

    let mut current_line = String::new();
    while reader.read_line(&mut current_line)? > 0 {
        line_number += 1;

        let processed_line = if open_block_line.is_some() {
            if end_pattern.is_match(&current_line) {
                open_block_line = None;
            }
            String::new()
        } else if let Some(marker) = begin_pattern.find(&current_line) {
            if inline_end_pattern
                .find_at(&current_line, marker.end())
                .is_none()
            {
                open_block_line = Some(line_number);
            }
            String::new()
        } else {
            strip_keyword_comment(&current_line, &keyword_pattern, comment_token)
        };

        if !processed_line.is_empty() {
            write!(writer, "{}", processed_line)?;
        }
        current_line.clear();
    }

    if let Some(opened_at) = open_block_line {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "unterminated {keyword}-{BLOCK_BEGIN_SUFFIX} block opened on line {opened_at} \
                 (expected a matching {keyword}-{BLOCK_END_SUFFIX} on a line of its own); \
                 no files were modified"
            ),
        ));
    }

    Ok(())
}

const BLOCK_BEGIN_SUFFIX: &str = "BEGIN";
const BLOCK_END_SUFFIX: &str = "END";

fn build_keyword_comment_pattern(comment_token: &str, keyword: &str) -> Regex {
    let comment_token_literal = regex::escape(comment_token);
    let keyword_literal = regex::escape(keyword);

    let pattern_format = format!(
        r"\s*{}\s*{}\b:?[^\r\n]*",
        comment_token_literal, keyword_literal
    );
    build_case_insensitive(&pattern_format)
}

fn build_block_marker_pattern(comment_token: &str, keyword: &str, suffix: &str) -> Regex {
    build_marker_pattern(comment_token, keyword, suffix, r"^[ \t]*")
}

fn build_inline_block_marker_pattern(comment_token: &str, keyword: &str, suffix: &str) -> Regex {
    build_marker_pattern(comment_token, keyword, suffix, "")
}

fn build_marker_pattern(comment_token: &str, keyword: &str, suffix: &str, prefix: &str) -> Regex {
    let comment_token_literal = regex::escape(comment_token);
    let keyword_literal = regex::escape(keyword);

    let pattern_format = format!(
        r"{}{}[ \t]*{}-{}\b",
        prefix, comment_token_literal, keyword_literal, suffix
    );
    build_case_insensitive(&pattern_format)
}

fn build_case_insensitive(pattern: &str) -> Regex {
    RegexBuilder::new(pattern)
        .case_insensitive(true)
        .build()
        .unwrap_or_else(|e| panic!("Failed to build regex: {}", e))
}

fn remove_span(text: &str, start: usize, end: usize) -> String {
    let mut stripped_text = text.to_string();
    stripped_text.replace_range(start..end, "");

    if stripped_text.trim().is_empty() {
        return String::new();
    }

    stripped_text
}

#[derive(Debug, PartialEq, Eq)]
enum CommentStart {
    At(usize),
    None,
    Unknown,
}

fn find_comment_start(line: &str, comment_token: &str) -> CommentStart {
    let bytes = line.as_bytes();
    let token = comment_token.as_bytes();

    let mut quote: Option<u8> = None;
    let mut index = 0;

    while index < bytes.len() {
        match quote {
            Some(open_quote) => {
                match bytes[index] {
                    b'\\' => index += 1,
                    byte if byte == open_quote => quote = None,
                    _ => {}
                }
                index += 1;
            }
            None => {
                if bytes[index..].starts_with(token) {
                    return CommentStart::At(index);
                }
                if matches!(bytes[index], b'"' | b'\'' | b'`') {
                    quote = Some(bytes[index]);
                }
                index += 1;
            }
        }
    }

    match quote {
        Some(_) => CommentStart::Unknown,
        None => CommentStart::None,
    }
}

/// Removes a comment from `text` when the comment *opens* with the keyword.
///
/// A comment that merely mentions the keyword further along — `// see the
/// FREO-BEGIN block above` — is prose about the tool, not a note addressed to a
/// reviewer, and is left alone. Erring this way is deliberate: a comment that
/// survives stays visible in the diff and can be deleted by hand, whereas
/// content deleted in error is committed unreviewed.
fn strip_keyword_comment(text: &str, pattern: &Regex, comment_token: &str) -> String {
    let comment_start = match find_comment_start(text, comment_token) {
        CommentStart::At(index) => index,
        CommentStart::None => return text.to_string(),
        // Quotes did not balance, so fall back to the first token on the line.
        // The keyword still has to follow it, which keeps the guess the
        // conservative one: at worst a comment survives that should have gone.
        CommentStart::Unknown => match text.find(comment_token) {
            Some(index) => index,
            None => return text.to_string(),
        },
    };

    // Start at the whitespace run in front of the comment so the pattern's
    // leading `\s*` can still absorb the gap after the code it trails.
    let search_from = text[..comment_start].trim_end().len();

    match pattern.find_at(text, search_from) {
        Some(match_) if match_.start() == search_from => {
            remove_span(text, match_.start(), match_.end())
        }
        _ => text.to_string(),
    }
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
        let result = strip_keyword_comment("let x = 5; // FREO: remove debug", &pattern, "//");

        assert_eq!(result, "let x = 5;");
    }

    #[test]
    fn strip_keyword_comment_preserves_newline_when_matching_comment_is_trailing() {
        let pattern = build_keyword_comment_pattern("//", "FREO");
        let result = strip_keyword_comment("let x = 5; // FREO: remove debug\n", &pattern, "//");

        assert_eq!(result, "let x = 5;\n");
    }

    #[test]
    fn strip_keyword_comment_returns_empty_when_only_matching_comment_remains() {
        let pattern = build_keyword_comment_pattern("//", "FREO");

        let result = strip_keyword_comment("// FREO clean up\n", &pattern, "//");
        assert_eq!(result, "");

        let result = strip_keyword_comment("// FREO clean up", &pattern, "//");
        assert_eq!(result, "");
    }

    #[test]
    fn strip_keyword_comment_returns_original_line_if_no_match() {
        let pattern = build_keyword_comment_pattern("//", "FREO");
        let original = "let x = 5; // NOTE keep";
        let result = strip_keyword_comment(original, &pattern, "//");

        assert_eq!(result, original);
    }

    #[test]
    fn strip_keyword_comment_ignores_keyword_inside_string_literal() {
        let pattern = build_keyword_comment_pattern("//", "FREO");
        let original = r#"println!("FREO: keep this string");"#;

        let result = strip_keyword_comment(original, &pattern, "//");

        assert_eq!(result, original);

        let original = r#"//println!("FREO: keep this string");"#;

        let result = strip_keyword_comment(original, &pattern, "//");

        assert_eq!(result, original);
    }

    #[test]
    fn strip_keyword_comment_ignores_a_comment_token_inside_a_string_literal() {
        let hash_pattern = build_keyword_comment_pattern("#", "FREO");
        let original = r##"x = "#FREO in a string""##;

        assert_eq!(
            strip_keyword_comment(original, &hash_pattern, "#"),
            original
        );

        let slash_pattern = build_keyword_comment_pattern("//", "FREO");
        let original = "let url = \"http://FREO.example/docs\";";

        assert_eq!(
            strip_keyword_comment(original, &slash_pattern, "//"),
            original
        );
    }

    #[test]
    fn strip_keyword_comment_still_strips_a_comment_that_follows_a_string_literal() {
        let pattern = build_keyword_comment_pattern("#", "FREO");
        let result = strip_keyword_comment("print('#FREO') # FREO: drop this\n", &pattern, "#");

        assert_eq!(result, "print('#FREO')\n");
    }

    #[test]
    fn strip_keyword_comment_falls_back_to_the_first_token_when_quotes_do_not_balance() {
        let pattern = build_keyword_comment_pattern("//", "FREO");
        let result =
            strip_keyword_comment("fn first<'a>(x: &str) {} // FREO: tidy", &pattern, "//");

        assert_eq!(result, "fn first<'a>(x: &str) {}");
    }

    #[test]
    fn strip_keyword_comment_keeps_a_comment_that_only_mentions_the_keyword() {
        let pattern = build_keyword_comment_pattern("//", "FREO");

        for original in [
            // The second token is what used to turn this into a match.
            "// see the // FREO-BEGIN block above",
            "// see the FREO-BEGIN block above",
            "// TODO FREO: this is prose about the tool",
            "let x = 5; // note: FREO strips these",
            "//! the FREO keyword is configurable",
        ] {
            assert_eq!(
                strip_keyword_comment(original, &pattern, "//"),
                original,
                "should have been left alone: {original}"
            );
        }
    }

    #[test]
    fn strip_keyword_comment_requires_the_keyword_to_open_the_comment() {
        let pattern = build_keyword_comment_pattern("//", "FREO");

        // Opening the comment: removed.
        assert_eq!(strip_keyword_comment("// FREO: go", &pattern, "//"), "");
        assert_eq!(strip_keyword_comment("//FREO go", &pattern, "//"), "");
        assert_eq!(
            strip_keyword_comment("let x = 5;   // FREO go", &pattern, "//"),
            "let x = 5;"
        );

        // One word into the comment: kept.
        let original = "// a FREO: go";
        assert_eq!(strip_keyword_comment(original, &pattern, "//"), original);
    }

    #[test]
    fn strip_keyword_comment_keeps_a_later_comment_that_opens_with_the_keyword() {
        // The first `//` opens the comment, so everything after it is one
        // comment body, and that body does not start with the keyword.
        let pattern = build_keyword_comment_pattern("//", "FREO");
        let original = "let x = 5; // note // FREO: go";

        assert_eq!(strip_keyword_comment(original, &pattern, "//"), original);
    }

    #[test]
    fn find_comment_start_classifies_lines() {
        assert_eq!(
            find_comment_start("let x = 5; // note", "//"),
            CommentStart::At(11)
        );
        assert_eq!(
            find_comment_start(r#"x = "// note""#, "//"),
            CommentStart::None
        );
        assert_eq!(find_comment_start("let y = 5;", "//"), CommentStart::None);
        assert_eq!(
            find_comment_start("it's fine // note", "//"),
            CommentStart::Unknown
        );
        assert_eq!(
            find_comment_start(r#"x = "a\"// b""#, "//"),
            CommentStart::None
        );
        assert_eq!(
            find_comment_start("let e = \"é\"; // t", "//"),
            CommentStart::At(14)
        );
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

        let input = Cursor::new(input_text.as_bytes().to_vec());
        let mut output = Vec::new();

        remove_matching_comments_from_stream("//", "FREO", input, &mut output).unwrap();

        assert_eq!(String::from_utf8(output).unwrap(), input_text);
    }

    #[test]
    fn remove_matching_comments_from_stream_does_not_add_trailing_newline_when_input_has_none() {
        let input = Cursor::new(b"let x = 5; // FREO remove".to_vec());
        let mut output = Vec::new();

        remove_matching_comments_from_stream("//", "FREO", input, &mut output).unwrap();

        assert_eq!(String::from_utf8(output).unwrap(), "let x = 5;");
    }

    fn run_stream(input: &str) -> io::Result<String> {
        let mut output = Vec::new();
        remove_matching_comments_from_stream(
            "//",
            "FREO",
            Cursor::new(input.as_bytes().to_vec()),
            &mut output,
        )?;
        Ok(String::from_utf8(output).unwrap())
    }

    #[test]
    fn build_block_marker_pattern_matches_only_the_full_marker() {
        let pattern = build_block_marker_pattern("//", "FREO", BLOCK_BEGIN_SUFFIX);

        assert!(pattern.is_match("// FREO-BEGIN"));
        assert!(pattern.is_match("//FREO-BEGIN"));
        assert!(pattern.is_match("  //  freo-begin  "));
        assert!(!pattern.is_match("// FREO-BEGINNING"));
        assert!(!pattern.is_match("// FREO-END"));
        assert!(!pattern.is_match("// FREO: ordinary"));
    }

    #[test]
    fn build_block_marker_pattern_requires_the_marker_to_own_the_line() {
        let pattern = build_block_marker_pattern("//", "FREO", BLOCK_BEGIN_SUFFIX);

        assert!(!pattern.is_match("    \"// FREO-BEGIN\\n\","));
        assert!(!pattern.is_match("let url = \"http://FREO-BEGIN\";"));
        assert!(!pattern.is_match("let a = 1; // FREO-BEGIN"));
    }

    #[test]
    fn a_quoted_marker_does_not_open_a_block() {
        let output = run_stream(concat!(
            "let fixture = [\n",
            "    \"// FREO-BEGIN\\n\",\n",
            "    \"// FREO-END\\n\",\n",
            "];\n",
        ))
        .unwrap();

        assert_eq!(
            output,
            "let fixture = [\n    \"// FREO-BEGIN\\n\",\n    \"// FREO-END\\n\",\n];\n"
        );
    }

    #[test]
    fn a_block_opened_and_closed_on_one_line_does_not_stay_open() {
        let output = run_stream("// FREO-BEGIN short note // FREO-END\nkeep me\n").unwrap();

        assert_eq!(output, "keep me\n");
    }

    #[test]
    fn a_marker_sharing_a_line_with_code_is_not_a_marker() {
        let output = run_stream("let a = 1; // FREO-BEGIN\nlet b = 2;\n").unwrap();

        assert_eq!(output, "let a = 1;\nlet b = 2;\n");
    }

    #[test]
    fn block_markers_remove_every_line_between_them() {
        let output = run_stream(concat!(
            "fn main() {\n",
            "// FREO-BEGIN\n",
            "// the vendor API has no webhook yet, so we poll;\n",
            "// revisit once they ship one\n",
            "// FREO-END\n",
            "let x = 5;\n",
            "}\n",
        ))
        .unwrap();

        assert_eq!(output, "fn main() {\nlet x = 5;\n}\n");
    }

    #[test]
    fn block_markers_remove_non_comment_lines_between_them() {
        let output =
            run_stream("// FREO-BEGIN\nlet debug = 1;\n// FREO-END\nlet x = 5;\n").unwrap();

        assert_eq!(output, "let x = 5;\n");
    }

    #[test]
    fn block_markers_are_case_insensitive() {
        let output = run_stream("// freo-begin\n// note\n// Freo-End\nkeep\n").unwrap();

        assert_eq!(output, "keep\n");
    }

    #[test]
    fn a_trailing_end_marker_does_not_close_a_block() {
        let error = run_stream("// FREO-BEGIN\ndrop me\nlet b = 2; // FREO-END\n").unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("line 1"), "{error}");
    }

    #[test]
    fn block_markers_do_not_nest_so_the_first_end_closes() {
        let output = run_stream(concat!(
            "// FREO-BEGIN\n",
            "// FREO-BEGIN\n",
            "// FREO-END\n",
            "keep me\n",
            "// FREO-END\n",
        ))
        .unwrap();

        assert_eq!(output, "keep me\n");
    }

    #[test]
    fn consecutive_blocks_are_each_removed() {
        let output = run_stream(concat!(
            "// FREO-BEGIN\n// one\n// FREO-END\n",
            "keep\n",
            "// FREO-BEGIN\n// two\n// FREO-END\n",
        ))
        .unwrap();

        assert_eq!(output, "keep\n");
    }

    #[test]
    fn unterminated_block_marker_is_an_error_so_nothing_is_persisted() {
        let error = run_stream("keep\n// FREO-BEGIN\n// note\n").unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        let message = error.to_string();
        assert!(message.contains("line 2"), "{message}");
        assert!(message.contains("FREO-END"), "{message}");
    }

    #[test]
    fn stray_end_marker_is_stripped_as_an_ordinary_keyword_comment() {
        let output = run_stream("let x = 1;\n// FREO-END\nlet y = 2;\n").unwrap();

        assert_eq!(output, "let x = 1;\nlet y = 2;\n");
    }

    #[test]
    fn block_markers_follow_a_custom_keyword() {
        let mut output = Vec::new();
        remove_matching_comments_from_stream(
            ";;",
            "ticket-123",
            Cursor::new(b";; Ticket-123-BEGIN\n;; note\n;; ticket-123-end\nkeep\n".to_vec()),
            &mut output,
        )
        .unwrap();

        assert_eq!(String::from_utf8(output).unwrap(), "keep\n");
    }
}
