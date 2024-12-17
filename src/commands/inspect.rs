use crate::project::{Project, ProjectType};
use crate::provider::Provider;
use clap::Args;
use serde::Serialize;
use std::path::Path;

/// Prepare & publish a new release for a package
#[derive(Args, Clone, Debug, Eq, PartialEq)]
pub(crate) struct Inspect;

impl Inspect {
    pub(crate) fn run(self, _provider: Provider) -> anyhow::Result<()> {
        let project = Project::locate()?;
        let package = project.current_package()?;
        let current_package = package.as_ref().map(|p| PackageDetails {
            name: p.name(),
            manifest_path: p.manifest_path(),
            bin: p.is_bin(),
            lib: p.is_lib(),
            root_package: project.is_root_package(p),
        });
        let details = Details {
            manifest_path: project.manifest_path(),
            is_workspace: project.project_type().is_workspace(),
            is_virtual_workspace: project.project_type() == ProjectType::VirtualWorkspace,
            repository: project.repository(),
            current_package,
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
    current_package: Option<PackageDetails<'a>>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct PackageDetails<'a> {
    name: &'a str,
    manifest_path: &'a Path,
    bin: bool,
    lib: bool,
    root_package: bool,
}
