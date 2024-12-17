use crate::package::Package;
use crate::project::{Project, ProjectType};
use crate::provider::Provider;
use clap::Args;
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Args, Clone, Debug, Eq, PartialEq)]
pub(crate) struct Inspect {
    #[arg(long)]
    workspace: bool,
}

impl Inspect {
    #[expect(clippy::if_then_some_else_none)] // invalid suggestion
    pub(crate) fn run(self, _provider: Provider) -> anyhow::Result<()> {
        let project = Project::locate()?;
        let current_package = project.current_package()?.map(PackageDetails::from);
        let packages = if self.workspace {
            Some(
                project
                    .packages()?
                    .into_iter()
                    .map(PackageDetails::from)
                    .collect::<Vec<_>>(),
            )
        } else {
            None
        };
        let details = Details {
            manifest_path: project.manifest_path(),
            is_workspace: project.project_type().is_workspace(),
            is_virtual_workspace: project.project_type() == ProjectType::VirtualWorkspace,
            repository: project.repository(),
            current_package,
            packages,
        };
        println!(
            "{}",
            serde_json::to_string_pretty(&details).expect("serialization should not fail")
        );
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct Details<'a> {
    manifest_path: &'a Path,
    is_workspace: bool,
    is_virtual_workspace: bool,
    repository: Option<&'a str>,
    current_package: Option<PackageDetails>,
    #[serde(skip_serializing_if = "Option::is_none")]
    packages: Option<Vec<PackageDetails>>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct PackageDetails {
    name: String,
    manifest_path: PathBuf,
    bin: bool,
    lib: bool,
    root_package: bool,
}

impl From<Package> for PackageDetails {
    fn from(p: Package) -> PackageDetails {
        PackageDetails {
            name: p.name().to_owned(),
            manifest_path: p.manifest_path().to_owned(),
            bin: p.is_bin(),
            lib: p.is_lib(),
            root_package: p.is_root_package(),
        }
    }
}
