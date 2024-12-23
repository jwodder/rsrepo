use crate::github::{CreateRepoBody, Label, RequiredStatusChecks, SetBranchProtection, Topic};
use crate::project::{HasReadme, Package, Project};
use crate::provider::Provider;
use crate::readme::Repostatus;
use anyhow::bail;
use clap::Args;
use ghrepo::GHRepo;
use serde::{ser::Serializer, Serialize};
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
        let cts = match (self.no_codecov_token, self.codecov_token) {
            (true, _) => CodecovTokenSource::None,
            (false, Some(token)) => CodecovTokenSource::Cli(token),
            (false, None) => CodecovTokenSource::Config,
        };
        let project = Project::locate()?;
        let ghmaker = GitHubMaker::new(project, provider)?
            .with_repo_name(self.repo_name)
            .with_private(self.private)
            .with_codecov_token_source(cts);
        let plan = ghmaker.plan()?;
        ghmaker.execute(plan)?;
        Ok(())
    }
}

#[derive(Clone, Debug)]
struct GitHubMaker {
    project: Project,
    provider: Provider,
    has_lib: bool,
    root_package: Option<Package>,
    repo_name: Option<String>,
    private: bool,
    codecov_token_source: CodecovTokenSource,
}

impl GitHubMaker {
    fn new(project: Project, provider: Provider) -> anyhow::Result<GitHubMaker> {
        let pkgset = project.package_set()?;
        let has_lib = pkgset.iter().any(Package::is_lib);
        let root_package = pkgset.into_root_package();
        Ok(GitHubMaker {
            project,
            provider,
            has_lib,
            root_package,
            repo_name: None,
            private: false,
            codecov_token_source: CodecovTokenSource::None,
        })
    }

    fn with_repo_name(mut self, repo_name: Option<String>) -> Self {
        self.repo_name = repo_name;
        self
    }

    fn with_private(mut self, private: bool) -> Self {
        self.private = private;
        self
    }

    fn with_codecov_token_source(mut self, codecov_token_source: CodecovTokenSource) -> Self {
        self.codecov_token_source = codecov_token_source;
        self
    }

    fn plan(&self) -> anyhow::Result<Plan> {
        let flavor = self
            .root_package
            .as_ref()
            .map_or_else(|| self.project.flavor().clone(), Package::flavor);
        let repo_name = if let Some(s) = self.repo_name.clone() {
            s
        } else {
            match flavor.repository.as_ref().map(|s| s.parse::<GHRepo>()) {
                Some(Ok(r)) => {
                    let github_user = match self.provider.config()?.github_user.as_ref() {
                        Some(user) => Cow::Borrowed(user),
                        None => Cow::Owned(self.provider.github()?.whoami()?),
                    };
                    if r.owner() != *github_user {
                        bail!("Project repository URL does not belong to GitHub user");
                    }
                    r.name().to_string()
                }
                Some(Err(_)) => bail!("Project repository URL does not point to GitHub"),
                None => flavor.name.clone().ok_or_else(|| {
                    anyhow::anyhow!("No repository URL found to determine repository name from")
                })?,
            }
        };

        let mut topics = Vec::from([Topic::new("rust")]);
        for keyword in &flavor.keywords {
            let tp = Topic::new(keyword);
            if tp != keyword {
                log::warn!("Keyword {keyword:?} sanitized to \"{tp}\" for use as GitHub topic");
            }
            topics.push(tp);
        }
        let readme = self
            .root_package
            .as_ref()
            .map_or_else(|| self.project.readme(), HasReadme::readme);
        if readme.get()?.and_then(|r| r.repostatus()) == Some(Repostatus::Wip) {
            topics.push(Topic::new("work-in-progress"));
        }

        let mut required_checks = vec![
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
        if self.has_lib {
            required_checks.push("docs");
        }

        let Some(default_branch) = self.project.git().default_branch()? else {
            bail!("Could not determine repository's default branch");
        };

        let codecov_token = self.codecov_token_source.resolve(&self.provider)?;

        Ok(Plan {
            repo_name,
            description: flavor.description,
            private: self.private,
            topics,
            required_checks,
            default_branch,
            codecov_token,
            expected_repo_url: flavor.repository,
        })
    }

    fn execute(&self, plan: Plan) -> anyhow::Result<()> {
        let github = self.provider.github()?;
        let repo = github.create_repository(CreateRepoBody {
            name: plan.repo_name,
            description: plan.description,
            private: Some(plan.private),
            delete_branch_on_merge: Some(true),
            allow_auto_merge: Some(true),
        })?;
        log::info!("Created GitHub repository {}", repo.html_url);

        log::info!("Setting remote and pushing");
        let git = self.project.git();
        if git.remotes()?.contains("origin") {
            git.rm_remote("origin")?;
        }
        git.add_remote("origin", &repo.ssh_url)?;
        git.run("push", ["-u", "origin", "refs/heads/*", "refs/tags/*"])?;

        let topics = plan.topics;
        log::info!(
            "Setting repository topics to: {}",
            itertools::join(&topics, " ")
        );
        github.set_topics(&repo, topics)?;

        log::info!("Setting protection rules for default branch ...");
        github.set_branch_protection(
            &repo,
            plan.default_branch,
            SetBranchProtection {
                required_status_checks: Some(RequiredStatusChecks {
                    strict: false,
                    contexts: plan.required_checks,
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

        if let Some(token) = plan.codecov_token {
            log::info!("Setting CODECOV_TOKEN secret");
            github.set_actions_secret(&repo, "CODECOV_TOKEN", &token)?;
        }

        if plan.expected_repo_url.is_none() {
            if let Some(ref pkg) = self.root_package {
                log::info!("Setting 'package.repository' field in Cargo.toml ...");
                pkg.set_package_field("repository", repo.html_url)?;
            } else {
                log::info!("Setting 'workspace.package.repository' field in Cargo.toml ...");
                self.project
                    .set_workspace_package_field("repository", repo.html_url)?;
            }
        } else if plan.expected_repo_url != Some(repo.html_url) {
            log::warn!(
                "'{}package.repository' field in Cargo.toml differs from GitHub repository URL",
                if self.root_package.is_some() {
                    ""
                } else {
                    "workspace."
                }
            );
        }

        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct Plan {
    repo_name: String,
    expected_repo_url: Option<String>,
    description: Option<String>,
    private: bool,
    topics: Vec<Topic>,
    required_checks: Vec<&'static str>,
    default_branch: &'static str,
    #[serde(serialize_with = "maybe_redact")]
    codecov_token: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum CodecovTokenSource {
    Cli(String),
    Config,
    None,
}

impl CodecovTokenSource {
    fn resolve(&self, provider: &Provider) -> anyhow::Result<Option<String>> {
        match self {
            CodecovTokenSource::Cli(token) => Ok(Some(token.clone())),
            CodecovTokenSource::Config => Ok(provider.config()?.codecov_token.clone()),
            CodecovTokenSource::None => Ok(None),
        }
    }
}

fn maybe_redact<S: Serializer>(secret: &Option<String>, serializer: S) -> Result<S::Ok, S::Error> {
    if secret.is_some() {
        serializer.serialize_some("--- SECRET ---")
    } else {
        serializer.serialize_none()
    }
}
