use crate::http_util::RaisingResponse;
use anyhow::Context;
use reqwest::blocking::{Client, ClientBuilder};
use reqwest::header::{self, HeaderMap};
use reqwest::Url;
use serde::{de::DeserializeOwned, Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

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
    client: Client,
}

impl GitHub {
    pub fn new() -> anyhow::Result<GitHub> {
        let token = gh_token::get().context("Failed to retrieve GitHub token")?;
        let mut headers = HeaderMap::new();
        let mut auth = header::HeaderValue::try_from(&format!("token {token}"))
            .context("Failed to create Authorization header value")?;
        auth.set_sensitive(true);
        headers.insert(header::AUTHORIZATION, auth);
        headers.insert(
            header::ACCEPT,
            header::HeaderValue::try_from("application/vnd.github+json")
                .context("Failed to create Accept header value")?,
        );
        let client = ClientBuilder::new()
            .user_agent(USER_AGENT)
            .default_headers(headers)
            .https_only(true)
            .build()
            .context("Failed to construct GitHub client")?;
        Ok(GitHub { client })
    }

    fn get<T: DeserializeOwned>(&self, path: &str) -> anyhow::Result<T> {
        self.client
            .get(mkurl(path)?)
            .send()
            .with_context(|| format!("Failed to make GET request to {path}"))?
            .raise_for_status()?
            .json::<T>()
            .with_context(|| format!("Failed to deserialize response from {path}"))
    }

    fn post<T: Serialize, U: DeserializeOwned>(&self, path: &str, body: &T) -> anyhow::Result<U> {
        self.client
            .post(mkurl(path)?)
            .json(body)
            .send()
            .with_context(|| format!("Failed to make POST request to {path}"))?
            .raise_for_status()?
            .json::<U>()
            .with_context(|| format!("Failed to deserialize response from {path}"))
    }

    fn put<T: Serialize, U: DeserializeOwned>(&self, path: &str, body: &T) -> anyhow::Result<U> {
        self.client
            .put(mkurl(path)?)
            .json(body)
            .send()
            .with_context(|| format!("Failed to make PUT request to {path}"))?
            .raise_for_status()?
            .json::<U>()
            .with_context(|| format!("Failed to deserialize response from {path}"))
    }

    fn create_repository(&self, config: NewRepoConfig) -> anyhow::Result<Repository> {
        let (create_repo_body, set_topics_body) = config.into_payloads();
        let r: Repository = self.post("/user/repos", &create_repo_body)?;
        if !set_topics_body.is_empty() {
            let _: SetTopicsBody = self.put(&r.url, &set_topics_body)?;
        }
        Ok(r)
    }
}

fn mkurl(path: &str) -> anyhow::Result<Url> {
    Url::parse(API_ENDPOINT)
        .context("Failed to construct a Url for the GitHub API endpoint")?
        .join(path)
        .with_context(|| format!("Failed to construct a URL with path {path:?}"))
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
struct Repository {
    id: u64,
    name: String,
    full_name: String,
    private: bool,
    html_url: String,
    description: String,
    url: String,
    ssh_url: String,
    topics: Vec<String>,
    // owner?
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
    pub fn new(name: &str) -> Self {
        Self {
            name: name.into(),
            description: None,
            private: None,
            topics: Vec::new(),
        }
    }

    pub fn description(mut self, description: &str) -> Self {
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
    fn new(s: &str) -> Topic {
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
