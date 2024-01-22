use chrono::naive::NaiveDate;
use semver::Version;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use thiserror::Error;
use winnow::{
    ascii::{digit1, space1, Caseless},
    combinator::alt,
    stream::AsChar,
    token::take_till,
    PResult, Parser,
};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct Changelog {
    pub(crate) sections: Vec<ChangelogSection>,
}

impl FromStr for Changelog {
    type Err = ParseChangelogError;

    fn from_str(s: &str) -> Result<Changelog, ParseChangelogError> {
        let mut sections = Vec::new();
        let mut current: Option<SectionBuilder<'_>> = None;
        let mut prev: Option<&str> = None;
        for line in s.lines() {
            if line.chars().all(|ch| ch == '-') && line.len() >= 3 {
                if let Some(sb) = current.take() {
                    sections.push(sb.build());
                }
                if let Some(p) = prev.take() {
                    current = Some(SectionBuilder::<'_>::new(p.parse::<ChangelogHeader>()?));
                } else {
                    return Err(ParseChangelogError::UnexpectedHrule);
                }
            } else if let Some(p) = prev.replace(line) {
                if let Some(sb) = current.as_mut() {
                    sb.push_line(p);
                } else {
                    return Err(ParseChangelogError::TextBeforeHeader);
                }
            }
        }
        if let Some(p) = prev {
            if let Some(sb) = current.as_mut() {
                sb.push_line(p);
            } else {
                return Err(ParseChangelogError::TextBeforeHeader);
            }
        }
        if let Some(sb) = current.take() {
            sections.push(sb.build());
        }
        Ok(Changelog { sections })
    }
}

impl fmt::Display for Changelog {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sections = self
            .sections
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        let sep = if sections.iter().any(|sect| sect.contains("\n\n")) {
            "\n\n"
        } else {
            "\n"
        };
        write!(f, "{}", sections.join(sep))?;
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct ChangelogSection {
    pub(crate) header: ChangelogHeader,
    pub(crate) content: String,
}

impl fmt::Display for ChangelogSection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let header = self.header.to_string();
        writeln!(f, "{header}")?;
        writeln!(f, "{}", "-".repeat(header.len()))?;
        write!(f, "{}", self.content)?;
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case", tag = "type")]
pub(crate) enum ChangelogHeader {
    Released { version: Version, date: NaiveDate },
    InProgress { version: Version },
    InDevelopment,
}

impl FromStr for ChangelogHeader {
    type Err = ParseHeaderError;

    fn from_str(s: &str) -> Result<ChangelogHeader, ParseHeaderError> {
        // TODO: Include error details from winnow error
        parse_header.parse(s).map_err(|_| ParseHeaderError)
    }
}

impl fmt::Display for ChangelogHeader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChangelogHeader::Released { version, date } => write!(f, "v{version} ({date})"),
            ChangelogHeader::InProgress { version } => write!(f, "v{version} (in development)"),
            ChangelogHeader::InDevelopment => write!(f, "In Development"),
        }
    }
}

#[derive(Copy, Clone, Debug, Error, Eq, PartialEq)]
pub(crate) enum ParseChangelogError {
    #[error("unexpected hrule")]
    UnexpectedHrule,
    #[error("text before first header")]
    TextBeforeHeader,
    #[error("invalid header title")]
    InvalidHeader(#[from] ParseHeaderError),
}

#[derive(Copy, Clone, Debug, Error, Eq, PartialEq)]
#[error("invalid changelog header title")]
pub(crate) struct ParseHeaderError;

#[derive(Clone, Debug, Eq, PartialEq)]
struct SectionBuilder<'a> {
    header: ChangelogHeader,
    lines: Vec<&'a str>,
}

impl<'a> SectionBuilder<'a> {
    fn new(header: ChangelogHeader) -> Self {
        SectionBuilder {
            header,
            lines: Vec::new(),
        }
    }

    fn push_line(&mut self, line: &'a str) {
        self.lines.push(line);
    }

    fn build(mut self) -> ChangelogSection {
        while let Some(line) = self.lines.last() {
            if line.is_empty() {
                self.lines.pop();
            } else {
                break;
            }
        }
        let mut content = String::new();
        for line in self.lines {
            content.push_str(line);
            content.push('\n');
        }
        ChangelogSection {
            header: self.header,
            content,
        }
    }
}

fn parse_header(input: &mut &str) -> PResult<ChangelogHeader> {
    alt((versioned_header, in_development)).parse_next(input)
}

fn versioned_header(input: &mut &str) -> PResult<ChangelogHeader> {
    let (_, version, _, _, parenthed, _) = (
        'v',
        take_till(1.., AsChar::is_space).try_map(|s: &str| s.parse::<Version>()),
        space1,
        '(',
        alt((ymd.map(Some), Caseless("in development").map(|_| None))),
        ')',
    )
        .parse_next(input)?;
    if let Some(date) = parenthed {
        Ok(ChangelogHeader::Released { version, date })
    } else {
        Ok(ChangelogHeader::InProgress { version })
    }
}

fn ymd(input: &mut &str) -> PResult<NaiveDate> {
    // TODO: Make this take exactly 4-2-2 digits
    (digit1, '-', digit1, '-', digit1)
        .recognize()
        .try_map(|s: &str| s.parse::<NaiveDate>())
        .parse_next(input)
}

fn in_development(input: &mut &str) -> PResult<ChangelogHeader> {
    Caseless("in development")
        .map(|_| ChangelogHeader::InDevelopment)
        .parse_next(input)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fs_err::{read_dir, read_to_string};
    use std::ffi::OsStr;
    use std::path::Path;

    #[test]
    fn test_changelog() {
        let diriter = read_dir(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/testdata/changelog"
        ))
        .unwrap();
        for entry in diriter {
            let entry = entry.unwrap();
            let fname = entry.file_name();
            let fname = Path::new(&fname);
            if fname.extension() == Some(OsStr::new("md")) {
                eprintln!("Testing: {}", fname.display());
                let path = entry.path();
                let src = read_to_string(&path).unwrap();
                let jsonsrc = read_to_string(path.with_extension("json")).unwrap();
                let changelog = src.parse::<Changelog>().unwrap();
                let expected = serde_json::from_str::<Changelog>(&jsonsrc).unwrap();
                assert_eq!(changelog, expected);
                assert_eq!(changelog.to_string(), src);
            }
        }
    }
}
