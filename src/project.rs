use crate::package::Package;
use crate::util::locate_project;
use anyhow::Context;
use cargo_metadata::{MetadataCommand, Package as CargoPackage};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct Project {
    manifest_path: PathBuf,
    project_type: ProjectType,
    repository: Option<String>,
}

impl Project {
    pub(crate) fn locate() -> anyhow::Result<Project> {
        Project::for_manifest_path(locate_project(true)?)
    }

    pub(crate) fn for_manifest_path<P: AsRef<Path>>(manifest_path: P) -> anyhow::Result<Project> {
        let manifest_path = PathBuf::from(manifest_path.as_ref());
        let src = fs_err::read_to_string(&manifest_path)?;
        let data = toml::from_str::<Cargo>(&src)
            .with_context(|| format!("failed to deserialize {}", manifest_path.display()))?;
        let (project_type, repository) = match data {
            Cargo::Package { package, .. } => (ProjectType::Package, package.repository),
            Cargo::Workspace { workspace, .. } => {
                (ProjectType::Workspace, workspace.package.repository)
            }
            Cargo::Virtual { workspace } => {
                (ProjectType::VirtualWorkspace, workspace.package.repository)
            }
        };
        Ok(Project {
            manifest_path,
            project_type,
            repository,
        })
    }

    fn packages(&self) -> anyhow::Result<Vec<CargoPackage>> {
        Ok(MetadataCommand::new()
            .manifest_path(&self.manifest_path)
            .no_deps()
            .exec()
            .context("Failed to get project metadata")?
            .packages)
    }

    pub(crate) fn current_package(&self) -> anyhow::Result<Package> {
        let manifest_path = locate_project(false)?;
        let mut matches = self
            .packages()?
            .into_iter()
            .filter(|p| p.manifest_path == manifest_path)
            .collect::<Vec<_>>();
        let metadata = if matches.len() == 1 {
            matches.pop().expect("one-length Vec should not be empty")
        } else {
            anyhow::bail!("failed to find package in workspace for current directory");
        };
        Ok(Package::new(manifest_path, metadata))
    }

    /*
        pub(crate) fn package(&self, name: &str) -> anyhow::Result<Package> {
            // Use `cargo metadata`
            todo!()
        }
    */

    #[allow(dead_code)] // TODO
    pub(crate) fn root_package(&self) -> anyhow::Result<Option<Package>> {
        if self.project_type == ProjectType::VirtualWorkspace {
            return Ok(None);
        }
        let mut matches = self
            .packages()?
            .into_iter()
            .filter(|p| p.manifest_path == self.manifest_path)
            .collect::<Vec<_>>();
        let metadata = if matches.len() == 1 {
            matches.pop().expect("one-length Vec should not be empty")
        } else {
            anyhow::bail!("failed to find root package in workspace");
        };
        Ok(Some(Package::new(self.manifest_path.clone(), metadata)))
    }

    /*
        pub(crate) fn is_root_package(&self, pkg: &Package) -> bool {
            self.manifest_path == pkg.manifest_path()
        }
    */
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum ProjectType {
    Package,
    Workspace,
    VirtualWorkspace,
}

/*
impl ProjectType {
    pub(crate) fn is_workspace(&self) -> bool {
        matches!(self, ProjectType::Workspace | ProjectType::VirtualWorkspace)
    }
}
*/

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(try_from = "RawCargo")]
enum Cargo {
    Package {
        package: PackageTbl,
    },
    Workspace {
        workspace: Workspace,
        package: PackageTbl,
    },
    Virtual {
        workspace: Workspace,
    },
}

/*
impl Cargo {
    fn name(&self) -> &str {
        match self {
            Cargo::Workspace { package, .. } => &package.name,
            Cargo::Virtual { workspace } => workspace.package.repository.name(),
            Cargo::Package { package } => &package.name,
        }
    }
}
*/

impl TryFrom<RawCargo> for Cargo {
    type Error = FromRawCargoError;

    fn try_from(value: RawCargo) -> Result<Cargo, FromRawCargoError> {
        match value {
            RawCargo {
                package: Some(package),
                workspace: None,
            } => Ok(Cargo::Package { package }),
            RawCargo {
                package: Some(package),
                workspace: Some(workspace),
            } => Ok(Cargo::Workspace { workspace, package }),
            RawCargo {
                package: None,
                workspace: Some(workspace),
            } => Ok(Cargo::Virtual { workspace }),
            RawCargo {
                package: None,
                workspace: None,
            } => Err(FromRawCargoError),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[error("Cargo.toml lacks both [package] and [workspace] tables")]
pub(crate) struct FromRawCargoError;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
struct RawCargo {
    package: Option<PackageTbl>,
    workspace: Option<Workspace>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
struct PackageTbl {
    name: String,
    repository: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
struct Workspace {
    package: WorkspacePackage,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
struct WorkspacePackage {
    repository: Option<String>,
}
