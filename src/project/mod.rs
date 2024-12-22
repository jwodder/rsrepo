mod package;
mod pkgset;
mod textfile;
mod util;
pub(crate) use self::package::Package;
pub(crate) use self::pkgset::PackageSet;
//pub(crate) use self::textfile::TextFile;
use self::util::locate_project;
use anyhow::Context;
use cargo_metadata::MetadataCommand;
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
            Cargo::Workspace { workspace, .. } => (
                ProjectType::Workspace,
                workspace.package.and_then(|pkg| pkg.repository),
            ),
            Cargo::Virtual { workspace } => (
                ProjectType::VirtualWorkspace,
                workspace.package.and_then(|pkg| pkg.repository),
            ),
        };
        Ok(Project {
            manifest_path,
            project_type,
            repository,
        })
    }

    pub(crate) fn path(&self) -> &Path {
        self.manifest_path()
            .parent()
            .expect("manifest_path should have a parent")
    }

    pub(crate) fn manifest_path(&self) -> &Path {
        &self.manifest_path
    }

    pub(crate) fn project_type(&self) -> ProjectType {
        self.project_type
    }

    pub(crate) fn repository(&self) -> Option<&str> {
        self.repository.as_deref()
    }

    pub(crate) fn package_set(&self) -> anyhow::Result<PackageSet> {
        log::debug!("Running `cargo metadata`");
        let mut names = std::collections::HashSet::new();
        let package_metadata = MetadataCommand::new()
            .manifest_path(&self.manifest_path)
            .no_deps()
            .exec()
            .context("Failed to get project metadata")?
            .packages;
        let mut packages = Vec::with_capacity(package_metadata.len());
        for md in package_metadata {
            if !names.insert(md.name.clone()) {
                anyhow::bail!(
                    "Workspace contains multiple packages named {:?}; not proceeding",
                    md.name
                );
            }
            let is_root = md.manifest_path == self.manifest_path;
            packages.push(Package::new(md, is_root));
        }
        Ok(PackageSet::new(packages))
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum ProjectType {
    Package,
    Workspace,
    VirtualWorkspace,
}

impl ProjectType {
    pub(crate) fn is_workspace(&self) -> bool {
        matches!(self, ProjectType::Workspace | ProjectType::VirtualWorkspace)
    }
}

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
    package: Option<WorkspacePackage>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
struct WorkspacePackage {
    repository: Option<String>,
}
