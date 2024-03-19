use anyhow::Context;
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use crate::{CONFIG, DOT_GIT, HEAD, HEADS, OBJECTS, REFS, TAGS};

#[derive(clap::Args)]
pub struct Args {
    /// Path to use for initializing the repository
    pub path: Option<PathBuf>,
}

/// Initializes a new git repository by creating the .git directory and its subdirectories.
pub fn init(path: impl AsRef<Path>, mut output: impl Write) -> anyhow::Result<()> {
    let path = path.as_ref().join(DOT_GIT);
    let config = "
[core]
\trepositoryformatversion = 0
\tfilemode = true
\tbare = false
\tlogallrefupdates = true
";

    fs::create_dir(&path)
        .and_then(|_| fs::create_dir(path.join(OBJECTS)))
        .and_then(|_| fs::create_dir(path.join(REFS)))
        .and_then(|_| fs::create_dir(path.join(REFS).join(HEADS)))
        .and_then(|_| fs::create_dir(path.join(REFS).join(TAGS)))
        .and_then(|_| fs::write(path.join(HEAD), "ref: refs/heads/main\n"))
        .and_then(|_| fs::write(path.join(CONFIG), config))
        .with_context(|| format!("failed to initialize {}", path.display()))?;

    writeln!(output, "Initialized git directory")?;

    Ok(())
}
