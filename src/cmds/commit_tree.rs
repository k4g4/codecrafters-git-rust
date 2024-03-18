use std::io::{self, Read, Write};

use anyhow::Context;
use flate2::read::ZlibEncoder;
use sha1::{Digest, Sha1};

use crate::{
    utils::{self, find_object, get_config_value},
    SHA_DISPLAY_LEN,
};

use super::write_tree::write_tree;

#[derive(clap::Args)]
pub struct Args {
    #[arg(short)]
    pub parents: Vec<String>,

    #[arg(short)]
    pub message: String,

    pub tree_hash: Option<String>,
}

pub fn commit_tree(
    parents: &[String],
    message: &str,
    tree_hash: Option<&str>,
    mut output: impl Write,
) -> anyhow::Result<()> {
    let name = get_config_value("user", "name")?.unwrap_or_else(|| "Anonymous".into());
    let email = get_config_value("user", "email")?.unwrap_or_else(|| "N/A".into());

    // hacky way to get the full hash if the hash is abbreviated
    let get_full_hash = |hash| -> anyhow::Result<_> {
        let hash = find_object(hash).context("failed to find parent")?;
        let hash = hash.to_str().expect("path is utf-8").replace('/', "");
        Ok(hash[hash.len() - SHA_DISPLAY_LEN..].to_owned())
    };

    let mut contents = vec![];
    write!(&mut contents, "tree ")?;

    if let Some(tree_hash) = tree_hash {
        let tree_hash = get_full_hash(tree_hash)?;
        writeln!(&mut contents, "{tree_hash}")?;
    } else {
        write_tree(&mut contents)?;
    }

    for parent in parents {
        let parent = get_full_hash(parent)?;
        writeln!(&mut contents, "parent {parent}")?;
    }

    writeln!(
        &mut contents,
        "author {name} <{email}> {}",
        chrono::Local::now().format("%s %z")
    )?;
    writeln!(
        &mut contents,
        "committer {name} <{email}> {}\n\n{message}",
        chrono::Local::now().format("%s %z")
    )?;

    let header = format!("commit {}\0", contents.len());

    let mut hasher = Sha1::new();
    io::copy(
        &mut header.as_bytes().chain(contents.as_slice()),
        &mut hasher,
    )?;
    let hash = hasher.finalize();

    let mut file = utils::create_object(&hash.into())?;
    let mut compressor = ZlibEncoder::new(
        header.as_bytes().chain(contents.as_slice()),
        Default::default(), // default compression is level 6
    );
    io::copy(&mut compressor, &mut file)?;

    for byte in hash {
        write!(&mut output, "{byte:02x}")?;
    }
    writeln!(&mut output)?;

    Ok(())
}
