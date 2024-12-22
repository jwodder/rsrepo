use crate::project::{Package, Project};
use crate::provider::Provider;
use crate::util::RustVersion;
use clap::Args;
use std::fmt::Write;

/// Update package's MSRV
#[derive(Args, Clone, Debug, Eq, PartialEq)]
pub(crate) struct SetMsrv {
    /// Update the MSRV of the package with the given name in the workspace.
    ///
    /// By default, the package for the current directory is updated.
    #[arg(short, long, value_name = "NAME")]
    package: Option<String>,

    /// Update workspace.package.rust-version in the project's root Cargo.toml
    /// and update the README and CHANGELOG for all packages in the workspace
    /// that inherit this value
    #[arg(short, long, conflicts_with = "package")]
    workspace: bool,

    /// New MSRV value
    #[arg(value_name = "VERSION")]
    msrv: RustVersion,
}

impl SetMsrv {
    pub(crate) fn run(self, _provider: Provider) -> anyhow::Result<()> {
        let project = Project::locate()?;
        let pkgset = project.package_set()?;
        if self.workspace {
            log::info!("Updating workspace.package.rust-version");
            project.set_workspace_package_field("rust-version", self.msrv.to_string())?;
            for package in &pkgset {
                if package.package_key_inherits_workspace("rust-version")? {
                    log::info!("Updating {} ...", package.name());
                    update_extras(package, self.msrv)?;
                }
            }
        } else {
            let package = pkgset.get(self.package.as_deref())?;
            log::info!("Updating Cargo.toml ...");
            package.set_package_field("rust-version", self.msrv.to_string())?;
            update_extras(package, self.msrv)?;
        }
        Ok(())
    }
}

fn update_extras(package: &Package, msrv: RustVersion) -> anyhow::Result<()> {
    let readme_file = package.readme();
    if let Some(mut readme) = readme_file.get()? {
        log::info!("Updating README.md ...");
        readme.set_msrv(msrv);
        readme_file.set(readme)?;
    }

    let chlog_file = package.changelog();
    if let Some(mut chlog) = chlog_file.get()? {
        log::info!("Updating CHANGELOG.md ...");
        if let Some(sect1) = chlog.sections.first_mut() {
            let mut content = String::new();
            let mut found = false;
            for ln in sect1.content.lines() {
                if ln.starts_with("- Increased MSRV to ") && !std::mem::replace(&mut found, true) {
                    writeln!(&mut content, "- Increased MSRV to {msrv}")
                        .expect("formatting a String should not fail");
                } else {
                    content.push_str(ln);
                    content.push('\n');
                }
            }
            if !found {
                let mut nlqty = 0;
                while content.ends_with("\n\n") {
                    content.pop();
                    nlqty += 1;
                }
                writeln!(&mut content, "- Increased MSRV to {msrv}")
                    .expect("formatting a String should not fail");
                content.push_str(&"\n".repeat(nlqty));
            }
            sect1.content = content;
        }
        chlog_file.set(chlog)?;
    }
    Ok(())
}
