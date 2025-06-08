use anyhow::Context;
use base64::{engine::general_purpose::STANDARD, Engine};
use dryoc::{constants::CRYPTO_BOX_PUBLICKEYBYTES, dryocbox::VecBox};
use ghrepo::GHRepo;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::fmt;

/* <https://github.com/jwodder/minigh/issues/17>
static USER_AGENT: &str = concat!(
    env!("CARGO_PKG_NAME"),
    "/",
    env!("CARGO_PKG_VERSION"),
    " (",
    env!("CARGO_PKG_REPOSITORY"),
    ")",
);
*/

#[derive(Clone, Debug)]
pub(crate) struct GitHub(minigh::Client);

impl GitHub {
    pub(crate) fn new(token: &str) -> Result<GitHub, minigh::BuildClientError> {
        Ok(GitHub(minigh::Client::new(token)?))
    }

    pub(crate) fn authed() -> anyhow::Result<GitHub> {
        let token = gh_token::get().context("Failed to retrieve GitHub token")?;
        GitHub::new(&token).map_err(Into::into)
    }

    pub(crate) fn whoami(&self) -> anyhow::Result<String> {
        Ok(self
            .0
            .get::<User>("/user")
            .context("failed to fetch authenticated GitHub user's login name")?
            .login)
    }

    pub(crate) fn create_repository(&self, config: CreateRepoBody) -> anyhow::Result<Repository> {
        self.0.post("/user/repos", &config).map_err(Into::into)
    }

    pub(crate) fn create_label<R>(&self, repo: &R, label: Label<'_>) -> anyhow::Result<()>
    where
        for<'a> R: RepositoryEndpoint<'a>,
    {
        let _: Label<'_> = self.0.post(&format!("{}/labels", repo.api_url()), &label)?;
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
        self.0
            .post(&format!("{}/releases", repo.api_url()), &release)
            .map_err(Into::into)
    }

    pub(crate) fn latest_release<R>(&self, repo: &R) -> anyhow::Result<Release>
    where
        for<'a> R: RepositoryEndpoint<'a>,
    {
        self.0
            .get(&format!("{}/releases/latest", repo.api_url()))
            .map_err(Into::into)
    }

    pub(crate) fn get_topics<R>(&self, repo: &R) -> anyhow::Result<Vec<Topic>>
    where
        for<'a> R: RepositoryEndpoint<'a>,
    {
        let payload = self
            .0
            .get::<TopicsPayload>(&format!("{}/topics", repo.api_url()))?;
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
        let _: TopicsPayload = self.0.put(&format!("{}/topics", repo.api_url()), &body)?;
        Ok(())
    }

    pub(crate) fn set_actions_secret<R>(
        &self,
        repo: &R,
        name: &str,
        value: &str,
    ) -> anyhow::Result<()>
    where
        for<'a> R: RepositoryEndpoint<'a>,
    {
        let secrets = format!("{}/actions/secrets", repo.api_url());
        let pubkey = self.0.get::<PublicKey>(&format!("{secrets}/public-key"))?;
        let payload = CreateSecret {
            encrypted_value: encrypt_secret(&pubkey.key, value)?,
            key_id: pubkey.key_id,
        };
        self.0
            .put::<_, serde::de::IgnoredAny>(&format!("{secrets}/{name}"), &payload)?;
        Ok(())
    }

    pub(crate) fn set_branch_protection<R>(
        &self,
        repo: &R,
        branch: &str,
        body: SetBranchProtection,
    ) -> anyhow::Result<()>
    where
        for<'a> R: RepositoryEndpoint<'a>,
    {
        let url = format!("{}/branches/{}/protection", repo.api_url(), branch);
        self.0.put::<_, serde::de::IgnoredAny>(&url, &body)?;
        Ok(())
    }
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) allow_auto_merge: Option<bool>,
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

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct SetBranchProtection {
    pub(crate) required_status_checks: Option<RequiredStatusChecks>,
    pub(crate) allow_force_pushes: Option<bool>,
    pub(crate) enforce_admins: Option<bool>,
    pub(crate) required_pull_request_reviews: Option<()>,
    pub(crate) restrictions: Option<()>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct RequiredStatusChecks {
    pub(crate) strict: bool,
    pub(crate) contexts: Vec<&'static str>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
struct PublicKey {
    key_id: String,
    key: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct CreateSecret {
    encrypted_value: String,
    key_id: String,
}

fn encrypt_secret(public_key: &str, secret_value: &str) -> anyhow::Result<String> {
    let mut pkey = [0; CRYPTO_BOX_PUBLICKEYBYTES];
    if STANDARD.decode_slice(public_key, &mut pkey) != Ok(CRYPTO_BOX_PUBLICKEYBYTES) {
        anyhow::bail!("decoded public key not valid length");
    };
    let sealed_box =
        VecBox::seal(secret_value.as_bytes(), &pkey).context("failed to encrypt secret value")?;
    Ok(STANDARD.encode(sealed_box.to_vec()))
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
