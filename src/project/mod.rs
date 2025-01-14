mod package;
mod pkgset;
mod textfile;
mod traits;
mod util;
pub(crate) use self::package::Package;
pub(crate) use self::pkgset::PackageSet;
pub(crate) use self::textfile::TextFile;
pub(crate) use self::traits::HasReadme;
use self::util::locate_project;
use crate::git::Git;
use crate::readme::Readme;
use anyhow::{bail, Context};
use cargo_metadata::MetadataCommand;
use semver::VersionReq;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use thiserror::Error;
use toml_edit::DocumentMut;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Project {
    manifest_path: PathBuf,
    projtype: ProjectType,
    flavor: Flavor,
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
        let (projtype, flavor) = match data {
            Cargo::Package { package } => {
                let PackageTbl { name, flavor } = package;
                let mut flavor = flavor.map(Flavor::from).unwrap_or_default();
                flavor.name = Some(name);
                (ProjectType::Package, flavor)
            }
            Cargo::Workspace { workspace, .. } => (
                ProjectType::Workspace,
                workspace.package.map(Flavor::from).unwrap_or_default(),
            ),
            Cargo::Virtual { workspace } => (
                ProjectType::VirtualWorkspace,
                workspace.package.map(Flavor::from).unwrap_or_default(),
            ),
        };
        Ok(Project {
            manifest_path,
            projtype,
            flavor,
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
        self.projtype
    }

    pub(crate) fn repository(&self) -> Option<&str> {
        self.flavor.repository.as_deref()
    }

    pub(crate) fn git(&self) -> Git<'_> {
        Git::new(self.path())
    }

    pub(crate) fn package_set(&self) -> anyhow::Result<PackageSet> {
        log::debug!("Running `cargo metadata`");
        let package_metadata = MetadataCommand::new()
            .manifest_path(&self.manifest_path)
            .no_deps()
            .exec()
            .context("Failed to get project metadata")?
            .packages;
        let mut packages = BTreeMap::new();
        // Mapping from package names to the names of the packages that depend
        // on them and their version reqs
        let mut rdeps: BTreeMap<String, BTreeMap<String, VersionReq>> = BTreeMap::new();
        for md in package_metadata {
            if packages.contains_key(&md.name) {
                anyhow::bail!(
                    "Workspace contains multiple packages named {:?}; not proceeding",
                    md.name
                );
            }
            let is_root = md.manifest_path == self.manifest_path;
            for dep in &md.dependencies {
                if dep
                    .path
                    .as_ref()
                    .is_some_and(|p| p.starts_with(self.path()))
                {
                    rdeps
                        .entry(dep.name.clone())
                        .or_default()
                        .insert(md.name.clone(), dep.req.clone());
                }
            }
            let name = md.name.clone();
            packages.insert(name, (md, is_root));
        }
        let mut package_vec = Vec::with_capacity(packages.len());
        for (pkgname, (md, root)) in packages {
            let dependents = rdeps.remove(&pkgname).unwrap_or_default();
            package_vec.push(Package::new(md, root, dependents));
        }
        // TODO: Warn if `rdeps` is non-empty?
        Ok(PackageSet::new(package_vec))
    }

    pub(crate) fn manifest(&self) -> TextFile<'_, DocumentMut> {
        TextFile::new(self.path(), "Cargo.toml")
    }

    pub(crate) fn set_workspace_package_field<V: Into<toml_edit::Value>>(
        &self,
        key: &str,
        value: V,
    ) -> anyhow::Result<()> {
        let manifest = self.manifest();
        let Some(mut doc) = manifest.get()? else {
            bail!("Project lacks Cargo.toml");
        };
        let Some(pkg) = doc
            .get_mut("workspace")
            .and_then(|it| it.as_table_like_mut())
            .and_then(|tbl| tbl.get_mut("package"))
            .and_then(|it| it.as_table_like_mut())
        else {
            bail!("No [workspace.package] table in Cargo.toml");
        };
        pkg.insert(key, toml_edit::value(value));
        manifest.set(doc)?;
        Ok(())
    }

    pub(crate) fn flavor(&self) -> &Flavor {
        &self.flavor
    }
}

impl HasReadme for Project {
    fn readme(&self) -> TextFile<'_, Readme> {
        TextFile::new(self.path(), "README.md")
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

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Flavor {
    pub(crate) name: Option<String>,
    pub(crate) description: Option<String>,
    pub(crate) repository: Option<String>,
    pub(crate) keywords: Vec<String>,
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
    #[serde(flatten)]
    flavor: Option<PackageFlavor>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
struct Workspace {
    package: Option<PackageFlavor>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
struct PackageFlavor {
    description: Option<String>,
    repository: Option<String>,
    keywords: Option<Vec<String>>,
}

impl From<PackageFlavor> for Flavor {
    fn from(value: PackageFlavor) -> Flavor {
        Flavor {
            name: None,
            description: value.description,
            repository: value.repository,
            keywords: value.keywords.unwrap_or_default(),
        }
    }
}
