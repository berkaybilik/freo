use std::fs;
use std::path::{Path, PathBuf};
use std::collections::HashSet;
use std::str::FromStr;

use clap::Parser;
use freo::AppConfig;

#[derive(Clone, Debug)]
struct PathsFromJson {
    pub paths: Vec<PathBuf>,
}

// FREO: Are you going to remove this?
impl FromStr for PathsFromJson {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Accept JSON array of strings, e.g. ["a","b"]
        let raw: Vec<String> = serde_json::from_str(s)
            .map_err(|e| format!("failed to parse JSON array: {e}"))?;
        let paths = raw.into_iter().map(PathBuf::from).collect();
        Ok(PathsFromJson { paths })
    }
}

#[derive(Parser, Debug)]
#[command(name = "freo")]
#[command(about = "For the Reviewers Eyes Only", long_about = None)]
struct Cli {
    #[arg(long = "changed-files-json", value_name = "JSON", num_args = 1.., required = true)]
    changed_files_json: Vec<PathsFromJson>,

    #[arg(short = 'c', long = "config")]
    config: Option<PathBuf>,
}

impl Cli {
    fn unique_existing_paths(&self) -> HashSet<PathBuf> {
        self.changed_files_json
            .iter()
            .flat_map(|json_path_list| json_path_list.paths.iter().cloned())
            .filter(|p| p.exists())
            .collect()
    }
}

fn main() {
    let cli = Cli::parse();

    let files: Vec<PathBuf> = cli.unique_existing_paths().into_iter().collect();
    if files.is_empty() {
        eprintln!("No input files found on disk; nothing to do.");
        std::process::exit(0);
    }

    let config: AppConfig = if let Some(config_path) = cli.config.as_ref() {
        match read_config(config_path.as_path()) {
            Ok(cfg) => cfg,
            Err(err) => {
                eprintln!("Warning: {err}");
                AppConfig::new(None)
            }
        }
    } else {
        AppConfig::new(None)
    };
    
    freo::run(&config, &files);
}

fn read_config(config_path: &Path) -> Result<AppConfig, String> {
    if !config_path.exists() {
        return Err(format!("Config not found at {}", config_path.display()));
    }

    let contents = fs::read_to_string(config_path)
        .map_err(|e| format!("Failed to read {}: {e}", config_path.display()))?;

    let cfg: AppConfig = serde_json::from_str(&contents)
        .map_err(|e| format!("Failed to parse JSON in {}: {e}", config_path.display()))?;

    Ok(cfg)
}
