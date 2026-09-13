use std::io;
use std::path::{Path, PathBuf};

pub mod comment;
pub mod config;
pub mod processor;

pub use comment::CommentTokenResolver;
pub use config::AppConfig;

pub fn run(
    config: &AppConfig,
    file_paths: &[PathBuf],
    resolver: &CommentTokenResolver,
) -> io::Result<()> {
    println!(
        "Running for the reviewers eyes only with keyword: {}",
        config.keyword()
    );

    println!("Finding matching lines...");

    // Rewrite every file into a temporary file first and only write the batch
    // back once all of it is known to be good. A file rejected halfway through
    // must not leave the files ahead of it already modified.
    let mut prepared = Vec::with_capacity(file_paths.len());

    for file_path in file_paths {
        println!("Processing file: {}", file_path.display());
        let Some(token) = resolver.token_for(file_path) else {
            println!(
                "Skipping {}: no comment token mapping for extension",
                file_path.display()
            );
            continue;
        };
        prepared.push(
            processor::prepare(token, config.keyword(), file_path.as_path())
                .map_err(|err| name_the_file(file_path, err))?,
        );
    }

    for file in prepared {
        let file_path = file.path().to_path_buf();
        processor::commit(file).map_err(|err| name_the_file(&file_path, err))?;
    }

    Ok(())
}

fn name_the_file(file_path: &Path, err: io::Error) -> io::Error {
    io::Error::new(err.kind(), format!("{}: {}", file_path.display(), err))
}
