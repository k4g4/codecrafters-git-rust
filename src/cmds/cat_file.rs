use anyhow::{anyhow, bail, ensure, Context, Result};
use flate2::read::ZlibDecoder;
use nom::{
    branch::alt,
    bytes::complete::tag,
    character::complete::{char, digit1},
};
use std::{
    fs,
    io::{self, Read, Write},
    path::Path,
};

use crate::cmds::{DOT_GIT, OBJECTS, SHA_LEN};

#[derive(clap::Args)]
pub struct Args {
    #[command(flatten)]
    pub info: InfoArgs,

    /// The object's hash
    pub hash: String,
}

#[derive(clap::Args)]
#[group(required = true, multiple = false)]
pub struct InfoArgs {
    /// Print the object's type
    #[arg(short)]
    pub r#type: bool,

    /// Print the object's size
    #[arg(short)]
    pub size: bool,

    /// Print the object's contents
    #[arg(short)]
    pub print: bool,
}

pub enum Info {
    Type,
    Size,
    Print,
}

impl From<InfoArgs> for Info {
    fn from(
        InfoArgs {
            r#type,
            size,
            print,
        }: InfoArgs,
    ) -> Self {
        match (r#type, size, print) {
            (true, _, _) => Self::Type,
            (_, true, _) => Self::Size,
            (_, _, true) => Self::Print,
            _ => unreachable!("clap ensures at least one is present"),
        }
    }
}

/// Prints an object's type, size, or contents if it exists in the .git database.
pub fn cat_file(info: Info, hash: &str, output: Option<&mut dyn Write>) -> Result<()> {
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
            entry.file_name().len() == SHA_LEN - 2
                && entry
                    .file_name()
                    .as_os_str()
                    .to_string_lossy()
                    .starts_with(sha_file)
        })
        .with_context(failed_context)?;

    let mut decoder = ZlibDecoder::new(fs::File::open(entry.path())?);

    let mut stdout = None;
    let writer = output.unwrap_or_else(|| stdout.insert(io::stdout().lock()));

    match info {
        Info::Type => {
            let mut buf = [0u8; 64];
            decoder.read_exact(&mut buf)?;
            let (_, r#type) = parse_type(&buf)?;
            writer.write(r#type)?;
        }

        Info::Size => {
            let mut buf = [0u8; 64];
            decoder.read_exact(&mut buf)?;
            let (buf, _) = parse_type(&buf)?;
            let (buf, _) = char::<_, ()>(' ')(buf)
                .map_err(|_| anyhow!("unexpected character in object file"))?;
            let (_, size) = parse_size(&buf)?;
            write!(writer, "{size}")?;
        }

        Info::Print => {
            // possible optimization: read up to the filesize,
            // then perform just one allocation for the next read
            let mut buf = vec![];
            decoder.read_to_end(&mut buf)?;
            let contents = parse_contents(buf.as_slice())?;
            writer.write(contents)?;
        }
    }

    Ok(())
}

/// Object type parsed using nom
fn parse_type(object: &[u8]) -> Result<(&[u8], &[u8])> {
    let mut object_type = alt((
        tag::<_, _, ()>(b"blob"),
        tag(b"tree"),
        tag(b"commit"),
        tag(b"tag"),
    ));

    let Ok((object, r#type)) = object_type(object) else {
        bail!("invalid object type")
    };

    Ok((object, r#type))
}

/// Object size parsed using nom
fn parse_size(object: &[u8]) -> Result<(&[u8], usize)> {
    let Ok((object, size)) = digit1::<_, ()>(object) else {
        bail!("invalid size in object file")
    };

    let size = std::str::from_utf8(size)
        .context("invalid size in object file")?
        .parse::<usize>()
        .context("failed to parse size")?;

    Ok((object, size))
}

/// Object contents parsed using nom
fn parse_contents(object: &[u8]) -> Result<&[u8]> {
    let (object, _) = parse_type(object)?;

    let Ok((object, _)) = char::<_, ()>(' ')(object) else {
        bail!("unexpected character in object file")
    };

    let (object, size) = parse_size(object)?;

    let Ok((object, _)) = char::<_, ()>('\0')(object) else {
        bail!("unexpected character in object file")
    };

    ensure!(object.len() == size, "object size is incorrect");

    Ok(object)
}
