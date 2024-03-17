use std::{
    cell::Cell,
    fmt, fs,
    io::Read,
    path::{Path, PathBuf},
};

use anyhow::{ensure, Context};
use flate2::read::ZlibDecoder;

use crate::{parsing, DOT_GIT, OBJECTS, SHA_DISPLAY_LEN, SHA_LEN};

#[derive(Clone, Copy)]
pub struct EntryDisplay {
    pub trees_only: bool,
    pub name_only: bool,
    pub abbrev: u8,
}

pub struct Entry {
    pub display: Cell<Option<EntryDisplay>>,
    pub mode: u32,
    pub hash: [u8; SHA_LEN],
    pub name: String,
    pub tree: bool,
    pub children: Option<Vec<Entry>>,
}

impl fmt::Display for Entry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let display = self
            .display
            .get()
            .expect("assigned before display is called");

        if display.trees_only && !self.tree {
            return Ok(());
        };

        if !display.name_only {
            write!(f, "{:06}\t", self.mode)?;
            write!(f, "{}\t", if self.tree { "tree" } else { "blob" })?;
            for byte in &self.hash[..display.abbrev as usize] {
                write!(f, "{byte:02x}")?;
            }
            write!(f, "\t")?;
        }

        writeln!(f, "{}", self.name)?;

        if let Some(children) = self.children.as_deref() {
            for child in children {
                child.display.set(self.display.get());
                write!(f, "{child}")?;
            }
        }

        Ok(())
    }
}

pub fn find_object(hash: &str) -> anyhow::Result<PathBuf> {
    let failed_context = || format!("failed to find {hash}");

    ensure!(hash.len() > 3, "object hash is not long enough");
    let (sha_dir, sha_file) = hash.split_at(2);

    let entries = fs::read_dir(Path::new(DOT_GIT).join(OBJECTS))?;

    let entry = entries
        .filter_map(Result::ok)
        .find(|entry| sha_dir == entry.file_name())
        .with_context(failed_context)?;

    let entries = fs::read_dir(entry.path())?;

    let entry = entries
        .filter_map(Result::ok)
        .find(|entry| {
            entry.file_name().len() == SHA_DISPLAY_LEN - 2
                && entry
                    .file_name()
                    .as_os_str()
                    .to_string_lossy()
                    .starts_with(sha_file)
        })
        .with_context(failed_context)?;

    Ok(entry.path())
}

pub fn tree_level(hash: &str, recurse: bool) -> anyhow::Result<Vec<Entry>> {
    let path = find_object(hash)?;

    let mut buf = vec![];
    ZlibDecoder::new(fs::File::open(path)?).read_to_end(&mut buf)?;

    let (_, entries) = parsing::parse_tree(recurse)(buf.as_slice())?;

    Ok(entries)
}
