use std::fmt;

use nom::{
    branch::alt,
    bytes::complete::tag,
    character::complete::{char, digit1},
    sequence::separated_pair,
};

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

#[derive(Debug)]
pub struct Error(anyhow::Error);

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
pub fn parse_contents(object: &[u8]) -> nom::IResult<&[u8], (), Error> {
    let (object, Header { size, .. }) = parse_header(object)?;

    if object.len() != size {
        return Err(Error::new("object size is incorrect"));
    }

    Ok((object, ()))
}
