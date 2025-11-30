use std::path::PathBuf;

pub mod comment;
pub mod config;
pub mod processor;

pub use comment::CommentTokenResolver;
pub use config::AppConfig;

pub fn run(config: &AppConfig, file_paths: &[PathBuf], resolver: &CommentTokenResolver) {
    println!(
        "Running for the reviewers eyes only with keyword: {}",
        config.keyword()
    );

    println!("Finding matching lines...");

    for file_path in file_paths {
        println!("Processing file: {}", file_path.display());
        let Some(token) = resolver.token_for(file_path) else {
            println!(
                "Skipping {}: no comment token mapping for extension",
                file_path.display()
            );
            continue;
        };
        processor::remove_matching_comments(token, config.keyword(), file_path.as_path());
    }
}
