use crate::project::Project;
use crate::provider::Provider;
use clap::Args;

/// Begin work on the next version of the project
#[derive(Args, Clone, Debug, Eq, PartialEq)]
pub(crate) struct BeginDev;

impl BeginDev {
    pub(crate) fn run(self, _provider: Provider) -> anyhow::Result<()> {
        let project = Project::locate()?;
        let pkgset = project.package_set()?;
        let package = pkgset.get(None)?;
        package.begin_dev(&pkgset).run()?;
        Ok(())
    }
}
