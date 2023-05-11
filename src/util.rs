use chrono::Datelike;
use nom::character::complete::{char, u32 as nom_u32};
use nom::combinator::{all_consuming, opt};
use nom::sequence::preceded;
use nom::{Finish, IResult};
use serde::de::{Deserializer, Unexpected, Visitor};
use serde::ser::Serializer;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::iter::FusedIterator;
use std::str::FromStr;
use thiserror::Error;

pub fn this_year() -> i32 {
    chrono::Local::now().year()
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StringLines {
    content: String,
}

impl StringLines {
    pub fn new(content: String) -> StringLines {
        StringLines { content }
    }
}

impl Iterator for StringLines {
    type Item = String;

    fn next(&mut self) -> Option<String> {
        if self.content.is_empty() {
            return None;
        }
        let i = self.content.find('\n').unwrap_or(self.content.len() - 1);
        let mut line = self.content.drain(0..=i).collect::<String>();
        if line.ends_with('\n') {
            line.pop();
            if line.ends_with('\r') {
                line.pop();
            }
        }
        Some(line)
    }
}

impl FusedIterator for StringLines {}

#[derive(Copy, Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct RustVersion {
    major: u32,
    minor: u32,
    patch: Option<u32>,
}

impl FromStr for RustVersion {
    type Err = ParseRustVersionError;

    fn from_str(s: &str) -> Result<RustVersion, ParseRustVersionError> {
        match all_consuming(rust_version)(s).finish() {
            Ok((_, rv)) => Ok(rv),
            Err(_) => Err(ParseRustVersionError),
        }
    }
}

impl fmt::Display for RustVersion {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)?;
        if let Some(patch) = self.patch {
            write!(f, ".{}", patch)?;
        }
        Ok(())
    }
}

impl Serialize for RustVersion {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.to_string().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for RustVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(RustVersionVisitor)
    }
}

struct RustVersionVisitor;

impl<'de> Visitor<'de> for RustVersionVisitor {
    type Value = RustVersion;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a Rust version of the form X.Y or X.Y.Z")
    }

    fn visit_str<E>(self, input: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        input
            .parse::<RustVersion>()
            .map_err(|_| E::invalid_value(Unexpected::Str(input), &self))
    }
}

#[derive(Copy, Clone, Debug, Error, Eq, PartialEq)]
#[error("invalid Rust version/MSRV")]
pub struct ParseRustVersionError;

fn rust_version(input: &str) -> IResult<&str, RustVersion> {
    let (input, major) = nom_u32(input)?;
    let (input, _) = char('.')(input)?;
    let (input, minor) = nom_u32(input)?;
    let (input, patch) = opt(preceded(char('.'), nom_u32))(input)?;
    Ok((
        input,
        RustVersion {
            major,
            minor,
            patch,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_lines() {
        let mut iter = StringLines::new("foo\r\nbar\n\nbaz\n".into());
        assert_eq!(iter.next().unwrap(), "foo");
        assert_eq!(iter.next().unwrap(), "bar");
        assert_eq!(iter.next().unwrap(), "");
        assert_eq!(iter.next().unwrap(), "baz");
        assert_eq!(iter.next(), None);
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn string_lines_no_final_newline() {
        let mut iter = StringLines::new("foo\nbar\n\r\nbaz".into());
        assert_eq!(iter.next().unwrap(), "foo");
        assert_eq!(iter.next().unwrap(), "bar");
        assert_eq!(iter.next().unwrap(), "");
        assert_eq!(iter.next().unwrap(), "baz");
        assert_eq!(iter.next(), None);
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn two_part_rust_version() {
        let rv = "1.69".parse::<RustVersion>().unwrap();
        assert_eq!(
            rv,
            RustVersion {
                major: 1,
                minor: 69,
                patch: None
            }
        );
        assert_eq!(rv.to_string(), "1.69");
    }

    #[test]
    fn three_part_rust_version() {
        let rv = "1.69.0".parse::<RustVersion>().unwrap();
        assert_eq!(
            rv,
            RustVersion {
                major: 1,
                minor: 69,
                patch: Some(0)
            }
        );
        assert_eq!(rv.to_string(), "1.69.0");
    }
}
