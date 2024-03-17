use flate2::read::ZlibDecoder;

use std::{
    fs,
    io::{Read, Write},
};

use crate::{parsing, utils};

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
pub fn cat_file(info: Info, hash: &str, mut output: impl Write) -> anyhow::Result<()> {
    let path = utils::find_object(hash)?;

    let mut decoder = ZlibDecoder::new(fs::File::open(path)?);

    match info {
        Info::Type => {
            let mut buf = [0u8; 64];
            let count = decoder.read(&mut buf)?;
            if count < 16 {
                decoder.read(&mut buf[count..])?;
            }
            let (_, r#type) = parsing::parse_type(&buf)?;
            write!(output, "{type}")?;
        }

        Info::Size => {
            let mut buf = [0u8; 64];
            let count = decoder.read(&mut buf)?;
            if count < 16 {
                decoder.read(&mut buf[count..])?;
            }
            let (_, parsing::Header { size, .. }) = parsing::parse_header(&buf)?;
            write!(output, "{size}")?;
        }

        Info::Print => {
            // possible optimization: read up to the filesize,
            // then perform just one allocation for the next read
            let mut buf = vec![];
            decoder.read_to_end(&mut buf)?;
            let (contents, r#type) = parsing::parse_contents(buf.as_slice())?;

            // dispatch to ls_tree for tree objects
            if matches!(r#type, parsing::Type::Tree) {
                super::ls_tree::ls_tree(false, false, false, 20, hash, output)?;
            } else {
                output.write(contents)?;
            }
        }
    }

    Ok(())
}
