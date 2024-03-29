use std::{
    borrow::Cow,
    fmt, fs,
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

    /// Object type
    #[arg(short, value_enum, default_value_t = Type::Blob)]
    pub r#type: Type,

    #[command(flatten)]
    pub source: SourceArgs,
}

#[derive(Clone, Copy, clap::ValueEnum)]
pub enum Type {
    Blob,
    Commit,
    Tree,
    Tag,
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::Blob => write!(f, "blob"),
            Type::Commit => write!(f, "commit"),
            Type::Tree => write!(f, "tree"),
            Type::Tag => write!(f, "tag"),
        }
    }
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

pub enum Source<'buf> {
    Path(PathBuf),
    Buf(&'buf [u8]),
    Stdin,
}

impl From<SourceArgs> for Source<'_> {
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
    write: bool,
    r#type: Type,
    source: Source,
    as_hex: bool,
    mut output: impl Write,
) -> anyhow::Result<()> {
    let contents = match source {
        Source::Path(path) => Cow::Owned(fs::read(path)?),
        Source::Stdin => {
            let mut buf = vec![];
            io::stdin().read_to_end(&mut buf)?;
            Cow::Owned(buf)
        }
        Source::Buf(buf) => Cow::Borrowed(buf),
    };

    let header = format!("{type} {}\0", contents.len());

    let mut hasher = Sha1::new();

    // could optimize this function by writing directly to a file and
    // the hasher at the same time, then moving the file to the final location.
    // would prevent needing the 'contents' in-memory buffer.
    io::copy(&mut header.as_bytes().chain(contents.as_ref()), &mut hasher)?;

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
            header.as_bytes().chain(contents.as_ref()),
            Default::default(), // default compression is level 6
        );

        io::copy(&mut compressor, &mut file)?;
    }

    Ok(())
}
