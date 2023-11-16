use crate::http_util::StatusError;
use anyhow::Context;
use ghrepo::GHRepo;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
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
pub(crate) struct GitHub {
    client: Agent,
}

impl GitHub {
    pub(crate) fn new(token: &str) -> GitHub {
        let auth = format!("Bearer {token}");
        let client = AgentBuilder::new()
            .user_agent(USER_AGENT)
            .https_only(true)
            .middleware(move |req: ureq::Request, next: ureq::MiddlewareNext<'_>| {
                next.handle(
                    req.set("Authorization", &auth)
                        .set("Accept", "application/vnd.github+json"),
                )
            })
            .build();
        GitHub { client }
    }

    pub(crate) fn authed() -> anyhow::Result<GitHub> {
        let token = gh_token::get().context("Failed to retrieve GitHub token")?;
        Ok(GitHub::new(&token))
    }

    fn request<T: Serialize, U: DeserializeOwned>(
        &self,
        method: &str,
        path: &str,
        payload: Option<T>,
    ) -> anyhow::Result<U> {
        let url = mkurl(path)?;
        log::debug!("{} {}", method, url);
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

    pub(crate) fn whoami(&self) -> anyhow::Result<String> {
        Ok(self
            .get::<User>("/user")
            .context("failed to fetch authenticated GitHub user's login name")?
            .login)
    }

    pub(crate) fn create_repository(&self, config: CreateRepoBody) -> anyhow::Result<Repository> {
        self.post("/user/repos", config)
    }

    pub(crate) fn create_label<R>(&self, repo: &R, label: Label<'_>) -> anyhow::Result<()>
    where
        for<'a> R: RepositoryEndpoint<'a>,
    {
        let _: Label<'_> = self.post(&format!("{}/labels", repo.api_url()), label)?;
        Ok(())
    }

    pub(crate) fn create_release<R>(
        &self,
        repo: &R,
        release: CreateRelease,
    ) -> anyhow::Result<Release>
    where
        for<'a> R: RepositoryEndpoint<'a>,
    {
        self.post(&format!("{}/releases", repo.api_url()), release)
    }

    pub(crate) fn latest_release<R>(&self, repo: &R) -> anyhow::Result<Release>
    where
        for<'a> R: RepositoryEndpoint<'a>,
    {
        self.get(&format!("{}/releases/latest", repo.api_url()))
    }

    pub(crate) fn get_topics<R>(&self, repo: &R) -> anyhow::Result<Vec<Topic>>
    where
        for<'a> R: RepositoryEndpoint<'a>,
    {
        let payload = self.get::<TopicsPayload>(&format!("{}/topics", repo.api_url()))?;
        Ok(payload.names)
    }

    pub(crate) fn set_topics<I, R>(&self, repo: &R, topics: I) -> anyhow::Result<()>
    where
        I: IntoIterator<Item = Topic>,
        for<'a> R: RepositoryEndpoint<'a>,
    {
        let body = TopicsPayload {
            names: topics.into_iter().collect(),
        };
        let _: TopicsPayload = self.put(&format!("{}/topics", repo.api_url()), body)?;
        Ok(())
    }
}

fn mkurl(path: &str) -> anyhow::Result<Url> {
    Url::parse(API_ENDPOINT)
        .context("Failed to construct a Url for the GitHub API endpoint")?
        .join(path)
        .with_context(|| format!("Failed to construct a GitHub API URL with path {path:?}"))
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
struct User {
    login: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct CreateRepoBody {
    pub(crate) name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) private: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) delete_branch_on_merge: Option<bool>,
}

pub(crate) trait RepositoryEndpoint<'a> {
    type Url: fmt::Display;

    fn api_url(&'a self) -> Self::Url;
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub(crate) struct Repository {
    pub(crate) id: u64,
    pub(crate) name: String,
    pub(crate) full_name: String,
    pub(crate) private: bool,
    pub(crate) html_url: String,
    pub(crate) description: String,
    pub(crate) url: String,
    pub(crate) ssh_url: String,
    pub(crate) topics: Vec<String>,
    // owner?
}

impl<'a> RepositoryEndpoint<'a> for Repository {
    type Url = &'a str;

    fn api_url(&'a self) -> &'a str {
        &self.url
    }
}

impl<'a> RepositoryEndpoint<'a> for GHRepo {
    type Url = String;

    fn api_url(&'a self) -> String {
        self.api_url()
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct TopicsPayload {
    names: Vec<Topic>,
}

#[derive(Clone, Debug, Deserialize, Hash, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub(crate) struct Topic(String);

impl Topic {
    pub(crate) fn new(s: &str) -> Topic {
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
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct Label<'a> {
    name: Cow<'a, str>,
    color: Cow<'a, str>,
    description: Cow<'a, str>,
}

impl<'a> Label<'a> {
    pub(crate) fn new(name: &'a str, color: &'a str, description: &'a str) -> Self {
        Label {
            name: name.into(),
            color: color.into(),
            description: description.into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct CreateRelease {
    tag_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    body: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    prerelease: Option<bool>,
}

impl CreateRelease {
    pub(crate) fn new<S: Into<String>>(tag_name: S) -> CreateRelease {
        CreateRelease {
            tag_name: tag_name.into(),
            name: None,
            body: None,
            prerelease: None,
        }
    }

    pub(crate) fn name<S: Into<String>>(mut self, name: S) -> Self {
        self.name = Some(name.into());
        self
    }

    pub(crate) fn body<S: Into<String>>(mut self, body: S) -> Self {
        self.body = Some(body.into());
        self
    }

    pub(crate) fn prerelease(mut self, prerelease: bool) -> Self {
        self.prerelease = Some(prerelease);
        self
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub(crate) struct Release {
    pub(crate) url: String,
    pub(crate) html_url: String,
    pub(crate) assets_url: String,
    pub(crate) upload_url: String,
    pub(crate) tarball_url: String,
    pub(crate) zipball_url: String,
    pub(crate) id: u64,
    pub(crate) tag_name: String,
    pub(crate) target_commitish: String,
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) body: Option<String>,
    pub(crate) draft: bool,
    pub(crate) prerelease: bool,
    //pub(crate) created_at: DateTime<FixedOffset>,
    //pub(crate) published_at: DateTime<FixedOffset>,
    //pub(crate) author: SimpleUser,
    //pub(crate) assets: Vec<ReleaseAsset>,
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
