use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use clap::Parser;
use for_the_reviewers_eyes_only::AppConfig;

#[derive(Parser, Debug)]
#[command(name = "freo")]
#[command(about = "For the Reviewers Eyes Only", long_about = None)]
struct Cli {
    #[arg(short = 'b', long = "base-branch")]
    base_branch: Option<String>,

    #[arg(short = 'c', long = "config")]
    config: Option<String>,
}

fn main() {
    let cli = Cli::parse();

    let base_branch = cli
        .base_branch
        .or_else(|| env::var("GITHUB_BASE_REF").ok())
        .unwrap_or_else(|| {
            eprintln!("Error: --base-branch not set and GITHUB_BASE_REF not available.");
            std::process::exit(2);
        });

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
    
    for_the_reviewers_eyes_only::run(&config, &base_branch);
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

    let contents = fs::read_to_string(path)
        .map_err(|e| format!("Failed to read {config_path}: {e}"))?;

    let cfg: AppConfig = serde_json::from_str(&contents)
        .map_err(|e| format!("Failed to parse JSON in {config_path}: {e}"))?;

    Ok(cfg)
}
