use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use clap::Parser;
use freo::{AppConfig, CommentTokenResolver};

#[derive(Parser, Debug)]
#[command(name = "freo")]
#[command(about = "For the Reviewers Eyes Only", long_about = None)]
struct Cli {
    #[arg(short = 'f', long = "file", value_name = "FILE", num_args = 1.., required = true)]
    files: Vec<std::path::PathBuf>,

    #[arg(short = 'c', long = "config")]
    config: Option<String>,

    #[arg(
        long = "comment-map",
        value_name = "FILE",
        help = "JSON file of {\"ext\": \"token\"} pairs"
    )]
    comment_map: Option<String>,
}

fn main() {
    let cli = Cli::parse();

    let missing: Vec<&std::path::PathBuf> = cli.files.iter().filter(|p| !p.exists()).collect();
    if !missing.is_empty() {
        eprintln!("Error: file not found at {}", missing[0].display());
        std::process::exit(2);
    }

    let config_path: String = cli
        .config
        .unwrap_or_else(|| default_config_path().to_string_lossy().into_owned());

    let config: AppConfig = match read_config(&config_path) {
        Ok(cfg) => cfg,
        Err(err) => {
            eprintln!("Warning: {err}");
            AppConfig::new(None)
        }
    };

    let comment_map = cli
        .comment_map
        .as_deref()
        .map(load_comment_map)
        .transpose()
        .unwrap_or_else(|err| {
            eprintln!("Error: {err}");
            std::process::exit(3);
        });

    let resolver = CommentTokenResolver::new(comment_map);

    freo::run(&config, &cli.files, &resolver);
}

fn default_config_path() -> PathBuf {
    if let Ok(workspace) = env::var("GITHUB_WORKSPACE") {
        let p = PathBuf::from(workspace).join("freo.json");
        return p;
    }

    if let Ok(current) = env::current_dir() {
        for ancestor in current.ancestors() {
            let git_dir = ancestor.join(".git");
            if git_dir.exists() {
                return ancestor.join("freo.json");
            }
        }
        return current.join("freo.json");
    }

    PathBuf::from("freo.json")
}

fn read_config(config_path: &str) -> Result<AppConfig, String> {
    let path = Path::new(config_path);
    if !path.exists() {
        return Err(format!("Config not found at {config_path}"));
    }

    let contents =
        fs::read_to_string(path).map_err(|e| format!("Failed to read {config_path}: {e}"))?;

    let cfg: AppConfig = serde_json::from_str(&contents)
        .map_err(|e| format!("Failed to parse JSON in {config_path}: {e}"))?;

    Ok(cfg)
}

fn load_comment_map(path: &str) -> Result<HashMap<String, String>, String> {
    let contents =
        fs::read_to_string(path).map_err(|e| format!("Failed to read comment map {path}: {e}"))?;
    let map: HashMap<String, String> = serde_json::from_str(&contents)
        .map_err(|e| format!("Failed to parse JSON in {path}: {e}"))?;

    Ok(map)
}
