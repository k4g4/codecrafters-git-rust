use anyhow::{bail, ensure, Context, Result};
use clap::Args;
use flate2::read::ZlibDecoder;
use nom::{
    bytes::complete::tag,
    character::complete::{char, digit1},
};
use std::{
    fs,
    io::{self, Read, Write},
    path::Path,
};

use crate::commands::{DOT_GIT, OBJECTS, SHA_LEN};

#[derive(Args)]
pub struct CatFileArgs {
    /// The object's hash
    #[arg(short = 'p')]
    pub blob_sha: String,
}

/// Prints the contents of a blob object if it exists in .git
pub fn cat_file(blob_sha: &str, output: Option<&mut Vec<u8>>) -> Result<()> {
    let failed_context = || format!("failed to find {blob_sha}");

    ensure!(blob_sha.len() > 3, "object hash is not long enough");
    let (sha_dir, sha_file) = blob_sha.split_at(2);

    let entries = fs::read_dir(Path::new(DOT_GIT).join(OBJECTS))?;

    let entry = entries
        .filter_map(Result::ok)
        .find(|entry| sha_dir == entry.file_name())
        .with_context(failed_context)?;

    let entries = fs::read_dir(entry.path())?;

    let entry = entries
        .filter_map(Result::ok)
        .find(|entry| {
            entry.file_name().len() == SHA_LEN - 2
                && entry
                    .file_name()
                    .as_os_str()
                    .to_string_lossy()
                    .starts_with(sha_file)
        })
        .with_context(failed_context)?;

    // possible optimization: read up to the filesize,
    // then perform just one allocation for the next read
    let mut blob = vec![];
    ZlibDecoder::new(fs::File::open(entry.path())?).read_to_end(&mut blob)?;

    let contents = parse_blob(blob.as_slice()).context("failed to parse object file")?;

    if let Some(output) = output {
        output.write(contents)?;
    } else {
        let mut stdout = io::stdout().lock();
        stdout.write(contents)?;
    }

    Ok(())
}

/// Blob object contents parsed using nom
fn parse_blob(blob: &[u8]) -> Result<&[u8]> {
    let Ok((blob, _)) = tag::<_, _, ()>(b"blob ")(blob) else {
        bail!("object file is not a blob")
    };

    let Ok((blob, size)) = digit1::<_, ()>(blob) else {
        bail!("invalid blob size in object file")
    };

    let size = std::str::from_utf8(size)
        .context("invalid blob size in object file")?
        .parse::<usize>()
        .context("failed to parse blob size")?;

    let Ok((blob, _)) = char::<_, ()>('\0')(blob) else {
        bail!("unexpected character in object file")
    };
    ensure!(blob.len() == size, "blob size is incorrect");

    Ok(blob)
}
