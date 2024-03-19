use std::{
    io::{self, Write},
    path::{Path, PathBuf},
};

use tokio::runtime::Runtime;

#[derive(clap::Args)]
pub struct Args {
    /// Remote repository
    pub remote: String,

    /// Repository path
    pub path: Option<PathBuf>,
}

pub fn clone(remote: &str, path: impl AsRef<Path>, mut output: impl Write) -> anyhow::Result<()> {
    Runtime::new()?.block_on(async {
        let res = reqwest::get(remote).await?;

        io::copy(&mut res.bytes().await?.as_ref(), &mut output)?;

        anyhow::Result::<_>::Ok(())
    })?;

    Ok(())
}
