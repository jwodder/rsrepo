use crate::github::{CreateRepoBody, Label, RequiredStatusChecks, SetBranchProtection, Topic};
use crate::project::Project;
use crate::provider::Provider;
use crate::readme::Repostatus;
use anyhow::bail;
use clap::Args;
use ghrepo::GHRepo;
use std::borrow::Cow;

/// Create a GitHub repository for the project and push
#[derive(Args, Clone, Debug, Eq, PartialEq)]
pub(crate) struct Mkgithub {
    /// Value for `CODECOV_TOKEN` actions secret
    #[arg(
        long,
        value_name = "SECRET",
        env = "CODECOV_TOKEN",
        hide_env_values = true
    )]
    codecov_token: Option<String>,

    /// Do not set `CODECOV_TOKEN` actions secret
    #[arg(long)]
    no_codecov_token: bool,

    /// Make the new repository private
    #[arg(short = 'P', long)]
    private: bool,

    /// Name for the repository
    ///
    /// If not specified, defaults to the name used in the `repository` URL in
    /// the Cargo metadata, or to the name of the package.
    #[arg(value_name = "NAME")]
    repo_name: Option<String>,
}

impl Mkgithub {
    pub(crate) fn run(self, provider: Provider) -> anyhow::Result<()> {
        let github = provider.github()?;
        let project = Project::locate()?;
        let package = project.current_package()?;
        let metadata = package.metadata();
        let name = if let Some(s) = self.repo_name {
            s
        } else {
            match metadata.repository.as_ref().map(|s| s.parse::<GHRepo>()) {
                Some(Ok(r)) => {
                    let github_user = match provider.config()?.github_user.as_ref() {
                        Some(user) => Cow::Borrowed(user),
                        None => Cow::Owned(github.whoami()?),
                    };
                    if r.owner() != *github_user {
                        bail!("Package repository URL does not belong to GitHub user");
                    }
                    r.name().to_string()
                }
                Some(Err(_)) => bail!("Package repository URL does not point to GitHub"),
                None => metadata.name.clone(),
            }
        };

        let repo = github.create_repository(CreateRepoBody {
            name,
            description: metadata.description.clone(),
            private: Some(self.private),
            delete_branch_on_merge: Some(true),
            allow_auto_merge: Some(true),
        })?;
        log::info!("Created GitHub repository {}", repo.html_url);

        log::info!("Setting remote and pushing");
        let git = package.git();
        if git.remotes()?.contains("origin") {
            git.rm_remote("origin")?;
        }
        git.add_remote("origin", &repo.ssh_url)?;
        git.run("push", ["-u", "origin", "refs/heads/*", "refs/tags/*"])?;

        let mut topics = Vec::from([Topic::new("rust")]);
        for keyword in &metadata.keywords {
            let tp = Topic::new(keyword);
            if tp != keyword {
                log::warn!("Keyword {keyword:?} sanitized to \"{tp}\" for use as GitHub topic");
            }
            topics.push(tp);
        }
        if package.readme().get()?.and_then(|r| r.repostatus()) == Some(Repostatus::Wip) {
            topics.push(Topic::new("work-in-progress"));
        }
        log::info!(
            "Setting repository topics to: {}",
            itertools::join(&topics, " ")
        );
        github.set_topics(&repo, topics)?;

        log::info!("Setting protection rules for default branch ...");
        let mut contexts = vec![
            // This needs to be kept in sync with the tests in test.yml.tt:
            "test (ubuntu-latest, msrv)",
            "test (ubuntu-latest, stable)",
            "test (ubuntu-latest, beta)",
            "test (ubuntu-latest, nightly)",
            "test (macos-latest, stable)",
            "test (windows-latest, stable)",
            "minimal-versions",
            "lint",
            "coverage",
        ];
        if package.is_lib() {
            contexts.push("docs");
        }
        let Some(default_branch) = git.default_branch()? else {
            bail!("Could not determine repository's default branch");
        };
        github.set_branch_protection(
            &repo,
            default_branch,
            SetBranchProtection {
                required_status_checks: Some(RequiredStatusChecks {
                    strict: false,
                    contexts,
                }),
                allow_force_pushes: Some(true),
                enforce_admins: Some(false),
                required_pull_request_reviews: None,
                restrictions: None,
            },
        )?;

        log::info!("Creating dependency-update PR labels ...");
        github.create_label(
            &repo,
            Label::new(
                "dependencies",
                "8732bc",
                "Update one or more dependencies' versions",
            ),
        )?;
        github.create_label(
            &repo,
            Label::new("d:cargo", "dea584", "Update a Cargo (Rust) dependency"),
        )?;
        github.create_label(
            &repo,
            Label::new(
                "d:github-actions",
                "74fa75",
                "Update a GitHub Actions action dependency",
            ),
        )?;

        if !self.no_codecov_token {
            let token = match self.codecov_token.as_deref() {
                Some(t) => t,
                None => provider
                    .config()?
                    .codecov_token
                    .as_deref()
                    .unwrap_or_default(),
            };
            if !token.is_empty() {
                log::info!("Setting CODECOV_TOKEN secret");
                github.set_actions_secret(&repo, "CODECOV_TOKEN", token)?;
            } else {
                log::warn!("CODECOV_TOKEN value not set; not setting secret");
            }
        }

        if metadata.repository.is_none() {
            log::info!("Setting 'package.repository' field in Cargo.toml ...");
            package.set_package_field("repository", repo.html_url)?;
        } else if metadata.repository != Some(repo.html_url) {
            log::warn!(
                "'package.repository' field in Cargo.toml differs from GitHub repository URL"
            );
        }

        Ok(())
    }
}
