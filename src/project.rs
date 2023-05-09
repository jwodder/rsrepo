use crate::cmd::{CommandOutputError, LoggedCommand};
use crate::git::Git;
use anyhow::Context;
use cargo_metadata::{MetadataCommand, Package};
use serde::Deserialize;
use std::borrow::Cow;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Project {
    manifest_path: PathBuf,
}

impl Project {
    pub fn locate() -> Result<Project, LocateProjectError> {
        let output = LoggedCommand::new("cargo")
            .arg("locate-project")
            .check_output()?;
        let location = serde_json::from_str::<LocateProject<'_>>(&output)?;
        if !location.root.is_absolute() {
            return Err(LocateProjectError::InvalidPath(location.root.into()));
        }
        if location.root.parent().is_some() {
            Ok(Project {
                manifest_path: location.root.into(),
            })
        } else {
            Err(LocateProjectError::InvalidPath(location.root.into()))
        }
    }

    pub fn path(&self) -> &Path {
        self.manifest_path.parent().unwrap()
    }

    pub fn manifest_path(&self) -> &Path {
        &self.manifest_path
    }

    pub fn git(&self) -> Git<'_> {
        Git::new(self.path())
    }

    pub fn metadata(&self) -> anyhow::Result<Package> {
        MetadataCommand::new()
            .manifest_path(self.manifest_path())
            .no_deps()
            .exec()
            .context("Failed to get project metadata")?
            .packages
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("No projects listed in metadata"))
    }
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq)]
struct LocateProject<'a> {
    #[serde(borrow)]
    root: Cow<'a, Path>,
}

#[derive(Debug, Error)]
pub enum LocateProjectError {
    #[error("could not get project root from cargo")]
    Command(#[from] CommandOutputError),
    #[error("could not deserialize `cargo locate-project` output")]
    Deserialize(#[from] serde_json::Error),
    #[error("manifest path is absolute or parentless: {}", .0.display())]
    InvalidPath(PathBuf),
}
