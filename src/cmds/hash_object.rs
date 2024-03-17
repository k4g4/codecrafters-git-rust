use std::{
    fs,
    io::{self, Read, Write},
    path::PathBuf,
};

use flate2::read::ZlibEncoder;
use sha1::{Digest, Sha1};

use crate::utils;

#[derive(clap::Args)]
pub struct Args {
    /// Write the object to the .git database
    #[arg(short)]
    pub write: bool,

    #[command(flatten)]
    pub source: SourceArgs,
}

#[derive(clap::Args)]
#[group(required = true, multiple = false)]
pub struct SourceArgs {
    /// Path to the object
    pub path: Option<PathBuf>,

    /// Read the object from stdin
    #[arg(long)]
    pub stdin: bool,
}

pub enum Source {
    Path(PathBuf),
    Stdin,
}

impl From<SourceArgs> for Source {
    fn from(SourceArgs { path, stdin }: SourceArgs) -> Self {
        match (path, stdin) {
            (None, true) => Self::Stdin,
            (Some(path), false) => Self::Path(path),
            _ => unreachable!("clap ensures at least one is present"),
        }
    }
}

/// Prints the sha1 hash of a file, and writes it to the .git
/// database as a blob if `write == true`.
pub fn hash_object(
    source: Source,
    write: bool,
    as_hex: bool,
    mut output: impl Write,
) -> anyhow::Result<()> {
    let contents = match source {
        Source::Path(path) => fs::read(path)?,
        Source::Stdin => {
            let mut buf = vec![];
            io::stdin().read_to_end(&mut buf)?;
            buf
        }
    };
    let header = format!("blob {}\0", contents.len());

    let mut hasher = Sha1::new();

    // could optimize this function by writing directly to a file and
    // the hasher at the same time, then moving the file to the final location.
    // would prevent needing the 'contents' in-memory buffer.
    io::copy(
        &mut header.as_bytes().chain(contents.as_slice()),
        &mut hasher,
    )?;

    let hash = hasher.finalize();
    if as_hex {
        for byte in &hash {
            write!(output, "{byte:02x}")?;
        }
        writeln!(output)?;
    } else {
        output.write_all(&hash)?;
    }

    if write {
        let mut file = utils::create_object(&hash.into())?;

        let mut compressor = ZlibEncoder::new(
            header.as_bytes().chain(contents.as_slice()),
            Default::default(), // default compression is level 6
        );

        io::copy(&mut compressor, &mut file)?;
    }

    Ok(())
}
