use chrono::naive::NaiveDate;
use nom::branch::alt;
use nom::bytes::complete::{tag_no_case, take_till1};
use nom::character::complete::{char, digit1, space1};
use nom::combinator::{all_consuming, map_res, recognize};
use nom::sequence::{delimited, tuple};
use nom::{Finish, IResult, Parser};
use semver::Version;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use thiserror::Error;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Changelog {
    sections: Vec<ChangelogSection>,
}

impl FromStr for Changelog {
    type Err = ParseChangelogError;

    fn from_str(s: &str) -> Result<Changelog, ParseChangelogError> {
        let mut sections = Vec::new();
        let mut prev: Option<&str> = None;
        for line in s.lines() {
            if line.chars().all(|ch| ch == '-') && line.len() >= 3 {
                if let Some(p) = prev.take() {
                    sections.push(ChangelogSection {
                        header: p.parse::<ChangelogHeader>()?,
                        content: String::new(),
                    });
                } else {
                    return Err(ParseChangelogError::UnexpectedHrule);
                }
            } else if let Some(p) = prev.replace(line) {
                if let Some(sect) = sections.last_mut() {
                    sect.content.push_str(p);
                    sect.content.push('\n');
                } else {
                    return Err(ParseChangelogError::TextBeforeHeader);
                }
            }
        }
        if let Some(p) = prev {
            if let Some(sect) = sections.last_mut() {
                sect.content.push_str(p);
                sect.content.push('\n');
            } else {
                return Err(ParseChangelogError::TextBeforeHeader);
            }
        }
        Ok(Changelog { sections })
    }
}

impl fmt::Display for Changelog {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let sections = self
            .sections
            .iter()
            .map(|sect| sect.to_string())
            .collect::<Vec<_>>();
        let sep = if sections.iter().any(|sect| sect.contains("\n\n")) {
            "\n\n\n"
        } else {
            "\n\n"
        };
        write!(f, "{}", sections.join(sep))?;
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ChangelogSection {
    header: ChangelogHeader,
    content: String,
}

impl fmt::Display for ChangelogSection {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let header = self.header.to_string();
        writeln!(f, "{header}")?;
        writeln!(f, "{}", "-".repeat(header.len()))?;
        write!(f, "{}", self.content)?;
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case", tag = "type")]
pub enum ChangelogHeader {
    Released { version: Version, date: NaiveDate },
    InProgress { version: Version },
    InDevelopment,
}

impl FromStr for ChangelogHeader {
    type Err = ParseHeaderError;

    fn from_str(s: &str) -> Result<ChangelogHeader, ParseHeaderError> {
        match all_consuming(parse_header)(s).finish() {
            Ok((_, header)) => Ok(header),
            // TODO: Include error details from nom error
            Err(_) => Err(ParseHeaderError),
        }
    }
}

impl fmt::Display for ChangelogHeader {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ChangelogHeader::Released { version, date } => write!(f, "v{version} ({date})"),
            ChangelogHeader::InProgress { version } => write!(f, "v{version} (in development)"),
            ChangelogHeader::InDevelopment => write!(f, "In Development"),
        }
    }
}

#[derive(Copy, Clone, Debug, Error, Eq, PartialEq)]
pub enum ParseChangelogError {
    #[error("unexpected hrule")]
    UnexpectedHrule,
    #[error("text before first header")]
    TextBeforeHeader,
    #[error("invalid header title")]
    InvalidHeader(#[from] ParseHeaderError),
}

#[derive(Copy, Clone, Debug, Error, Eq, PartialEq)]
#[error("invalid changelog header title")]
pub struct ParseHeaderError;

fn parse_header(input: &str) -> IResult<&str, ChangelogHeader> {
    alt((versioned_header, in_development))(input)
}

fn versioned_header(input: &str) -> IResult<&str, ChangelogHeader> {
    let (input, _) = char('v')(input)?;
    let (input, version) = map_res(
        take_till1(|ch: char| ch.is_ascii_whitespace()),
        |s: &str| s.parse::<Version>(),
    )(input)?;
    let (input, _) = space1(input)?;
    let (input, parenthed) = delimited(
        char('('),
        alt((ymd.map(Some), tag_no_case("in development").map(|_| None))),
        char(')'),
    )(input)?;
    let header = if let Some(date) = parenthed {
        ChangelogHeader::Released { version, date }
    } else {
        ChangelogHeader::InProgress { version }
    };
    Ok((input, header))
}

fn ymd(input: &str) -> IResult<&str, NaiveDate> {
    // TODO: Make this take exactly 4-2-2 digits
    map_res(
        recognize(tuple((digit1, char('-'), digit1, char('-'), digit1))),
        |s: &str| s.parse::<NaiveDate>(),
    )(input)
}

fn in_development(input: &str) -> IResult<&str, ChangelogHeader> {
    tag_no_case("in development")
        .map(|_| ChangelogHeader::InDevelopment)
        .parse(input)
}
