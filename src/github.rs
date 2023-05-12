#![allow(dead_code)]
use crate::http_util::StatusError;
use anyhow::Context;
use ghrepo::GHRepo;
use serde::{de::DeserializeOwned, Deserialize, Deserializer, Serialize, Serializer};
use std::borrow::Cow;
use std::fmt;
use ureq::{Agent, AgentBuilder};
use url::Url;

static USER_AGENT: &str = concat!(
    env!("CARGO_PKG_NAME"),
    "/",
    env!("CARGO_PKG_VERSION"),
    " (",
    env!("CARGO_PKG_REPOSITORY"),
    ")",
);

static API_ENDPOINT: &str = "https://api.github.com";

#[derive(Clone, Debug)]
pub struct GitHub {
    client: Agent,
}

impl GitHub {
    pub fn new<S: AsRef<str>>(token: Option<S>) -> GitHub {
        let authorization = token.map(|tk| format!("token {}", tk.as_ref()));
        let client = AgentBuilder::new()
            .user_agent(USER_AGENT)
            .https_only(true)
            .middleware(move |mut req: ureq::Request, next: ureq::MiddlewareNext| {
                if let Some(auth) = authorization.as_ref() {
                    req = req.set("Authorization", auth);
                }
                next.handle(req.set("Accept", "application/vnd.github+json"))
            })
            .build();
        GitHub { client }
    }

    pub fn authed() -> anyhow::Result<GitHub> {
        let token = gh_token::get().context("Failed to retrieve GitHub token")?;
        Ok(GitHub::new(Some(token)))
    }

    fn request<T: Serialize, U: DeserializeOwned>(
        &self,
        method: &str,
        path: &str,
        payload: Option<T>,
    ) -> anyhow::Result<U> {
        let url = mkurl(path)?;
        //log::debug!("{} {}", method, url);
        let req = self.client.request_url(method, &url);
        let r = if let Some(p) = payload {
            req.send_json(p)
        } else {
            req.call()
        };
        match r {
            Ok(r) => r
                .into_json::<U>()
                .with_context(|| format!("Failed to deserialize response from {path}")),
            Err(ureq::Error::Status(_, r)) => Err(StatusError::for_response(method, r).into()),
            Err(e) => Err(e).with_context(|| format!("Failed to make {method} request to {path}")),
        }
    }

    fn get<T: DeserializeOwned>(&self, path: &str) -> anyhow::Result<T> {
        self.request::<(), T>("GET", path, None)
    }

    fn post<T: Serialize, U: DeserializeOwned>(&self, path: &str, body: T) -> anyhow::Result<U> {
        self.request::<T, U>("POST", path, Some(body))
    }

    fn put<T: Serialize, U: DeserializeOwned>(&self, path: &str, body: T) -> anyhow::Result<U> {
        self.request::<T, U>("PUT", path, Some(body))
    }

    pub fn create_repository(&self, config: NewRepoConfig) -> anyhow::Result<Repository> {
        let (create_repo_body, set_topics_body) = config.into_payloads();
        let r: Repository = self.post("/user/repos", create_repo_body)?;
        if !set_topics_body.is_empty() {
            let _: SetTopicsBody = self.put(&format!("{}/topics", r.url), set_topics_body)?;
        }
        Ok(r)
    }

    pub fn create_label(&self, repo: &GHRepo, label: Label<'_>) -> anyhow::Result<()> {
        let _: Label<'_> = self.post(&format!("{}/labels", repo.api_url()), label)?;
        Ok(())
    }

    pub fn create_release(&self, repo: &GHRepo, release: CreateRelease) -> anyhow::Result<Release> {
        self.post(&format!("{}/releases", repo.api_url()), release)
    }

    pub fn latest_release(&self, repo: &GHRepo) -> anyhow::Result<Release> {
        self.get(&format!("{}/releases/latest", repo.api_url()))
    }

    pub fn set_topics<I>(&self, repo: &GHRepo, topics: I) -> anyhow::Result<()>
    where
        I: IntoIterator<Item = Topic>,
    {
        let body = SetTopicsBody {
            names: topics.into_iter().collect(),
        };
        let _: SetTopicsBody = self.put(&format!("{}/topics", repo.api_url()), body)?;
        Ok(())
    }
}

impl Default for GitHub {
    fn default() -> GitHub {
        GitHub::new::<&str>(None)
    }
}

