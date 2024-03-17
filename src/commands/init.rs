use anyhow::{Context, Result};
use clap::Args;
use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::commands::{DOT_GIT, HEAD, OBJECTS, REFS};

#[derive(Args)]
pub struct InitArgs {
    /// Path to use for initializing the repository
    pub path: Option<PathBuf>,
}

/// Initializes a new git repository by creating the .git directory and its subdirectories
pub fn init(path: impl AsRef<Path>) -> Result<()> {
    let path = path.as_ref().join(DOT_GIT);

    fs::create_dir(&path)
        .and_then(|_| fs::create_dir(path.join(OBJECTS)))
        .and_then(|_| fs::create_dir(path.join(REFS)))
        .and_then(|_| fs::write(path.join(HEAD), "ref: refs/heads/main\n"))
        .with_context(|| format!("failed to initialize {}", path.display()))?;

    println!("Initialized git directory");

    Ok(())
}
