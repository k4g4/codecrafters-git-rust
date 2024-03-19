use std::{
    borrow::Borrow,
    ffi::OsString,
    fs,
    io::{self, Read, Write},
    os::unix::fs::PermissionsExt,
    path::Path,
};

use flate2::read::ZlibEncoder;
use sha1::{Digest, Sha1};

use crate::{utils, SHA_LEN};

const IGNORE: &[&str] = &[".git", ".vscode", "target"];

#[derive(clap::Args)]
pub struct Args {}

struct Entry {
    mode: u32,
    name: OsString,
    hash: [u8; SHA_LEN],
}

pub fn write_tree(mut output: impl Write) -> anyhow::Result<()> {
    let hash = write_tree_at(".")?;

    for byte in hash {
        write!(output, "{byte:02x}")?;
    }
    writeln!(output)?;

    Ok(())
}

fn write_tree_at(path: impl AsRef<Path>) -> anyhow::Result<[u8; SHA_LEN]> {
    let entries = {
        let mut entries = vec![];

        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let name = entry.file_name();

            if !IGNORE.contains(&name.to_string_lossy().borrow()) {
                entries.push(if entry.file_type()?.is_dir() {
                    Entry {
                        mode: 40_000,
                        name,
                        hash: write_tree_at(entry.path())?,
                    }
                } else {
                    let mut hash = [0u8; SHA_LEN];
                    super::hash_object::hash_object(
                        super::hash_object::Source::Path(entry.path()),
                        true,
                        false,
                        hash.as_mut(),
                    )?;
                    let metadata = entry.metadata()?;
                    let permissions = metadata.permissions().mode();
                    let mode = if metadata.is_symlink() {
                        120_000 // symlink
                    } else if permissions & 0o111 > 0 {
                        100_755 // executable
                    } else {
                        100_644 // normal file
                    };

                    Entry { mode, name, hash }
                });
            }
        }

        entries.sort_unstable_by(|left, right| {
            let (left, right) = (left.name.as_encoded_bytes(), right.name.as_encoded_bytes());
            let small_len = left.len().min(right.len());

            if left[..small_len] == right[..small_len] {
                // git prefers this edge case to be reversed for some reason
                left.len().cmp(&right.len()).reverse()
            } else {
                left.cmp(right)
            }
        });
        entries
    };

    let mut contents = vec![];

    for entry in entries {
        write!(contents, "{} ", entry.mode)?;
        contents.write_all(entry.name.as_encoded_bytes())?;
        contents.write_all(b"\0")?;
        contents.write_all(&entry.hash)?;
    }

    let header = format!("tree {}\0", contents.len());

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

    Ok(hash.into())
}
