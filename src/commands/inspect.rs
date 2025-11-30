use crate::project::{Package, Project, ProjectType};
use crate::provider::Provider;
use cargo_metadata::semver::VersionReq;
use clap::Args;
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::Path;

/// Display details about current project/package
#[derive(Args, Clone, Debug, Eq, PartialEq)]
pub(crate) struct Inspect {
    /// List all packages in the workspace
    #[arg(short, long)]
    workspace: bool,
}

impl Inspect {
    pub(crate) fn run(self, _provider: Provider) -> anyhow::Result<()> {
        let project = Project::locate()?;
        let pkgset = project.package_set()?;
        let current_package = pkgset.current_package()?.map(PackageDetails::from);
        let packages = self
            .workspace
            .then(|| pkgset.iter().map(PackageDetails::from).collect());
        let details = Details {
            manifest_path: project.manifest_path(),
            is_workspace: project.project_type().is_workspace(),
            is_virtual_workspace: project.project_type() == ProjectType::VirtualWorkspace,
            repository: project.repository(),
            current_package,
            packages,
        };
        println!("{}", serde_json::to_string_pretty(&details)?);
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct Details<'a> {
    manifest_path: &'a Path,
    is_workspace: bool,
    is_virtual_workspace: bool,
    repository: Option<&'a str>,
    current_package: Option<PackageDetails<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    packages: Option<Vec<PackageDetails<'a>>>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct PackageDetails<'a> {
    name: &'a str,
    manifest_path: &'a Path,
    bin: bool,
    lib: bool,
    root_package: bool,
    public: bool,
    dependents: &'a BTreeMap<String, VersionReq>,
}

impl<'a> From<&'a Package> for PackageDetails<'a> {
    fn from(p: &'a Package) -> PackageDetails<'a> {
        PackageDetails {
            name: p.name(),
            manifest_path: p.manifest_path(),
            bin: p.is_bin(),
            lib: p.is_lib(),
            root_package: p.is_root_package(),
            public: p.is_public(),
            dependents: p.dependents(),
        }
    }
}
