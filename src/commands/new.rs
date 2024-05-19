use crate::cmd::LoggedCommand;
use crate::git::Git;
use crate::provider::Provider;
use crate::tmpltr::Templater;
use crate::util::{this_year, RustVersion};
use anyhow::{bail, Context};
use clap::Args;
use ghrepo::GHRepo;
use serde::Serialize;
use std::borrow::Cow;
use std::ffi::OsStr;
use std::path::PathBuf;
use which::which;

/// Create a new repository and populate it with Rust packaging boilerplate
#[derive(Args, Clone, Debug, Eq, PartialEq)]
pub(crate) struct New {
    /// Template a binary crate
    #[arg(long)]
    bin: bool,

    /// Copyright year(s) to put in the LICENSE; defaults to the current year
    #[arg(long, value_name = "STRING")]
    copyright_year: Option<String>,

    /// Package description
    #[arg(short = 'd', long)]
    description: Option<String>,

    /// Template a library crate
    ///
    /// This is the default if neither `--bin` nor `--lib` is given.
    #[arg(long)]
    lib: bool,

    /// MSRV for the new crate.  Defaults to the latest stable rustc version.
    #[arg(long)]
    msrv: Option<RustVersion>,

    /// Name of package; defaults to the directory basename
    #[arg(long, value_name = "NAME")]
    name: Option<String>,

    /// GitHub repository name; defaults to the package name
    #[arg(long, value_name = "NAME")]
    repo_name: Option<String>,

    /// Directory to create & populate
    #[arg(value_name = "PATH")]
    dirpath: PathBuf,
}

impl New {
    pub(crate) fn run(self, provider: Provider) -> anyhow::Result<()> {
        let config = provider.config()?;
        let mut templater = Templater::load()?;
        let name = self.name()?;
        let author_email = templater
            .render_str(&config.author_email, AuthorEmailContext { package: name })
            .context("Failed to render author-email template")?;

        let msrv = if let Some(rv) = self.msrv {
            rv
        } else {
            let rustrepo = GHRepo::new("rust-lang", "rust")
                .expect("\"rust-lang/rust\" should be valid ghrepo specifier");
            let stable = provider.github()?.latest_release(&rustrepo)?;
            stable
                .tag_name
                .parse::<RustVersion>()
                .context("Failed to parse latest stable rustc version")?
                .without_patch()
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
        let github_user = match config.github_user.as_deref() {
            Some(user) => Cow::Borrowed(user),
            None => Cow::Owned(provider.github()?.whoami()?),
        };
        let context = NewContext {
            github_user,
            author: &config.author,
            author_email,
            copyright_year: self.copyright_year(),
            name,
            repo_name: self.repo_name()?,
            default_branch,
            bin,
            lib,
            msrv,
            description: self.description.as_deref(),
        };

        for template in [
            "Cargo.toml",
            ".gitignore",
            "LICENSE",
            "README.md",
            "clippy.toml",
            ".pre-commit-config.yaml",
            ".github/renovate.json5",
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

    pub(crate) fn bin(&self) -> bool {
        self.bin
    }

    pub(crate) fn lib(&self) -> bool {
        self.lib || !self.bin
    }

    pub(crate) fn copyright_year(&self) -> Cow<'_, str> {
        match self.copyright_year.as_ref() {
            Some(s) => s.into(),
            None => this_year().to_string().into(),
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
struct NewContext<'a> {
    github_user: Cow<'a, str>,
    author: &'a str,
    author_email: String,
    copyright_year: Cow<'a, str>,
    name: &'a str,
    repo_name: &'a str,
    default_branch: String,
    bin: bool,
    lib: bool,
    msrv: RustVersion,
    description: Option<&'a str>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct AuthorEmailContext<'a> {
    package: &'a str,
}
