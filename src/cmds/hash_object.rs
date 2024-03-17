use std::{
    fs,
    io::{self, Read},
    path::PathBuf,
};

use anyhow::ensure;
use flate2::read::ZlibEncoder;
use sha1::{Digest, Sha1};

use crate::cmds::{DOT_GIT, OBJECTS, SHA_LEN};

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
/// database if `write == true`.
pub fn hash_object(source: Source, write: bool) -> anyhow::Result<()> {
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

    let digest = hasher.finalize();
    for byte in &digest {
        print!("{byte:02x}");
    }
    println!();

    if write {
        let mut path: PathBuf = [DOT_GIT, OBJECTS, &format!("{:02x}", &digest[0])]
            .iter()
            .collect();

        if let Err(error) = fs::create_dir(&path) {
            ensure!(
                error.kind() == io::ErrorKind::AlreadyExists,
                "failed to create object subdirectory"
            );
        }

        path.push({
            use std::fmt::Write; //here to prevent conflict with io::Write

            let mut filename = String::with_capacity(SHA_LEN - 2);
            for byte in &digest[1..] {
                write!(&mut filename, "{byte:02x}")?;
            }
            filename
        });

        let mut file = fs::File::create(path)?;

        let mut compressor = ZlibEncoder::new(
            header.as_bytes().chain(contents.as_slice()),
            Default::default(), // default compression is level 6
        );

        io::copy(&mut compressor, &mut file)?;
    }

    Ok(())
}
