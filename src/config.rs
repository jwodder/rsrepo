use anyhow::{bail, Context};
use fs_err::read_to_string;
use serde::Deserialize;
use std::borrow::Cow;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct Config {
    pub author: String,
    pub author_email: String,
    pub github_user: Option<String>,
}

impl Config {
    pub fn load(path: Option<&Path>) -> anyhow::Result<Self> {
        let path: Cow<'_, Path> = match path {
            Some(p) => p.into(),
            None => Config::default_path()?.into(),
        };
        let src = read_to_string(path)?;
        toml::from_str::<Config>(&src).context("Failed to deserialize config file")
    }

    fn default_path() -> anyhow::Result<PathBuf> {
        let Some(home) = home::home_dir() else {
            bail!("Could not determine home directory");
        };
        Ok(home.join(".config").join("rsrepo.toml"))
    }
}
