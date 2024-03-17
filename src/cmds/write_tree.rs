use std::{fs, io::Write, path::Path};

use crate::SHA_LEN;

#[derive(clap::Args)]
pub struct Args {}

pub fn write_tree(mut output: impl Write) -> anyhow::Result<()> {
    let hash = write_tree_at(".")?;

    for byte in hash {
        write!(output, "{byte:02x}")?;
    }
    writeln!(output)?;

    Ok(())
}

fn write_tree_at(path: impl AsRef<Path>) -> anyhow::Result<[u8; SHA_LEN]> {
    for entry in fs::read_dir(path)? {
        let entry = entry?;

        if entry.file_type()?.is_dir() {
            let hash = write_tree_at(entry.path())?;
        } else {
            //
        }
    }

    Ok([0u8; SHA_LEN])
}
