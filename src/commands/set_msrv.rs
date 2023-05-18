use crate::package::Package;
use crate::util::RustVersion;
use clap::Args;
use std::fmt::Write;
use std::path::PathBuf;

/// Update package's MSRV
#[derive(Args, Clone, Debug, Eq, PartialEq)]
pub struct SetMsrv {
    /// New MSRV value
    #[clap(value_name = "VERSION")]
    msrv: RustVersion,
}

impl SetMsrv {
    pub fn run(self, _: Option<PathBuf>) -> anyhow::Result<()> {
        let package = Package::locate()?;

        log::info!("Updating Cargo.toml ...");
        package.set_package_field("rust-version", self.msrv.to_string())?;

        let readme_file = package.readme();
        if let Some(mut readme) = readme_file.get()? {
            log::info!("Updating README.md ...");
            readme.set_msrv(self.msrv);
            readme_file.set(readme)?;
        }

        let chlog_file = package.changelog();
        if let Some(mut chlog) = chlog_file.get()? {
            log::info!("Updating CHANGELOG.md ...");
            if let Some(sect1) = chlog.sections.first_mut() {
                let mut content = String::new();
                let mut found = false;
                for ln in sect1.content.lines() {
                    if ln.starts_with("- Increased MSRV to ")
                        && !std::mem::replace(&mut found, true)
                    {
                        writeln!(&mut content, "- Increased MSRV to {}", self.msrv).unwrap();
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
                    writeln!(&mut content, "- Increased MSRV to {}", self.msrv).unwrap();
                    content.push_str(&"\n".repeat(nlqty));
                }
                sect1.content = content;
            }
            chlog_file.set(chlog)?;
        }

        Ok(())
    }
}
