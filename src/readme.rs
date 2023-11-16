use crate::util::RustVersion;
use ghrepo::GHRepo;
use nom::bytes::complete::{is_not, tag};
use nom::character::complete::{alpha1, char, line_ending, space1};
use nom::combinator::{all_consuming, map_res, rest};
use nom::multi::{many1, separated_list1};
use nom::sequence::{delimited, terminated, tuple};
use nom::{Finish, IResult};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use thiserror::Error;
use url::Url;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Readme {
    pub badges: Vec<Badge>,
    pub links: Vec<Link>,
    pub text: String,
}

impl Readme {
    pub fn repostatus(&self) -> Option<Repostatus> {
        for badge in &self.badges {
            if let Some(BadgeKind::Repostatus(rs)) = badge.kind() {
                return Some(rs);
            }
        }
        None
    }

    pub fn set_repostatus_badge(&mut self, badge: Badge) {
        match self
            .badges
            .iter()
            .position(|badge| matches!(badge.kind(), Some(BadgeKind::Repostatus(_))))
        {
            Some(i) => self.badges[i] = badge,
            None => self.badges.insert(0, badge),
        }
    }

    pub fn set_msrv(&mut self, msrv: RustVersion) {
        let url = format!("https://img.shields.io/badge/MSRV-{msrv}-orange");
        if let Some(i) = self
            .badges
            .iter()
            .position(|badge| badge.kind() == Some(BadgeKind::Msrv))
        {
            self.badges[i].url = url;
        } else {
            let pos = self
                .badges
                .iter()
                .position(|badge| badge.kind() == Some(BadgeKind::License))
                .unwrap_or(self.badges.len());
            self.badges.insert(
                pos,
                Badge {
                    url,
                    alt: "Minimum Supported Rust Version".into(),
                    target: "https://www.rust-lang.org".into(),
                },
            );
        }
    }

    // Returns `true` if changed
    pub fn ensure_crates_links(&mut self, package: &str, docs: bool) -> bool {
        let mut changed = false;
        let github_index = self
            .links
            .iter()
            .position(|lnk| lnk.text == "GitHub")
            .unwrap_or(0);
        let crates_index =
            if let Some(i) = self.links.iter().position(|lnk| lnk.text == "crates.io") {
                i
            } else {
                self.links.insert(
                    github_index + 1,
                    Link {
                        url: format!("https://crates.io/crates/{package}"),
                        text: "crates.io".into(),
                    },
                );
                changed = true;
                github_index + 1
            };
        if docs && !self.links.iter().any(|lnk| lnk.text == "Documentation") {
            self.links.insert(
                crates_index + 1,
                Link {
                    url: format!("https://docs.rs/{package}"),
                    text: "Documentation".into(),
                },
            );
            changed = true;
        }
        changed
    }

