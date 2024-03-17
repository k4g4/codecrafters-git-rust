use std::io::Write;

use anyhow::ensure;

use crate::{
    utils::{self, EntryDisplay},
    SHA_LEN,
};

#[derive(clap::Args)]
pub struct Args {
    /// Recurse into subtrees
    #[arg(short)]
    pub recurse: bool,

    /// Show trees only
    #[arg(short = 'd')]
    pub trees_only: bool,

    /// Only display filenames
    #[arg(long)]
    pub name_only: bool,

    /// Abbreviate hashes
    #[arg(long, default_value_t = SHA_LEN as u8)]
    pub abbrev: u8,

    /// The object's hash
    pub hash: String,
}

pub fn ls_tree(
    recurse: bool,
    trees_only: bool,
    name_only: bool,
    abbrev: u8,
    hash: &str,
    mut output: impl Write,
) -> anyhow::Result<()> {
    ensure!(abbrev <= SHA_LEN as u8, "abbrev value must be <= {SHA_LEN}");

    for entry in utils::tree_level(hash, recurse)? {
        entry.display.set(Some(EntryDisplay {
            trees_only,
            name_only,
            abbrev,
        }));
        write!(output, "{entry}")?;
    }

    Ok(())
}
