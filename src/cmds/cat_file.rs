use anyhow::{anyhow, ensure, Context};
use flate2::read::ZlibDecoder;

use std::{
    fs,
    io::{self, Read, Write},
    path::Path,
};

use crate::{
    cmds::{DOT_GIT, OBJECTS, SHA_LEN},
    parsing,
};

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
pub fn cat_file(info: Info, hash: &str, output: Option<&mut dyn Write>) -> anyhow::Result<()> {
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
            let count = decoder.read(&mut buf)?;
            if count < 16 {
                decoder.read(&mut buf[count..])?;
            }
            let (_, r#type) = parsing::parse_type(&buf)?;
            write!(writer, "{type}")?;
        }

        Info::Size => {
            let mut buf = [0u8; 64];
            let count = decoder.read(&mut buf)?;
            if count < 16 {
                decoder.read(&mut buf[count..])?;
            }
            let (buf, _) = parsing::parse_type(&buf)?;
            let (buf, _) = nom::character::complete::char::<_, ()>(' ')(buf)
                .map_err(|_| anyhow!("unexpected character in object file"))?;
            let (_, header) = parsing::parse_header(&buf)?;
            write!(writer, "{}", header.size)?;
        }

        Info::Print => {
            // possible optimization: read up to the filesize,
            // then perform just one allocation for the next read
            let mut buf = vec![];
            decoder.read_to_end(&mut buf)?;
            let (contents, _) = parsing::parse_contents(buf.as_slice())?;
            writer.write(contents)?;
        }
    }

    Ok(())
}
