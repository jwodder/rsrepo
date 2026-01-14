use cargo_metadata::semver::Version;
use chrono::Datelike;
use fs_err::{create_dir_all, read_dir, remove_dir};
use rangemap::RangeInclusiveSet;
use renamore::rename_exclusive;
use serde::de::{Deserializer, Unexpected, Visitor};
use serde::ser::Serializer;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fmt;
use std::fs::FileType;
use std::iter::FusedIterator;
use std::ops::RangeInclusive;
use std::path::{Path, PathBuf, StripPrefixError};
use std::str::FromStr;
use thiserror::Error;
use winnow::{
    Parser,
    ascii::{dec_uint, digit1, space0, space1},
    combinator::{opt, preceded, separated},
    error::ModalResult,
    seq,
    token::rest,
};

pub(crate) fn this_year() -> i32 {
    chrono::Local::now().year()
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct StringLines {
    content: String,
    offset: usize,
}

impl StringLines {
    pub(crate) fn new(content: String) -> StringLines {
        StringLines { content, offset: 0 }
    }
}

impl Iterator for StringLines {
    type Item = String;

    fn next(&mut self) -> Option<String> {
        if self.offset == self.content.len() {
            return None;
        }
        let next_offset = self.content[self.offset..]
            .find('\n')
            .map_or(self.content.len(), |i| self.offset + i + 1);
        let mut end = next_offset;
        if end > 0 && self.content.as_bytes()[end - 1] == b'\n' {
            end -= 1;
        }
        if end > 0 && self.content.as_bytes()[end - 1] == b'\r' {
            end -= 1;
        }
        let line = &self.content[self.offset..end];
        self.offset = next_offset;
        Some(line.to_owned())
    }
}

impl FusedIterator for StringLines {}

#[derive(Copy, Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct RustVersion {
    major: u32,
    minor: u32,
    patch: Option<u32>,
}

impl RustVersion {
    pub(crate) fn without_patch(mut self) -> RustVersion {
        self.patch = None;
        self
    }
}

impl FromStr for RustVersion {
    type Err = ParseRustVersionError;

    fn from_str(s: &str) -> Result<RustVersion, ParseRustVersionError> {
        rust_version.parse(s).map_err(|_| ParseRustVersionError)
    }
}

impl fmt::Display for RustVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)?;
        if let Some(patch) = self.patch {
            write!(f, ".{patch}")?;
        }
        Ok(())
    }
}

