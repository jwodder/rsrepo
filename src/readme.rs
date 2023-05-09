#![allow(dead_code)]
use nom::bytes::complete::{is_not, tag, take_until};
use nom::character::complete::{char, line_ending};
use nom::combinator::{all_consuming, map_res, rest};
use nom::multi::{many1, separated_list1};
use nom::sequence::{delimited, terminated};
use nom::{Finish, IResult};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use thiserror::Error;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Readme {
    pub badges: Vec<Badge>,
    pub links: Vec<Link>,
    pub text: String,
}

impl Readme {
    fn repostatus(&self) -> Option<Repostatus> {
        for badge in &self.badges {
            if let Some(BadgeKind::Repostatus(rs)) = badge.kind() {
                return Some(rs);
            }
        }
        None
    }
}

impl FromStr for Readme {
    type Err = ParseReadmeError;

    fn from_str(s: &str) -> Result<Readme, ParseReadmeError> {
        match all_consuming(parse_readme)(s).finish() {
            Ok((_, readme)) => Ok(readme),
            // TODO: Include error details from nom error
            Err(_) => Err(ParseReadmeError),
        }
    }
}

#[derive(Copy, Clone, Debug, Error, Eq, PartialEq)]
#[error("invalid readme")]
pub struct ParseReadmeError;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Badge {
    pub url: String,
    pub alt: String,
    pub target: String,
}

impl Badge {
    pub fn kind(&self) -> Option<BadgeKind> {
        BadgeKind::for_url(&self.url)
    }
}

pub enum BadgeKind {
    Repostatus(Repostatus),
    GitHubActions,
    Codecov,
    License,
}

impl BadgeKind {
    fn for_url(s: &str) -> Option<BadgeKind> {
        let url = Url::parse(s).ok()?;
        match url.domain() {
            Some("www.repostatus.org") => Repostatus::for_url(s).map(BadgeKind::Repostatus),
            Some("github.com") => matches!(
                url.path_segments()?.collect::<Vec<_>>()[..],
                [_, _, "actions", "workflows", _, "badge.svg"]
            )
            .then_some(BadgeKind::GitHubActions),
            Some("codecov.io") => matches!(
                url.path_segments()?.collect::<Vec<_>>()[..],
                [_, _, _, "branch", _, "graph", "badge.svg"]
            )
            .then_some(BadgeKind::Codecov),
            Some("img.shields.io") => matches!(
                url.path_segments()?.collect::<Vec<_>>()[..],
                [_, "license", _, _]
            )
            .then_some(BadgeKind::License),
            _ => None,
        }
    }
}

pub enum Repostatus {
    Abandoned,
    Active,
    Concept,
    Inactive,
    Moved,
    Suspended,
    Unsupported,
    Wip,
}

impl Repostatus {
    pub fn for_url(url: &str) -> Option<Repostatus> {
        match all_consuming(repostatus_url)(url).finish() {
            Ok((_, repostatus)) => Some(repostatus),
            Err(_) => None,
        }
    }
}

impl FromStr for Repostatus {
    type Err = ParseRepostatusError;

    fn from_str(s: &str) -> Result<Repostatus, ParseRepostatusError> {
        use Repostatus::*;
        match s.to_ascii_lowercase().as_str() {
            "abandoned" => Ok(Abandoned),
            "active" => Ok(Active),
            "concept" => Ok(Concept),
            "inactive" => Ok(Inactive),
            "moved" => Ok(Moved),
            "suspended" => Ok(Suspended),
            "unsupported" => Ok(Unsupported),
            "wip" => Ok(Wip),
            _ => Err(ParseRepostatusError),
        }
    }
}

#[derive(Copy, Clone, Debug, Error, Eq, PartialEq)]
#[error("invalid repostatus status")]
pub struct ParseRepostatusError;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Link {
    pub url: String,
    pub text: String,
}

struct Image {
    url: String,
    alt: String,
}

fn parse_readme(input: &str) -> IResult<&str, Readme> {
    let (input, badges) = many1(terminated(badge, line_ending))(input)?;
    let (input, _) = line_ending(input)?;
    let (input, links) = separated_list1(tag(" | "), link)(input)?;
    let (input, _) = line_ending(input)?;
    let (input, _) = line_ending(input)?;
    let (input, text) = rest(input)?;
    Ok((
        input,
        Readme {
            badges,
            links,
            text: text.into(),
        },
    ))
}

fn badge(input: &str) -> IResult<&str, Badge> {
    let (input, image) = delimited(char('['), image, char(']'))(input)?;
    let (input, url) = delimited(char('('), many1(is_not(")")), char(')'))(input)?;
    Ok((
        input,
        Badge {
            url: image.url,
            alt: image.alt,
            target: url.into_iter().collect(),
        },
    ))
}

fn image(input: &str) -> IResult<&str, Image> {
    let (input, _) = char('!')(input)?;
    let (input, lnk) = link(input)?;
    Ok((
        input,
        Image {
            alt: lnk.text,
            url: lnk.url,
        },
    ))
}

fn link(input: &str) -> IResult<&str, Link> {
    let (input, text) = delimited(char('['), many1(is_not("]")), char(']'))(input)?;
    let (input, url) = delimited(char('('), many1(is_not(")")), char(')'))(input)?;
    Ok((
        input,
        Link {
            text: text.into_iter().collect(),
            url: url.into_iter().collect(),
        },
    ))
}

fn repostatus_url(input: &str) -> IResult<&str, Repostatus> {
    let (input, _) = tag("https://www.repostatus.org/badges/latest/")(input)?;
    map_res(take_until(".svg"), |s: &str| s.parse::<Repostatus>())(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readme01() {
        let src = include_str!("testdata/readme01.md");
        let jsonsrc = include_str!("testdata/readme01.json");
        let readme = src.parse::<Readme>().unwrap();
        let expected = serde_json::from_str::<Readme>(jsonsrc).unwrap();
        assert_eq!(readme, expected);
        // TODO: assert_eq!(readme.to_string(), src);
    }
}
