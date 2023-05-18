use crate::cmd::LoggedCommand;
use crate::config::Config;
use crate::git::Git;
use crate::github::GitHub;
use crate::tmpltr::Templater;
use crate::util::{this_year, RustVersion};
use anyhow::{bail, Context};
use clap::Args;
use ghrepo::GHRepo;
use serde::Serialize;
use std::ffi::OsStr;
use std::path::PathBuf;
use which::which;

/// Create a new repository and populate it with Rust packaging boilerplate
#[derive(Args, Clone, Debug, Eq, PartialEq)]
pub struct New {
    /// Template a binary crate
    #[clap(long)]
    bin: bool,

    /// Copyright year(s) to put in the LICENSE; defaults to the current year
    #[clap(long, value_name = "STRING")]
    copyright_year: Option<String>,

    /// Package description
    #[clap(short = 'd', long)]
    description: Option<String>,

    /// Template a library crate
    ///
    /// This is the default if neither `--bin` nor `--lib` is given.
    #[clap(long)]
    lib: bool,

    /// MSRV for the new crate.  Defaults to the latest stable rustc version.
    #[clap(long)]
    msrv: Option<RustVersion>,

    /// Name of package; defaults to the directory basename
    #[clap(long, value_name = "NAME")]
    name: Option<String>,

    /// GitHub repository name; defaults to the package name
    #[clap(long, value_name = "NAME")]
    repo_name: Option<String>,

    /// Directory to create & populate
    #[clap(value_name = "PATH")]
    dirpath: PathBuf,
}

impl New {
    pub fn run(self, config_path: Option<PathBuf>) -> anyhow::Result<()> {
        let config = Config::load(config_path.as_deref())?;
        let mut templater = Templater::load()?;
        let name = self.name()?;
        let author_email = templater
            .render_str(
                &config.author_email,
                AuthorEmailContext {
                    package: name.into(),
                },
            )
            .context("Failed to render author-email template")?;

        let msrv = if let Some(rv) = self.msrv {
            rv
        } else {
            let rustrepo = GHRepo::new("rust-lang", "rust").unwrap();
            let stable = GitHub::default().latest_release(&rustrepo)?;
            stable
                .tag_name
                .parse::<RustVersion>()
                .context("Failed to parse latest stable rustc version")?
        };

        log::info!("Creating Git repository ...");
        LoggedCommand::new("git")
            .arg("init")
            .arg("--")
            .arg(&self.dirpath)
            .status()
            .context("Failed to init Git repository")?;

        let bin = self.bin();
        let lib = self.lib();
        let default_branch = Git::new(&self.dirpath)
            .current_branch()?
            .ok_or_else(|| anyhow::anyhow!("No branch set in new repository"))?;
        let context = NewContext {
            github_user: config.github_user,
            author: config.author,
            author_email,
            copyright_year: self.copyright_year(),
            name: name.into(),
            repo_name: self.repo_name()?.into(),
            default_branch,
            bin,
            lib,
            msrv,
            description: self.description,
        };

        for template in [
            "Cargo.toml",
            ".gitignore",
            "LICENSE",
            "README.md",
            ".pre-commit-config.yaml",
            ".github/dependabot.yml",
            ".github/workflows/test.yml",
        ] {
            log::info!("Rendering {template} ...");
            templater.render_file(&self.dirpath, template, &context)?;
        }
        if bin {
            log::info!("Rendering src/main.rs ...");
            templater.render_file(&self.dirpath, "src/main.rs", &context)?;
        }
        if lib {
            log::info!("Rendering src/lib.rs ...");
            templater.render_file(&self.dirpath, "src/lib.rs", &context)?;
        }
        if let Ok(pre_commit) = which("pre-commit") {
            LoggedCommand::new(pre_commit)
                .arg("install")
                .current_dir(&self.dirpath)
                .status()?;
        } else {
            log::warn!("pre-commit not found; not running `pre-commit install`");
        }
        Ok(())
    }

    pub fn bin(&self) -> bool {
        self.bin
    }

    pub fn lib(&self) -> bool {
        self.lib || !self.bin
    }

    pub fn copyright_year(&self) -> String {
        match self.copyright_year.as_ref() {
            Some(s) => s.clone(),
            None => this_year().to_string(),
        }
    }

    fn name(&self) -> anyhow::Result<&str> {
        if let Some(s) = self.name.as_ref() {
            return Ok(s);
        }
        if let Some(s) = self.dirpath.file_name().and_then(OsStr::to_str) {
            Ok(s)
        } else {
            bail!(
                "Could not get directory basename as a string: {}",
                self.dirpath.display()
            )
        }
    }

    fn repo_name(&self) -> anyhow::Result<&str> {
        if let Some(s) = self.repo_name.as_ref() {
            Ok(s)
        } else {
            self.name()
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct NewContext {
    github_user: String,
    author: String,
    author_email: String,
    copyright_year: String,
    name: String,
    repo_name: String,
    default_branch: String,
    bin: bool,
    lib: bool,
    msrv: RustVersion,
    description: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct AuthorEmailContext {
    package: String,
}