    // Returns `true` if changed
    pub fn ensure_changelog_link(&mut self, repo: &GHRepo) -> bool {
        if self.links.iter().any(|lnk| lnk.text == "Changelog") {
            false
        } else {
            self.links.push(Link {
                url: format!("https://github.com/{repo}/blob/master/CHANGELOG.md"),
                text: "Changelog".into(),
            });
            true
        }
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

impl fmt::Display for Readme {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for badge in &self.badges {
            writeln!(f, "{badge}")?;
        }
        writeln!(f)?;
        if !self.links.is_empty() {
            let mut first = true;
            for lnk in &self.links {
                if !std::mem::replace(&mut first, false) {
                    write!(f, " | ")?;
                }
                write!(f, "{lnk}")?;
            }
            writeln!(f)?;
            writeln!(f)?;
        }
        write!(f, "{}", self.text)?;
        Ok(())
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

impl fmt::Display for Badge {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "[![{}]({})]({})", self.alt, self.url, self.target)
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum BadgeKind {
    Repostatus(Repostatus),
    GitHubActions,
    Codecov,
    Msrv,
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
            Some("img.shields.io") => {
                if url.path().starts_with("/badge/MSRV-") {
                    Some(BadgeKind::Msrv)
                } else if matches!(
                    url.path_segments()?.collect::<Vec<_>>()[..],
                    [_, "license", _, _]
                ) {
                    Some(BadgeKind::License)
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
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

impl fmt::Display for Link {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "[{}]({})", self.text, self.url)
    }
}

struct Image {
    url: String,
    alt: String,
}

fn parse_readme(input: &str) -> IResult<&str, Readme> {
    let (input, badges) = many1(terminated(badge, line_ending))(input)?;
    let (input, _) = line_ending(input)?;
    let (input, links) = separated_list1(tuple((space1, char('|'), space1)), link)(input)?;
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
    delimited(
        tag("https://www.repostatus.org/badges/latest/"),
        map_res(alpha1, |s: &str| s.parse::<Repostatus>()),
        tag(".svg"),
    )(input)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[test]
    fn new_readme() {
        let src = include_str!("testdata/readme/new.md");
        let jsonsrc = include_str!("testdata/readme/new.json");
        let readme = src.parse::<Readme>().unwrap();
        let expected = serde_json::from_str::<Readme>(jsonsrc).unwrap();
        assert_eq!(readme, expected);
        assert_eq!(readme.to_string(), src);
        assert_eq!(readme.repostatus(), Some(Repostatus::Wip));
        let mut iter = readme.badges.into_iter();
        assert_eq!(
            iter.next().and_then(|b| b.kind()),
            Some(BadgeKind::Repostatus(Repostatus::Wip))
        );
        assert_eq!(
            iter.next().and_then(|b| b.kind()),
            Some(BadgeKind::GitHubActions)
        );
        assert_eq!(iter.next().and_then(|b| b.kind()), Some(BadgeKind::Codecov));
        assert_eq!(iter.next().and_then(|b| b.kind()), Some(BadgeKind::Msrv));
        assert_eq!(iter.next().and_then(|b| b.kind()), Some(BadgeKind::License));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn with_crates_readme() {
        let src = include_str!("testdata/readme/with-crates.md");
        let jsonsrc = include_str!("testdata/readme/with-crates.json");
        let readme = src.parse::<Readme>().unwrap();
        let expected = serde_json::from_str::<Readme>(jsonsrc).unwrap();
        assert_eq!(readme, expected);
        assert_eq!(readme.to_string(), src);
        assert_eq!(readme.repostatus(), Some(Repostatus::Wip));
    }

    #[test]
    fn active_readme() {
        let src = include_str!("testdata/readme/active.md");
        let jsonsrc = include_str!("testdata/readme/active.json");
        let readme = src.parse::<Readme>().unwrap();
        let expected = serde_json::from_str::<Readme>(jsonsrc).unwrap();
        assert_eq!(readme, expected);
        assert_eq!(readme.to_string(), src);
        assert_eq!(readme.repostatus(), Some(Repostatus::Active));
    }

    #[test]
    fn set_repostatus_badge() {
        let mut readme = include_str!("testdata/readme/new.md")
            .parse::<Readme>()
            .unwrap();
        let expected = include_str!("testdata/readme/active.md");
        readme.set_repostatus_badge(Badge {
            alt: "Project Status: Active – The project has reached a stable, usable state and is being actively developed.".into(),
            url: "https://www.repostatus.org/badges/latest/active.svg".into(),
            target: "https://www.repostatus.org/#active".into(),
        });
        assert_eq!(readme.to_string(), expected);
    }

    #[test]
    fn ensure_crates_links() {
        let mut readme = include_str!("testdata/readme/new.md")
            .parse::<Readme>()
            .unwrap();
        let expected = include_str!("testdata/readme/with-crates.md");
        assert!(readme.ensure_crates_links("foobar", true));
        assert_eq!(readme.to_string(), expected);
        assert!(!readme.ensure_crates_links("foobar", true));
        assert_eq!(readme.to_string(), expected);
    }

    #[rstest]
    #[case(
        "https://www.repostatus.org/badges/latest/wip.svg",
        Some(BadgeKind::Repostatus(Repostatus::Wip))
    )]
    #[case(
        "https://github.com/rs.test/foobar/actions/workflows/test.yml/badge.svg",
        Some(BadgeKind::GitHubActions)
    )]
    #[case(
        "https://codecov.io/gh/rs.test/foobar/branch/master/graph/badge.svg",
        Some(BadgeKind::Codecov)
    )]
    #[case("https://img.shields.io/badge/MSRV-1.69-orange", Some(BadgeKind::Msrv))]
    #[case(
        "https://img.shields.io/github/license/rs.test/foobar.svg",
        Some(BadgeKind::License)
    )]
    #[case("https://docs.rs/rs.test/badge.svg", None)]
    fn badge_kind_for_url(#[case] url: &str, #[case] kind: Option<BadgeKind>) {
        assert_eq!(BadgeKind::for_url(url), kind);
    }

    #[rstest]
    #[case(
        "https://www.repostatus.org/badges/latest/abandoned.svg",
        Some(Repostatus::Abandoned)
    )]
    #[case(
        "https://www.repostatus.org/badges/latest/active.svg",
        Some(Repostatus::Active)
    )]
    #[case(
        "https://www.repostatus.org/badges/latest/concept.svg",
        Some(Repostatus::Concept)
    )]
    #[case(
        "https://www.repostatus.org/badges/latest/inactive.svg",
        Some(Repostatus::Inactive)
    )]
    #[case(
        "https://www.repostatus.org/badges/latest/moved.svg",
        Some(Repostatus::Moved)
    )]
    #[case(
        "https://www.repostatus.org/badges/latest/suspended.svg",
        Some(Repostatus::Suspended)
    )]
    #[case(
        "https://www.repostatus.org/badges/latest/unsupported.svg",
        Some(Repostatus::Unsupported)
    )]
    #[case(
        "https://www.repostatus.org/badges/latest/wip.svg",
        Some(Repostatus::Wip)
    )]
    #[case("https://img.shields.io/badge/MSRV-1.69-orange", None)]
    fn repostatus_for_url(#[case] url: &str, #[case] status: Option<Repostatus>) {
        assert_eq!(Repostatus::for_url(url), status);
    }
}
