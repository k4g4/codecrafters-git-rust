use anyhow::Context;
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use crate::{DOT_GIT, HEAD, OBJECTS, REFS};

#[derive(clap::Args)]
pub struct Args {
    /// Path to use for initializing the repository
    pub path: Option<PathBuf>,
}

/// Initializes a new git repository by creating the .git directory and its subdirectories.
pub fn init(path: impl AsRef<Path>, mut output: impl Write) -> anyhow::Result<()> {
    let path = path.as_ref().join(DOT_GIT);

    fs::create_dir(&path)
        .and_then(|_| fs::create_dir(path.join(OBJECTS)))
        .and_then(|_| fs::create_dir(path.join(REFS)))
        .and_then(|_| fs::write(path.join(HEAD), "ref: refs/heads/main\n"))
        .with_context(|| format!("failed to initialize {}", path.display()))?;

    writeln!(output, "Initialized git directory")?;

    Ok(())
}
