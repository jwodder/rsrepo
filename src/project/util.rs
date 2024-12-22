use crate::cmd::{CommandOutputError, LoggedCommand};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use thiserror::Error;

pub(super) fn locate_project(workspace: bool) -> Result<PathBuf, LocateError> {
    let mut cmd = LoggedCommand::new("cargo");
    cmd.arg("locate-project");
    if workspace {
        cmd.arg("--workspace");
    }
    let output = cmd.check_output()?;
    let location = serde_json::from_str::<LocateProject<'_>>(&output)?;
    if !location.root.is_absolute() {
        return Err(LocateError::InvalidPath(location.root.into()));
    }
    if location.root.parent().is_some() {
        Ok(location.root.into())
    } else {
        Err(LocateError::InvalidPath(location.root.into()))
    }
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq)]
struct LocateProject<'a> {
    #[serde(borrow)]
    root: std::borrow::Cow<'a, Path>,
}

#[derive(Debug, Error)]
pub(crate) enum LocateError {
    #[error("could not get project root from cargo")]
    Command(#[from] CommandOutputError),
    #[error("could not deserialize `cargo locate-project` output")]
    Deserialize(#[from] serde_json::Error),
    #[error("manifest path is absolute or parentless: {}", .0.display())]
    InvalidPath(PathBuf),
}
