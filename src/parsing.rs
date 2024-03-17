use std::fmt;

use nom::{
    branch::alt,
    bytes::complete::{tag, take_until, take_while_m_n},
    character::{
        complete::{char, digit1},
        is_digit,
    },
    multi::many0,
    sequence::separated_pair,
};

use crate::{utils, SHA_DISPLAY_LEN, SHA_LEN};

pub enum Type {
    Blob,
    Tree,
    Commit,
    Tag,
}

impl From<&[u8]> for Type {
    fn from(value: &[u8]) -> Self {
        match value {
            b"blob" => Self::Blob,
            b"tree" => Self::Tree,
            b"commit" => Self::Commit,
            b"tag" => Self::Tag,
            _ => unreachable!("parse_type can only read these four types"),
        }
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::Blob => write!(f, "blob"),
            Type::Tree => write!(f, "tree"),
            Type::Commit => write!(f, "commit"),
            Type::Tag => write!(f, "tag"),
        }
    }
}

pub struct Header {
    pub r#type: Type,
    pub size: usize,
}

pub struct Error(anyhow::Error);

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Error {
    fn new(msg: &str) -> nom::Err<Self> {
        nom::Err::Error(Self(anyhow::anyhow!("{msg}")))
    }
}

impl<I> nom::error::ParseError<I> for Error {
    fn from_error_kind(_input: I, kind: nom::error::ErrorKind) -> Self {
        Self(anyhow::anyhow!("{}", kind.description()))
    }

    fn append(_input: I, kind: nom::error::ErrorKind, other: Self) -> Self {
        Self(anyhow::anyhow!("{}\n{}", kind.description(), other.0))
    }
}

/// Object type
pub fn parse_type(object: &[u8]) -> nom::IResult<&[u8], Type, Error> {
    let mut object_type = alt((tag(b"blob"), tag(b"tree"), tag(b"commit"), tag(b"tag")));

    let (object, r#type) = object_type(object)?;

    Ok((object, r#type.into()))
}

/// Object size
fn parse_size(object: &[u8]) -> nom::IResult<&[u8], usize, Error> {
    let (object, size) = digit1(object)?;

    let size = std::str::from_utf8(size)
        .map_err(|_| Error::new("invalid size in object file"))?
        .parse::<usize>()
        .map_err(|_| Error::new("failed to parse size"))?;

    Ok((object, size))
}

/// Object header
pub fn parse_header(object: &[u8]) -> nom::IResult<&[u8], Header, Error> {
    let (object, (r#type, size)) = separated_pair(parse_type, char(' '), parse_size)(object)?;
    let (object, _) = char('\0')(object)?;

    Ok((object, Header { r#type, size }))
}

/// Object contents
pub fn parse_contents(object: &[u8]) -> nom::IResult<(), &[u8], Error> {
    let (object, Header { size, .. }) = parse_header(object)?;

    if object.len() != size {
        return Err(Error::new("object size is incorrect"));
    }

    Ok(((), object))
}

/// Tree entries
pub fn parse_tree(
    recurse: bool,
) -> impl Fn(&[u8]) -> nom::IResult<&[u8], Vec<utils::Entry>, Error> {
    move |object| {
        let (object, Header { r#type, size }) = parse_header(object)?;

        if !matches!(r#type, Type::Tree) {
            return Err(Error::new("object is not a tree"));
        }
        if object.len() != size {
            return Err(Error::new("object size is incorrect"));
        }

        many0(entry(recurse))(object)
    }
}

fn entry(recurse: bool) -> impl Fn(&[u8]) -> nom::IResult<&[u8], utils::Entry, Error> {
    move |object| {
        let (object, mode) = mode(object)?;
        let (object, _) = char(' ')(object)?;
        let (object, name) = name(object)?;
        let (object, _) = char('\0')(object)?;
        let (object, hash) = hash(object)?;
        let tree = mode == 40_000;
        let children = if tree && recurse {
            let hash = {
                use std::fmt::Write;

                let mut new_hash = String::with_capacity(SHA_DISPLAY_LEN);
                for byte in hash {
                    write!(&mut new_hash, "{byte:02x}").expect("writing to a string");
                }
                new_hash
            };

            Some(utils::tree_level(&hash, true).map_err(|error| nom::Err::Error(Error(error)))?)
        } else {
            None
        };

        Ok((
            object,
            utils::Entry {
                mode,
                hash,
                name,
                tree,
                children,
                display: Default::default(),
            },
        ))
    }
}

fn mode(object: &[u8]) -> nom::IResult<&[u8], u32, Error> {
    let (object, mode) = take_while_m_n(5, 6, is_digit)(object)?;

    Ok((
        object,
        std::str::from_utf8(mode)
            .expect("all digits")
            .parse()
            .expect("all digits"),
    ))
}

fn name(object: &[u8]) -> nom::IResult<&[u8], String, Error> {
    let (object, name) = take_until("\0")(object)?;

    Ok((object, String::from_utf8_lossy(name).into_owned()))
}

fn hash(object: &[u8]) -> nom::IResult<&[u8], [u8; SHA_LEN], Error> {
    let hash = object
        .get(..SHA_LEN)
        .ok_or_else(|| Error::new("failed to read hash"))?
        .try_into()
        .expect("got 20 bytes");

    Ok((&object[SHA_LEN..], hash))
}
