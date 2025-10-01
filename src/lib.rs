use std::{fs::File, io::{BufRead, BufReader}};

use serde::Deserialize;
use regex::{Regex, RegexBuilder};

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

pub fn run(config: &AppConfig, file_path: &std::path::PathBuf) {
    println!(
        "Running for the reviewers eyes only with keyword: {} for file: {}", 
        config.keyword(), file_path.display()
    );

    const COMMENT_TOKEN: &str = "//";

    println!("Finding matching lines...");

    let matches = find_matching_lines(COMMENT_TOKEN, config.keyword(), file_path);
    println!("Found {} matches", matches.len());

    for matched_line in matches {
        println!("{}", matched_line);
    };
}

fn build_keyword_comment_regexp(comment_token: &str, keyword: &str) -> Regex {
    let comment_token_literal = regex::escape(comment_token);
    let keyword_literal = regex::escape(keyword);

    let pattern_format = format!(r"{}\s*{}\b:?", comment_token_literal, keyword_literal);
    RegexBuilder::new(&pattern_format)
        .case_insensitive(true)
        .build()
        .unwrap_or_else(|e| panic!("Failed to build regex: {}", e))
}

fn find_matching_lines(comment_token: &str, keyword: &str, file_path: &std::path::PathBuf) -> Vec<String> {
    let pattern: Regex = build_keyword_comment_regexp(comment_token, keyword);

    let file = File::open(file_path).unwrap_or_else(|e| panic!("Failed to open file: {}", e));
    let reader = BufReader::new(file);

    let mut matches= Vec::new();

    for read_line in reader.lines() {
        let line = read_line.unwrap_or_else(|e| panic!("Failed to read line: {}", e));
        if pattern.is_match(&line) {
            matches.push(line);
        }
    };

    matches
}
