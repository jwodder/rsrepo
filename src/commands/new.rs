use crate::cmd::LoggedCommand;
use crate::config::Config;
use crate::tmpltr::Templater;
use anyhow::{bail, Context};
use chrono::Datelike;
use clap::Args;
use serde::Serialize;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

#[derive(Args, Clone, Debug, Eq, PartialEq)]
pub struct New {
    #[clap(long)]
    bin: bool,

    #[clap(long)]
    lib: bool,

    #[clap(short = 'p', long)]
    project_name: Option<String>,

    #[clap(long)]
    repo_name: Option<String>,

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
        let context = NewContext {
            github_user: config.github_user,
            author: config.author,
            author_email,
            this_year: chrono::Local::now().year(),
            project_name: project_name.into(),
            repo_name: self.repo_name()?.into(),
            bin: self.bin(),
            lib: self.lib(),
        };
        log::info!("Creating Git repository ...");
        LoggedCommand::new("git", [Path::new("init"), Path::new("--"), &self.dirpath])
            .status()
            .context("Failed to init Git repository")?;
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
        Ok(())
    }

    pub fn bin(&self) -> bool {
        self.bin || !self.lib
    }

    pub fn lib(&self) -> bool {
        self.lib
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
    this_year: i32,
    project_name: String,
    repo_name: String,
    bin: bool,
    lib: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct AuthorEmailContext {
    project_name: String,
}
