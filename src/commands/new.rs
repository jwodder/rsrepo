use crate::cmd::LoggedCommand;
use crate::config::Config;
use crate::git::Git;
use crate::tmpltr::Templater;
use crate::util::this_year;
use anyhow::{bail, Context};
use clap::Args;
use serde::Serialize;
use std::ffi::OsStr;
use std::path::PathBuf;

/// Create a new repository and populate it with Rust packaging boilerplate
#[derive(Args, Clone, Debug, Eq, PartialEq)]
pub struct New {
    /// Template a binary crate
    ///
    /// This is the default if neither `--bin` nor `--lib` is given.
    #[clap(long)]
    bin: bool,

    /// Template a library crate
    #[clap(long)]
    lib: bool,

    /// Name of project; defaults to the directory basename
    #[clap(short = 'p', long, value_name = "NAME")]
    project_name: Option<String>,

    /// GitHub repository name; defaults to the project name
    #[clap(long, value_name = "NAME")]
    repo_name: Option<String>,

    /// Copyright year(s) to put in the LICENSE; defaults to the current year
    #[clap(long, value_name = "STRING")]
    copyright_year: Option<String>,

    /// Directory to create & populate
    #[clap(value_name = "PATH")]
    dirpath: PathBuf,
}

impl New {
    pub fn run(self, config: Config) -> anyhow::Result<()> {
        let mut templater = Templater::load()?;
        let project_name = self.project_name()?;
        let author_email = templater
            .render_str(
                &config.author_email,
                AuthorEmailContext {
                    project_name: project_name.into(),
                },
            )
            .context("Failed to render author-email template")?;

        log::info!("Creating Git repository ...");
        LoggedCommand::new("git")
            .arg("init")
            .arg("--")
            .arg(&self.dirpath)
            .status()
            .context("Failed to init Git repository")?;

        let default_branch = Git::new(&self.dirpath)
            .current_branch()?
            .ok_or_else(|| anyhow::anyhow!("No branch set in new repository"))?;
        let context = NewContext {
            github_user: config.github_user,
            author: config.author,
            author_email,
            copyright_year: self.copyright_year(),
            project_name: project_name.into(),
            repo_name: self.repo_name()?.into(),
            default_branch,
            bin: self.bin(),
            lib: self.lib(),
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
        if self.bin() {
            log::info!("Rendering src/main.rs ...");
            templater.render_file(&self.dirpath, "src/main.rs", &context)?;
        }
        if self.lib() {
            log::info!("Rendering src/lib.rs ...");
            templater.render_file(&self.dirpath, "src/lib.rs", &context)?;
        }
        LoggedCommand::new("pre-commit")
            .arg("install")
            .current_dir(&self.dirpath)
            .status()?;
        Ok(())
    }

    pub fn bin(&self) -> bool {
        self.bin || !self.lib
    }

    pub fn lib(&self) -> bool {
        self.lib
    }

    pub fn copyright_year(&self) -> String {
        match self.copyright_year.as_ref() {
            Some(s) => s.clone(),
            None => this_year().to_string(),
        }
    }

    fn project_name(&self) -> anyhow::Result<&str> {
        if let Some(s) = self.project_name.as_ref() {
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
            self.project_name()
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct NewContext {
    github_user: String,
    author: String,
    author_email: String,
    copyright_year: String,
    project_name: String,
    repo_name: String,
    default_branch: String,
    bin: bool,
    lib: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct AuthorEmailContext {
    project_name: String,
}
