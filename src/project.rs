#![allow(dead_code)]
use crate::cmd::{CommandOutputError, LoggedCommand};
use serde::Deserialize;
use std::borrow::Cow;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Project {
    manifest_file: PathBuf,
}

impl Project {
    pub fn locate() -> Result<Project, LocateProjectError> {
        let output = LoggedCommand::new("cargo", ["locate-project"]).check_output()?;
        let location = serde_json::from_str::<LocateProject<'_>>(&output)?;
        if !location.root.is_absolute() {
            return Err(LocateProjectError::InvalidPath(location.root.into()));
        }
        if location.root.parent().is_some() {
            Ok(Project {
                manifest_file: location.root.into(),
            })
        } else {
            Err(LocateProjectError::InvalidPath(location.root.into()))
        }
    }

    pub fn path(&self) -> &Path {
        self.manifest_file.parent().unwrap()
    }

    pub fn manifest_file(&self) -> &Path {
        &self.manifest_file
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