impl Serialize for RustVersion {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
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

impl Visitor<'_> for RustVersionVisitor {
    type Value = RustVersion;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
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
pub(crate) struct ParseRustVersionError;

fn rust_version(input: &mut &str) -> ModalResult<RustVersion> {
    seq! {
        RustVersion {
            _: opt('v'),
            major: dec_uint,
            _: '.',
            minor: dec_uint,
            patch: opt(preceded('.', dec_uint)),
        }
    }
    .parse_next(input)
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) enum Bump {
    Major,
    Minor,
    Patch,
}

pub(crate) fn bump_version(v: Version, level: Bump) -> Version {
    match level {
        Bump::Major => Version::new(v.major + 1, 0, 0),
        Bump::Minor => Version::new(v.major, v.minor + 1, 0),
        Bump::Patch => Version::new(v.major, v.minor, v.patch + 1),
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CopyrightLine {
    prefix: String,
    years: RangeInclusiveSet<i32>,
    authors: String,
}

impl CopyrightLine {
    pub(crate) fn add_year(&mut self, year: i32) {
        self.years.insert(year..=year);
    }
}

impl FromStr for CopyrightLine {
    type Err = ParseCopyrightError;

    fn from_str(s: &str) -> Result<CopyrightLine, ParseCopyrightError> {
        copyright.parse(s).map_err(|_| ParseCopyrightError)
    }
}

impl fmt::Display for CopyrightLine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.prefix)?;
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
pub(crate) struct ParseCopyrightError;

fn copyright(input: &mut &str) -> ModalResult<CopyrightLine> {
    seq! {
        CopyrightLine {
            prefix: (space0, "Copyright", space1, opt(("(c)", space1))).take().map(String::from),
            years: separated(1.., year_range, (space0, ',', space0)).map(|ranges: Vec<RangeInclusive<i32>>| ranges.into_iter().collect()),
            _: space1,
            authors: rest.map(String::from),
        }
    }.parse_next(input)
}

fn year_range(input: &mut &str) -> ModalResult<RangeInclusive<i32>> {
    let (start, end) = (
        digit1.parse_to(),
        opt(preceded((space0, '-', space0), digit1.parse_to())),
    )
        .parse_next(input)?;
    Ok(start..=end.unwrap_or(start))
}

pub(crate) fn move_dirtree_into(src: &Path, dest: &Path) -> Result<(), MoveDirtreeIntoError> {
    use MoveDirtreeIntoError::*;
    let mut stack = vec![DirWithEntries::new(src)?];
    while let Some(entries) = stack.last_mut() {
        if let Some((entry, ftype)) = entries.pop_front() {
            let relpath = match entry.strip_prefix(src) {
                Ok(relpath) => relpath,
                Err(source) => {
                    return Err(Relpath {
                        source,
                        path: entry,
                        base: src.into(),
                    });
                }
            };
            let target = dest.join(relpath);
            if ftype.is_dir() {
                create_dir_all(target)?;
                stack.push(DirWithEntries::new(&entry)?);
            } else if let Err(source) = rename_exclusive(&entry, &target) {
                return Err(Rename {
                    source,
                    src: entry,
                    dest: target,
                });
            }
        } else {
            remove_dir(&entries.dirpath)?;
            stack.pop();
        }
    }
    Ok(())
}

#[derive(Debug, Error)]
pub(crate) enum MoveDirtreeIntoError {
    #[error(transparent)]
    IO(#[from] std::io::Error),
    #[error("path {} beneath {} was not relative to it", .path.display(), .base.display())]
    Relpath {
        source: StripPrefixError,
        path: PathBuf,
        base: PathBuf,
    },
    #[error("could not rename path {} to {}", .src.display(), .dest.display())]
    Rename {
        source: std::io::Error,
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
        let mut entries = VecDeque::new();
        for entry in read_dir(dirpath)? {
            let entry = entry?;
            let ftype = entry.file_type()?;
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

pub(crate) fn workspace_tag_prefix(package_name: &str) -> String {
    format!("{package_name}/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_fs::TempDir;
    use assert_fs::prelude::*;
    use predicates::prelude::*;
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

    #[test]
    fn three_part_rust_version_without_patch() {
        let rv = RustVersion {
            major: 1,
            minor: 69,
            patch: Some(0),
        };
        let rv = rv.without_patch();
        assert_eq!(
            rv,
            RustVersion {
                major: 1,
                minor: 69,
                patch: None,
            }
        );
    }

    #[test]
    fn two_part_rust_version_without_patch() {
        let rv = RustVersion {
            major: 1,
            minor: 69,
            patch: None,
        };
        assert_eq!(rv, rv.without_patch());
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
                prefix: "Copyright (c) ".into(),
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
                prefix: "Copyright (c) ".into(),
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
                prefix: "Copyright  (c)\t".into(),
                years,
                authors: "John T. Wodder II".into()
            }
        );
        assert_eq!(
            crl.to_string(),
            "Copyright  (c)\t2023-2024 John T. Wodder II"
        );
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
                prefix: "Copyright (c) ".into(),
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
                prefix: "Copyright (c) ".into(),
                years,
                authors: "John T. Wodder II".into()
            }
        );
        assert_eq!(crl.to_string(), s);
    }

    #[test]
    fn test_copyright_line_variant_prefix() {
        let s = "\tCopyright  2023 John T. Wodder II";
        let crl = s.parse::<CopyrightLine>().unwrap();
        let mut years = RangeInclusiveSet::new();
        years.insert(2023..=2023);
        assert_eq!(
            crl,
            CopyrightLine {
                prefix: "\tCopyright  ".into(),
                years,
                authors: "John T. Wodder II".into()
            }
        );
        assert_eq!(crl.to_string(), s);
    }

    #[test]
    fn test_move_dirtree_into() {
        let src = TempDir::new().unwrap();
        src.child("orange.txt").write_str("Orange\n").unwrap();
        src.child("foo").create_dir_all().unwrap();
        src.child("foo")
            .child("apple.txt")
            .write_str("Apple\n")
            .unwrap();
        src.child("foo").child("bar").create_dir_all().unwrap();
        src.child("foo")
            .child("bar")
            .child("banana.txt")
            .write_str("Banana\n")
            .unwrap();
        src.child("foo")
            .child("bar")
            .child("coconut.txt")
            .write_str("Coconut\n")
            .unwrap();
        src.child("foo").child("empty").create_dir_all().unwrap();
        src.child("foo")
            .child("quux")
            .child("glarch")
            .create_dir_all()
            .unwrap();
        src.child("foo")
            .child("quux")
            .child("glarch")
            .child("lemon.txt")
            .write_str("Lemon\n")
            .unwrap();
        src.child("gnusto").create_dir_all().unwrap();
        src.child("gnusto")
            .child("pear.txt")
            .write_str("Pear\n")
            .unwrap();
        let dest = TempDir::new().unwrap();
        dest.child("foo").create_dir_all().unwrap();
        dest.child("foo")
            .child("pomegranate.txt")
            .write_str("Pomegranate\n")
            .unwrap();
        dest.child("cleesh").create_dir_all().unwrap();
        dest.child("cleesh")
            .child("mango.txt")
            .write_str("Mango.txt\n")
            .unwrap();
        move_dirtree_into(&src, &dest).unwrap();
        dest.child("orange.txt").assert("Orange\n");
        dest.child("foo").child("apple.txt").assert("Apple\n");
        dest.child("foo")
            .child("bar")
            .child("banana.txt")
            .assert("Banana\n");
        dest.child("foo")
            .child("bar")
            .child("coconut.txt")
            .assert("Coconut\n");
        dest.child("foo")
            .child("pomegranate.txt")
            .assert("Pomegranate\n");
        dest.child("foo")
            .child("empty")
            .assert(predicate::path::is_dir());
        dest.child("foo")
            .child("quux")
            .child("glarch")
            .child("lemon.txt")
            .assert("Lemon\n");
        dest.child("gnusto").child("pear.txt").assert("Pear\n");
        dest.child("cleesh")
            .child("mango.txt")
            .assert("Mango.txt\n");
    }
}
