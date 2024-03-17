use std::{
    ffi::OsString,
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
};

use anyhow::{ensure, Result};
use clap::Args;
use flate2::read::ZlibEncoder;
use sha1::{Digest, Sha1};

use crate::commands::{DOT_GIT, OBJECTS};

#[derive(Args)]
pub struct HashObjectArgs {
    /// Write the object to the .git database
    #[arg(short = 'w', default_value_t = false)]
    pub write: bool,

    /// Path to the object
    pub path: PathBuf,
}

pub fn hash_object(path: impl AsRef<Path>, write: bool) -> Result<()> {
    let contents = fs::read(path.as_ref())?;
    let header = format!("blob {}\0", contents.len());

    let mut hasher = Sha1::new();

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

            let mut filename = OsString::new();
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
