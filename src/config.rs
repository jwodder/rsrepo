use anyhow::{bail, Context};
use serde::Deserialize;
use std::borrow::Cow;
use std::fs::read_to_string;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct Config {
    pub author: String,
    pub author_email: String,
    pub github_user: String,
}

impl Config {
    pub fn load(path: Option<&Path>) -> anyhow::Result<Self> {
        let path: Cow<'_, Path> = match path {
            Some(p) => p.into(),
            None => Config::default_path()?.into(),
        };
        let src = read_to_string(&path)
            .with_context(|| format!("Failed to read config file at {}", path.display()))?;
        toml::from_str::<Config>(&src).context("Failed to deserialize config file")
    }

    fn default_path() -> anyhow::Result<PathBuf> {
        let home = match home::home_dir() {
            Some(p) => p,
            None => bail!("Could not determine home directory"),
        };
        Ok(home.join(".config").join("rsrepo.toml"))
    }
}
