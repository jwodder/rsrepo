#![allow(dead_code)]
use crate::changelog::Changelog;
use crate::cmd::{CommandOutputError, LoggedCommand};
use crate::git::Git;
use crate::readme::Readme;
use anyhow::Context;
use cargo_metadata::{MetadataCommand, Package};
use semver::Version;
use serde::Deserialize;
use std::borrow::Cow;
use std::fs::{read_to_string, File};
use std::io::{ErrorKind, Write};
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

    pub fn is_bin(&self) -> anyhow::Result<bool> {
        let srcdir = self.path().join("src");
        Ok(srcdir
            .join("main.rs")
            .try_exists()
            .context("could not determine whether src/main.rs exists")?
            || srcdir
                .join("bin")
                .try_exists()
                .context("could not determine whether src/bin/ exists")?)
    }

    pub fn is_lib(&self) -> anyhow::Result<bool> {
        let srcdir = self.path().join("src");
        srcdir
            .join("lib.rs")
            .try_exists()
            .context("could not determine whether src/main.rs exists")
    }

    pub fn latest_tag_version(&self) -> anyhow::Result<Option<Version>> {
        if let Some(tag) = self.git().latest_tag()? {
            tag.strip_prefix('v')
                .unwrap_or(&tag)
                .parse::<Version>()
                .with_context(|| format!("Failed to parse latest Git tag {tag:?} as a version"))
                .map(Some)
        } else {
            Ok(None)
        }
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

    pub fn readme(&self) -> anyhow::Result<Option<Readme>> {
        match read_to_string(self.path().join("README.md")) {
            Ok(s) => Ok(Some(s.parse::<Readme>()?)),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e).context("failed to read README.md"),
        }
    }

    pub fn set_readme(&self, readme: Readme) -> anyhow::Result<()> {
        let mut fp = File::create(self.path().join("README.md"))
            .context("failed to open README.md for writing")?;
        write!(&mut fp, "{}", readme).context("failed writing to README.md")?;
        Ok(())
    }

    pub fn changelog(&self) -> anyhow::Result<Option<Changelog>> {
        match read_to_string(self.path().join("CHANGELOG.md")) {
            Ok(s) => Ok(Some(s.parse::<Changelog>()?)),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e).context("failed to read CHANGELOG.md"),
        }
    }

    pub fn set_changelog(&self, changelog: Changelog) -> anyhow::Result<()> {
        let mut fp = File::create(self.path().join("CHANGELG.md"))
            .context("failed to open CHANGELG.md for writing")?;
        write!(&mut fp, "{}", changelog).context("failed writing to CHANGELG.md")?;
        Ok(())
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
