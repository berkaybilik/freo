use std::{fs::File, io::{BufRead, BufReader, BufWriter, Write}};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use tempfile::NamedTempFile;

use serde::Deserialize;
use regex::{Regex, RegexBuilder};

const DEFAULT_KEYWORD: &str = "FREO";

#[derive(Debug, Deserialize)]
#[serde(from = "AppConfigRaw")]
pub struct AppConfig {
    keyword: String,
}

// FREO: Get rid of me!!!
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

pub fn run(config: &AppConfig, file_paths: &Vec<std::path::PathBuf>) {
    println!(
        "Running for the reviewers eyes only with keyword: {}",
        config.keyword()
    );

    const COMMENT_TOKEN: &str = "//";

    println!("Finding matching lines...");

    for file_path in file_paths {
        println!("Processing file: {}", file_path.display());
        remove_matching_lines(COMMENT_TOKEN, config.keyword(), file_path);
    }
}

fn build_keyword_comment_pattern(comment_token: &str, keyword: &str) -> Regex {
    let comment_token_literal = regex::escape(comment_token);
    let keyword_literal = regex::escape(keyword);

    let pattern_format = format!(r"{}\s*{}\b:?", comment_token_literal, keyword_literal);
    RegexBuilder::new(&pattern_format)
        .case_insensitive(true)
        .build()
        .unwrap_or_else(|e| panic!("Failed to build regex: {}", e))
}

fn remove_matching_lines(comment_token: &str, keyword: &str, file_path: &std::path::PathBuf) {
    let parent = file_path.parent().unwrap_or(std::path::Path::new("."));
    let temp_file = NamedTempFile::new_in(parent).unwrap();

    if let Err(e) = File::create(&temp_file.path()) {
        eprintln!("Error: failed to create temp file in {}: {}", parent.display(), e);
        return;
    }

    let pattern: Regex = build_keyword_comment_pattern(comment_token, keyword);
    
    let reader = BufReader::new(File::open(file_path).unwrap()); 
    let mut writer = BufWriter::new(temp_file.as_file());

    // Pending line is used to handle the case where the last line does not match and needs to be written to the file
    // without a newline character. Example case: Suppose the file ends with:
    // a
    // If we just did writeln!("a") then, in the new file it would be:
    // a\n
    // We don't want this because we have to preserve all original data apart from the lines that match the pattern
    let mut pending_line: Option<String> = None;
    for line in reader.lines() {
        let line = line.unwrap();
        if !pattern.is_match(&line) {
            if let Some(prev) = pending_line.replace(line) {
                writeln!(writer, "{}", prev).unwrap();
            }
        }
    }
    if let Some(last) = pending_line {
        write!(writer, "{}", last).unwrap();
    }

    // Flush buffered writes before syncing to disk
    writer.flush().unwrap();
    // Release the borrow on the underlying file before further ops
    drop(writer); // Note to self: May avoid this if writer is moved to a separate scope

    // Best-effort: copy only commonly preserved metadata (permissions/mode)
    if let Ok(orig_meta) = std::fs::metadata(file_path) {
        let mut perms = orig_meta.permissions();
        perms.set_mode(orig_meta.mode());
        let _ = std::fs::set_permissions(temp_file.path(), perms); 
    }

    // Ensure all data and metadata are durably written to the temp file
    temp_file.as_file().sync_all().unwrap();

    // Atomically replace the original file by persisting within the same directory
    // Note to self: There may be a better way to scope reader and writer so that they are consumed before persisting
    temp_file.persist(file_path).unwrap();

    // Durability: fsync the parent directory to persist the rename
    if let Ok(dir) = File::open(parent) { let _ = dir.sync_all(); }
}
