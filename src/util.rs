#![allow(dead_code)]
use chrono::Datelike;
use nom::bytes::complete::tag;
use nom::character::complete::{char, i32 as nom_i32, space0, space1, u32 as nom_u32};
use nom::combinator::{all_consuming, opt, rest};
use nom::multi::separated_list1;
use nom::sequence::{preceded, tuple};
use nom::{Finish, IResult};
use rangemap::RangeInclusiveSet;
use renamore::rename_exclusive;
use semver::Version;
use serde::de::{Deserializer, Unexpected, Visitor};
use serde::ser::Serializer;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fmt;
use std::fs::{read_dir, remove_dir, FileType};
use std::io;
use std::iter::FusedIterator;
use std::ops::RangeInclusive;
use std::path::{Path, PathBuf, StripPrefixError};
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
    let (input, _) = opt(char('v'))(input)?;
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

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Bump {
    Major,
    Minor,
    Patch,
}

pub fn bump_version(v: Version, level: Bump) -> Version {
    match level {
        Bump::Major => Version::new(v.major + 1, 0, 0),
        Bump::Minor => Version::new(v.major, v.minor + 1, 0),
        Bump::Patch => Version::new(v.major, v.minor, v.patch + 1),
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CopyrightLine {
    years: RangeInclusiveSet<i32>,
    authors: String,
}

impl CopyrightLine {
    pub fn add_year(&mut self, year: i32) {
        self.years.insert(year..=year);
    }
}

impl FromStr for CopyrightLine {
    type Err = ParseCopyrightError;

    fn from_str(s: &str) -> Result<CopyrightLine, ParseCopyrightError> {
        match all_consuming(copyright)(s).finish() {
            Ok((_, c)) => Ok(c),
            Err(_) => Err(ParseCopyrightError),
        }
    }
}

impl fmt::Display for CopyrightLine {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Copyright (c) ")?;
        let mut first = true;
        for rng in self.years.iter() {
            if !std::mem::replace(&mut first, false) {
                write!(f, ", ")?;
            }
            if rng.start() == rng.end() {
                write!(f, "{}", rng.start())?;
            } else {
                write!(f, "{}-{}", rng.start(), rng.end())?;
            }
        }
        write!(f, " {}", self.authors)?;
        Ok(())
    }
}

#[derive(Copy, Clone, Debug, Error, Eq, PartialEq)]
#[error("invalid copyright line")]
pub struct ParseCopyrightError;

fn copyright(input: &str) -> IResult<&str, CopyrightLine> {
    let (input, _) = tuple((tag("Copyright"), space1, tag("(c)"), space1))(input)?;
    let (input, ranges) = separated_list1(tuple((space0, char(','), space0)), year_range)(input)?;
    let (input, _) = space1(input)?;
    let (input, authors) = rest(input)?;
    Ok((
        input,
        CopyrightLine {
            years: ranges.into_iter().collect(),
            authors: authors.into(),
        },
    ))
}

fn year_range(input: &str) -> IResult<&str, RangeInclusive<i32>> {
    let (input, start) = nom_i32(input)?;
    let (input, end) = opt(preceded(tuple((space0, char('-'), space0)), nom_i32))(input)?;
    let rng = if let Some(end) = end {
        start..=end
    } else {
        start..=start
    };
    Ok((input, rng))
}

pub fn move_dirtree_into(src: &Path, dest: &Path) -> Result<(), MoveDirtreeIntoError> {
    use MoveDirtreeIntoError::*;
    let mut stack = vec![DirWithEntries::new(src)?];
    while let Some(entries) = stack.last_mut() {
        match entries.pop_front() {
            Some((entry, ftype)) if ftype.is_dir() => {
                stack.push(DirWithEntries::new(&entry)?);
            }
            Some((entry, _)) => {
                let relpath = entry.strip_prefix(src).map_err(|source| Relpath {
                    source,
                    path: entry.clone(),
                    base: src.into(),
                })?;
                let target = dest.join(relpath);
                rename_exclusive(&entry, &target).map_err(|source| Rename {
                    source,
                    src: entry.clone(),
                    dest: target.clone(),
                })?;
                entries.pop_front();
            }
            None => {
                remove_dir(&entries.dirpath).map_err(|source| Rmdir {
                    source,
                    path: entries.dirpath.clone(),
                })?;
                stack.pop();
            }
        }
    }
    Ok(())
}

#[derive(Debug, Error)]
pub enum MoveDirtreeIntoError {
    #[error("could not stat path: {}: {source}", .path.display())]
    Stat { source: io::Error, path: PathBuf },
    #[error("could not open directory for reading: {}: {source}", .path.display())]
    Opendir { source: io::Error, path: PathBuf },
    #[error("could not fetch entry from directory: {}: {source}", .path.display())]
    Readdir { source: io::Error, path: PathBuf },
    #[error("could not remove directory: {}: {source}", .path.display())]
    Rmdir { source: io::Error, path: PathBuf },
    #[error("path {} beneath {} was not relative to it", .path.display(), .base.display())]
    Relpath {
        source: StripPrefixError,
        path: PathBuf,
        base: PathBuf,
    },
    #[error("could not rename path {} to {}: {source}", .src.display(), .dest.display())]
    Rename {
        source: io::Error,
        src: PathBuf,
        dest: PathBuf,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DirWithEntries {
    dirpath: PathBuf,
    entries: VecDeque<(PathBuf, FileType)>,
}

impl DirWithEntries {
    fn new(dirpath: &Path) -> Result<DirWithEntries, MoveDirtreeIntoError> {
        use MoveDirtreeIntoError::*;
        let mut entries = VecDeque::new();
        for entry in read_dir(dirpath).map_err(|source| Opendir {
            source,
            path: dirpath.into(),
        })? {
            let entry = entry.map_err(|source| Readdir {
                source,
                path: dirpath.into(),
            })?;
            let ftype = entry.file_type().map_err(|source| Stat {
                source,
                path: entry.path(),
            })?;
            entries.push_back((entry.path(), ftype));
        }
        Ok(DirWithEntries {
            dirpath: dirpath.into(),
            entries,
        })
    }

    fn pop_front(&mut self) -> Option<(PathBuf, FileType)> {
        self.entries.pop_front()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

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

    #[test]
    fn v_two_part_rust_version() {
        let rv = "v1.69".parse::<RustVersion>().unwrap();
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
    fn v_three_part_rust_version() {
        let rv = "v1.69.0".parse::<RustVersion>().unwrap();
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

    #[rstest]
    #[case("0.5.0", Bump::Major, "1.0.0")]
    #[case("0.5.0", Bump::Minor, "0.6.0")]
    #[case("0.5.0", Bump::Patch, "0.5.1")]
    #[case("1.2.3", Bump::Major, "2.0.0")]
    #[case("1.2.3", Bump::Minor, "1.3.0")]
    #[case("1.2.3", Bump::Patch, "1.2.4")]
    fn test_bump_version(#[case] v: Version, #[case] level: Bump, #[case] bumped: Version) {
        assert_eq!(bump_version(v, level), bumped);
    }

    #[test]
    fn test_copyright_line_one_year() {
        let s = "Copyright (c) 2023 John T. Wodder II";
        let crl = s.parse::<CopyrightLine>().unwrap();
        let mut years = RangeInclusiveSet::new();
        years.insert(2023..=2023);
        assert_eq!(
            crl,
            CopyrightLine {
                years,
                authors: "John T. Wodder II".into()
            }
        );
        assert_eq!(crl.to_string(), s);
    }

    #[test]
    fn test_copyright_line_two_years() {
        let s = "Copyright (c) 2023,2025 John T. Wodder II";
        let crl = s.parse::<CopyrightLine>().unwrap();
        let mut years = RangeInclusiveSet::new();
        years.insert(2023..=2023);
        years.insert(2025..=2025);
        assert_eq!(
            crl,
            CopyrightLine {
                years,
                authors: "John T. Wodder II".into()
            }
        );
        assert_eq!(
            crl.to_string(),
            "Copyright (c) 2023, 2025 John T. Wodder II"
        );
    }

    #[test]
    fn test_copyright_line_two_unmerged_years() {
        let s = "Copyright  (c)\t2023 , 2024  John T. Wodder II";
        let crl = s.parse::<CopyrightLine>().unwrap();
        let mut years = RangeInclusiveSet::new();
        years.insert(2023..=2024);
        assert_eq!(
            crl,
            CopyrightLine {
                years,
                authors: "John T. Wodder II".into()
            }
        );
        assert_eq!(crl.to_string(), "Copyright (c) 2023-2024 John T. Wodder II");
    }

    #[test]
    fn test_copyright_line_range() {
        let s = "Copyright (c) 2021 - 2023 John T. Wodder II";
        let crl = s.parse::<CopyrightLine>().unwrap();
        let mut years = RangeInclusiveSet::new();
        years.insert(2021..=2023);
        assert_eq!(
            crl,
            CopyrightLine {
                years,
                authors: "John T. Wodder II".into()
            }
        );
        assert_eq!(crl.to_string(), "Copyright (c) 2021-2023 John T. Wodder II");
    }

    #[test]
    fn test_copyright_line_range_year() {
        let s = "Copyright (c) 2021-2023, 2025 John T. Wodder II";
        let crl = s.parse::<CopyrightLine>().unwrap();
        let mut years = RangeInclusiveSet::new();
        years.insert(2021..=2023);
        years.insert(2025..=2025);
        assert_eq!(
            crl,
            CopyrightLine {
                years,
                authors: "John T. Wodder II".into()
            }
        );
        assert_eq!(crl.to_string(), s);
    }
}