fn mkurl(path: &str) -> anyhow::Result<Url> {
    Url::parse(API_ENDPOINT)
        .context("Failed to construct a Url for the GitHub API endpoint")?
        .join(path)
        .with_context(|| format!("Failed to construct a GitHub API URL with path {path:?}"))
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct CreateRepoBody {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    private: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    delete_branch_on_merge: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct Repository {
    pub id: u64,
    pub name: String,
    pub full_name: String,
    pub private: bool,
    pub html_url: String,
    pub description: String,
    pub url: String,
    pub ssh_url: String,
    pub topics: Vec<String>,
    // owner?
}

impl Repository {
    pub fn ghrepo(&self) -> anyhow::Result<GHRepo> {
        self.full_name.parse().with_context(|| {
            format!(
                "ghrepo failed to parse repository fullname {:?}",
                self.full_name
            )
        })
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct SetTopicsBody {
    names: Vec<Topic>,
}

impl SetTopicsBody {
    fn is_empty(&self) -> bool {
        self.names.is_empty()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewRepoConfig {
    name: String,
    description: Option<String>,
    private: Option<bool>,
    topics: Vec<Topic>,
}

impl NewRepoConfig {
    pub fn new<S: Into<String>>(name: S) -> Self {
        Self {
            name: name.into(),
            description: None,
            private: None,
            topics: Vec::new(),
        }
    }

    pub fn description<S: Into<String>>(mut self, description: S) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn private(mut self, private: bool) -> Self {
        self.private = Some(private);
        self
    }

    pub fn topics<I: IntoIterator<Item = Topic>>(mut self, iter: I) -> Self {
        self.topics = iter.into_iter().collect();
        self
    }

    fn into_payloads(self) -> (CreateRepoBody, SetTopicsBody) {
        (
            CreateRepoBody {
                name: self.name,
                description: self.description,
                private: self.private,
                delete_branch_on_merge: Some(true),
            },
            SetTopicsBody { names: self.topics },
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Topic(String);

impl Topic {
    pub fn new(s: &str) -> Topic {
        Topic(
            s.chars()
                .map(|ch| {
                    if ch.is_ascii_alphanumeric() {
                        ch.to_ascii_lowercase()
                    } else {
                        '-'
                    }
                })
                .take(50)
                .collect(),
        )
    }
}

impl<S: AsRef<str>> PartialEq<S> for Topic {
    fn eq(&self, other: &S) -> bool {
        self.0 == other.as_ref()
    }
}

impl fmt::Display for Topic {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Serialize for Topic {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Topic {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer).map(Topic)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Label<'a> {
    name: Cow<'a, str>,
    color: Cow<'a, str>,
    description: Cow<'a, str>,
}

impl<'a> Label<'a> {
    pub fn new(name: &'a str, color: &'a str, description: &'a str) -> Self {
        Label {
            name: name.into(),
            color: color.into(),
            description: description.into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CreateRelease {
    tag_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    body: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    prerelease: Option<bool>,
}

impl CreateRelease {
    pub fn new<S: Into<String>>(tag_name: S) -> CreateRelease {
        CreateRelease {
            tag_name: tag_name.into(),
            name: None,
            body: None,
            prerelease: None,
        }
    }

    pub fn name<S: Into<String>>(mut self, name: S) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn body<S: Into<String>>(mut self, body: S) -> Self {
        self.body = Some(body.into());
        self
    }

    pub fn prerelease(mut self, prerelease: bool) -> Self {
        self.prerelease = Some(prerelease);
        self
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct Release {
    pub url: String,
    pub html_url: String,
    pub assets_url: String,
    pub upload_url: String,
    pub tarball_url: String,
    pub zipball_url: String,
    pub id: u64,
    pub tag_name: String,
    pub target_commitish: String,
    pub name: String,
    #[serde(default)]
    pub body: Option<String>,
    pub draft: bool,
    pub prerelease: bool,
    //pub created_at: DateTime<FixedOffset>,
    //pub published_at: DateTime<FixedOffset>,
    //pub author: SimpleUser,
    //pub assets: Vec<ReleaseAsset>,
}

#[cfg(test)]
mod test {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case("work-in-progress", "work-in-progress")]
    #[case("Julian day", "julian-day")]
    fn new_topic(#[case] s: &str, #[case] tp: &str) {
        let topic = Topic::new(s);
        assert_eq!(topic, tp);
        if s != tp {
            assert_ne!(topic, s);
        }
    }
}
