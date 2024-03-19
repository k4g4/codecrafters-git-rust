use std::io::Write;

use crate::{cmds, utils};

#[derive(clap::Args)]
pub struct Args {
    /// Message for the commit
    #[arg(short)]
    pub message: String,
}

pub fn commit(message: String, mut output: impl Write) -> anyhow::Result<()> {
    let parent = utils::get_head()?;
    let mut commit_hash = vec![];
    cmds::commit_tree::commit_tree(parent.as_slice(), &message, None, &mut commit_hash)?;
    utils::update_head(std::str::from_utf8(&commit_hash)?.trim())?;

    Ok(write!(output, "New commit saved with message:\n{message}")?)
}
